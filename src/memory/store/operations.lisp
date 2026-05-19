;;; operations.lisp — Memory store/recall operations.

(in-package :harmonia)

;;; ═══════════════════════════════════════════════════════════════════════
;;; ROUTING POLICY — loaded from config/memory-routing.sexp
;;; ═══════════════════════════════════════════════════════════════════════

(defparameter *memory-routing-config* nil
  "Memory routing policy loaded from config/memory-routing.sexp.")

(defun %memory-routing-config-path ()
  (when (boundp '*boot-file*)
    (merge-pathnames "../../config/memory-routing.sexp"
                     (make-pathname :name nil :type nil :defaults *boot-file*))))

(defun load-memory-routing-config ()
  "Load memory-routing.sexp policy. Called at boot."
  (let ((path (%memory-routing-config-path)))
    (when (probe-file path)
      (handler-case
          (with-open-file (s path :direction :input)
            (let ((*read-eval* nil)
                  (*package* (find-package :harmonia)))
              (setf *memory-routing-config* (read s))))
        (error (e)
          (%log :warn "memory" "Failed to load memory-routing.sexp: ~A" e))))))

(defun %routing-policy (key)
  "Get a routing policy list by key from the :routing section. Returns nil if config not loaded."
  (when *memory-routing-config*
    (let ((routing (getf *memory-routing-config* :routing)))
      (when routing (getf routing key)))))

(defun %memory-should-store-p (class content depth)
  "Write filter: reject entries that add no information to the field.
   No class checks — the field topology decides importance, not labels.
   Returns NIL to reject, T to store."
  (declare (ignore class))
  ;; Entries with depth > 0 are crystallized/compressed — always store.
  (when (> depth 0) (return-from %memory-should-store-p t))
  (let ((text (if (stringp content) content (prin1-to-string content))))
    ;; Reject entries too short to carry semantic meaning.
    (when (< (length text) 20)
      (return-from %memory-should-store-p nil))
    ;; Reject near-duplicate: >80% word overlap with existing recent entry.
    (let ((words (%split-words text)))
      (when (and words (> (length words) 0))
        (block dedup-check
          (let ((count 0))
            (maphash (lambda (_ entry)
                       (declare (ignore _))
                       (when (> count 20) (return-from dedup-check)) ; limit scan
                       (incf count)
                       (let* ((existing (%entry-text entry))
                              (existing-words (%split-words existing))
                              (common (length (intersection words existing-words :test #'string=)))
                              (max-len (max (length words) (length existing-words) 1))
                              (overlap (/ (float common) max-len)))
                         (when (> overlap 0.8)
                           (return-from %memory-should-store-p nil))))
                     *memory-store*)))))
    t))

(defun %field-indexable-p (class)
  "Policy-driven: check routing config for field-indexable classes.
   Honors the :all sentinel (every class is indexed).
   Falls back to indexing every class if config not loaded — the field
   topology decides relevance, not class labels."
  (let ((policy (%routing-policy :field-indexable)))
    (cond
      ((null policy) t)
      ((eq policy :all) t)
      ((listp policy) (member class policy :test #'eq))
      (t t))))

(defun %palace-worthy-p (class depth)
  "Policy-driven: check routing config for palace-worthy classes.
   Falls back to hardcoded defaults if config not loaded."
  (let ((worthy (%routing-policy :palace-worthy))
        (with-depth (%routing-policy :palace-worthy-with-depth)))
    (if (or worthy with-depth)
        (or (member class (or worthy '()) :test #'eq)
            (and (> depth 0) (member class (or with-depth '()) :test #'eq)))
        ;; Fallback: original behavior
        (or (member class '(:daily :interaction) :test #'eq)
            (and (> depth 0) (member class '(:skill) :test #'eq))))))

(defun memory-put (class content &key (depth 0) (tags '()) (source-ids '()))
  "Store a memory entry with layer-separated routing.
   L1 Field:    :soul, :skill, :genesis → concept graph (global context)
   L2 Chronicle: ALL classes → persistent system log
   L3 Palace:   :daily, :interaction, :skill(depth>0) → user knowledge drawers
   Thread-safe: RAM mutations under lock, IPC outside lock."
  (unless (%memory-should-store-p class content depth)
    (return-from memory-put nil))
  (let (id now all-tags)
    (with-memory-lock ()
      (incf *memory-seq*)
      (setf now (get-universal-time))
      (setf id (format nil "~A-~A-~A" class now *memory-seq*))
      (setf all-tags (adjoin class (or tags '()) :test #'eq))
      (let ((entry (make-memory-entry :id id :time now :class class :depth depth
                                       :content content :tags all-tags
                                       :source-ids source-ids
                                       :access-count 0 :last-access nil)))
        (setf (gethash id *memory-store*) entry)
        (%push-class-id class id)
        ;; L1: Only index global context into concept graph (soul/skill/genesis).
        (when (%field-indexable-p class)
          (%index-entry-concepts id class depth content :tags all-tags))))
    ;; L2: ALL entries persist to Chronicle (system log).
    (handler-case (%persist-entry-to-chronicle id now content all-tags source-ids)
      (error (e) (%log :warn "memory" "Persist failed for ~A: ~A" id e)))
    ;; L3: User knowledge → Palace drawers. Rooms created on demand.
    (when (and (%palace-worthy-p class depth)
               (fboundp '%palace-ensure-room))
      (handler-case
          (let ((room-id (funcall '%palace-ensure-room
                                   (funcall '%palace-room-for-class class))))
            (when room-id
              (funcall 'palace-file-drawer content room-id
                       :tags (mapcar (lambda (tg) (string-downcase (symbol-name tg)))
                                     (remove-if-not #'keywordp all-tags)))))
        (error () nil)))
    ;; Reload field graph on every field-indexable put. Serialization now
    ;; happens under the memory lock (see memory-field-load-graph), so
    ;; eager reloads are safe and keep the field in sync with chronicle.
    (when (and (%field-indexable-p class)
               (fboundp 'memory-field-port-ready-p) (funcall 'memory-field-port-ready-p))
      (handler-case (funcall 'memory-field-load-graph) (error () nil)))
    (%pipeline-trace :memory-put :class class :depth depth
      :store-targets (format nil "chronicle~A~A"
                       (if (%field-indexable-p class) "+field" "")
                       (if (%palace-worthy-p class depth) "+palace" ""))
      :content-len (length content))
    id))

;; memory-seed-soul-from-dna is defined in dna.lisp — the DNA is the source of seeds.

(defun %memory-by-depth (limit min-depth)
  "Return entries with depth >= MIN-DEPTH, sorted by time. No class filter."
  (let ((values '()))
    (maphash
     (lambda (_ entry)
       (declare (ignore _))
       (when (>= (memory-entry-depth entry) min-depth)
         (push entry values)))
     *memory-store*)
    (let ((sorted (sort values #'> :key #'memory-entry-time)))
      (subseq sorted 0 (min limit (length sorted))))))

(defun memory-recent (&key (limit 5) class (max-depth nil))
  (let ((values '()))
    (maphash
     (lambda (_ entry)
       (declare (ignore _))
       (when (and (or (null class) (eq class (memory-entry-class entry)))
                  (or (null max-depth) (<= (memory-entry-depth entry) max-depth)))
         (push entry values)))
             *memory-store*)
    (subseq (sort values #'> :key #'memory-entry-time)
            0
            (min limit (length values)))))

(defun memory-record-tool-usage (tool-name &key latency-ms success)
  (memory-put :tool
              (list :tool tool-name
                    :latency-ms latency-ms
                    :success success)
              :depth 0
              :tags (list :tool-metric)))

(defun %populate-palace-from-memory ()
  "File high-value memory entries as palace drawers. Called at boot.
   Only entries with depth >= 1 (crystallized/identity) are filed."
  (let ((filed 0))
    (maphash (lambda (id entry)
               (declare (ignore id))
               (when (and (>= (memory-entry-depth entry) 1)
                          (stringp (memory-entry-content entry))
                          (> (length (memory-entry-content entry)) 30)
                          (fboundp 'palace-file-drawer))
                 (handler-case
                     (let* ((class (memory-entry-class entry))
                            (room-id (when (fboundp '%palace-ensure-room)
                                       (funcall '%palace-ensure-room
                                                (funcall '%palace-room-for-class class))))
                            (tags (memory-entry-tags entry))
                            (tag-strings (mapcar (lambda (tg)
                                                   (string-downcase (symbol-name tg)))
                                                 (remove-if-not #'keywordp tags))))
                       (funcall 'palace-file-drawer
                                (memory-entry-content entry) room-id
                                :tags tag-strings)
                       (incf filed))
                   (error () nil))))
             *memory-store*)
    (%log :info "mempalace" "Filed ~D entries as palace drawers." filed)
    filed))

(defun memory-record-orchestration (prompt response tool score latency-ms &key harmony)
  "Record interaction: L3 palace (user knowledge) + L2 chronicle (system log).
   The palace gets readable text. The chronicle gets the full plist."
  (let* ((text (format nil "Q: ~A~%A: ~A" (%clip-prompt prompt 300) (%clip-prompt response 500)))
         (daily-id
           (memory-put :daily text
                       :depth 0
                       :tags (list :interaction :orchestration))))
    ;; Also log delegation metrics to chronicle as system data
    (handler-case
        (memory-put :tool
                    (format nil "(DELEGATION :tool ~A :score ~,3F :latency-ms ~D)"
                            tool (or score 0.0) (or latency-ms 0))
                    :depth 0
                    :tags (list :delegation :metrics))
      (error () nil))
    daily-id))

(defparameter *memory-recall-tau-seconds* (* 14 86400.0)
  "Recency decay time constant for content-substring recall fallback (14 days).")

(defun %memory-substring-recall (query limit)
  "Scan *memory-store* for entries whose words intersect QUERY.
   Score by word_overlap × exp(-age/TAU). Returns up to LIMIT entries
   in descending score order. Thread-safe: walk happens under the memory lock."
  (let ((q-words (%split-words (or query "")))
        (matches '())
        (now (get-universal-time)))
    (when q-words
      (with-memory-lock ()
        (maphash (lambda (_ entry)
                   (declare (ignore _))
                   (let* ((words (%split-words (%entry-text entry)))
                          (common (when words
                                    (length (intersection q-words words :test #'string=)))))
                     (when (and common (>= common 1))
                       (let* ((age (- now (or (memory-entry-time entry) now)))
                              (decay (exp (- (/ (float age 1.0d0)
                                                *memory-recall-tau-seconds*))))
                              (score (* common decay)))
                         (push (cons score entry) matches)))))
                 *memory-store*)))
    (when matches
      (mapcar #'cdr
              (subseq (sort matches #'> :key #'car)
                      0 (min limit (length matches)))))))

(defun memory-recall (query &key (limit 10))
  "ONE recall function with four-tier fallback:
     1. Field topology recall (Rust engine via IPC).
     2. Content-substring scan of *memory-store* with recency-decayed score.
     3. High-depth entries (crystallized identity).
     4. Most-recent entries (unscored).
   Thread-safe: field IPC outside lock, hash-table walks under lock."
  (let ((source "none") (result-count 0))
    (or (handler-case
            (when (and (fboundp 'memory-field-port-ready-p)
                       (funcall 'memory-field-port-ready-p))
              ;; Field IPC call OUTSIDE lock — can take up to 90s.
              (let* ((field-result (funcall 'memory-field-recall query :limit (* limit 3)))
                     (activations (and (listp field-result) (getf field-result :activations)))
                     (all '()))
                ;; Hash-table reads/writes UNDER lock.
                (with-memory-lock ()
                  (dolist (act activations)
                    (when (listp act)
                      (dolist (entry-id (getf act :entries))
                        (when (stringp entry-id)
                          (let ((entry (gethash entry-id *memory-store*)))
                            (when entry
                              (incf (memory-entry-access-count entry))
                              (setf (memory-entry-last-access entry) (get-universal-time))
                              (push (cons (or (getf act :score) 0.0) entry) all))))))))
                (when all
                  (setf source "field" result-count (length all))
                  (let ((results (mapcar #'cdr
                                   (subseq (sort (remove-duplicates all
                                                   :key (lambda (p) (memory-entry-id (cdr p)))
                                                   :test #'string=)
                                                 #'> :key #'car)
                                           0 (min limit (length all))))))
                    (%pipeline-trace :memory-recall
                      :query (%clip-prompt query 60)
                      :source source :result-count result-count)
                    results))))
          (error () nil))
        ;; Fallback 1: content-substring scan with recency decay.
        ;; Re-promotes cold chronicle entries that match the query.
        (let ((matches (%memory-substring-recall query limit)))
          (when matches
            (setf source "substring-fallback" result-count (length matches))
            (%pipeline-trace :memory-recall
              :query (%clip-prompt query 60) :source source :result-count result-count))
          matches)
        ;; Fallback 2: high-depth entries.
        (let ((deep (%memory-by-depth limit 1)))
          (when deep (setf source "depth-fallback" result-count (length deep))
                (%pipeline-trace :memory-recall
                  :query (%clip-prompt query 60) :source source :result-count result-count))
          deep)
        ;; Fallback 3: most recent entries.
        (let ((recent (memory-recent :limit limit)))
          (when recent (setf source "recent-fallback" result-count (length recent))
                (%pipeline-trace :memory-recall
                  :query (%clip-prompt query 60) :source source :result-count result-count))
          recent))))

;; Legacy compat — old callers use memory-layered-recall
(defun memory-layered-recall (query &key (limit 10) (dive nil))
  (declare (ignore dive))
  (memory-recall query :limit limit))
