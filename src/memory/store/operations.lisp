;;; operations.lisp — Memory store/recall operations.

(in-package :harmonia)

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
  "Only global context classes feed the concept graph (L1 field).
   System logs, tool metrics, and user interactions do NOT pollute the field."
  (member class '(:soul :skill :genesis) :test #'eq))

(defun %palace-worthy-p (class depth)
  "User knowledge and high-value entries go to palace (L3).
   Daily interactions, skill summaries, and explicitly stored content."
  (or (member class '(:daily :interaction) :test #'eq)
      (and (> depth 0) (member class '(:skill) :test #'eq))))

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
    ;; L3: User knowledge → Palace drawers.
    ;; Room IDs: 4=soul, 5=skills, 6=conversations, 7=tools, 8=documents
    (when (and (%palace-worthy-p class depth)
               (fboundp 'mempalace-port-ready-p) (funcall 'mempalace-port-ready-p))
      (handler-case
          (funcall 'palace-file-drawer content
                   (case class (:soul 4) (:skill 5) (:daily 6) (:interaction 6) (:tool 7) (t 8))
                   :tags (mapcar (lambda (tg) (string-downcase (symbol-name tg)))
                                 (remove-if-not #'keywordp all-tags)))
        (error () nil)))
    ;; Reload field graph periodically (only if field-indexable content changed).
    (when (and (%field-indexable-p class)
               (fboundp 'memory-field-port-ready-p) (funcall 'memory-field-port-ready-p)
               (zerop (mod *memory-seq* 5)))
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
                            ;; Room IDs: 4=soul, 5=skills, 6=conversations, 7=tools, 8=documents
                            (room-id (case class
                                       (:soul 4) (:skill 5) (:tool 7) (t 8)))
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

(defun memory-recall (query &key (limit 10))
  "ONE recall function. Field first, recent entries as fallback. No class filter.
The field topology decides relevance. If field unavailable, return recent entries.
Thread-safe: field IPC outside lock, hash-table reads/writes under lock."
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
        ;; Fallback: high-depth entries first, then any recent.
        (let ((deep (%memory-by-depth limit 1)))
          (when deep (setf source "depth-fallback" result-count (length deep))
                (%pipeline-trace :memory-recall
                  :query (%clip-prompt query 60) :source source :result-count result-count))
          deep)
        (let ((recent (memory-recent :limit limit)))
          (when recent (setf source "recent-fallback" result-count (length recent))
                (%pipeline-trace :memory-recall
                  :query (%clip-prompt query 60) :source source :result-count result-count))
          recent))))

;; Legacy compat — old callers use memory-layered-recall
(defun memory-layered-recall (query &key (limit 10) (dive nil))
  (declare (ignore dive))
  (memory-recall query :limit limit))
