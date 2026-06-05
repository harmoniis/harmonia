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
   L1 Field:    policy-selected classes → concept graph (global context)
   L2 Chronicle: ALL classes → persistent system log
   L3 Palace:   policy-selected user knowledge → graph + drawers
   Thread-safe: RAM mutations under lock, IPC outside lock."
  (unless (%memory-should-store-p class content depth)
    (return-from memory-put nil))
  (let (id now all-tags indexed-concepts)
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
        ;; L1: policy-selected classes enter the field graph.
        (when (%field-indexable-p class)
          (setf indexed-concepts
                (%index-entry-concepts id class depth content :tags all-tags)))))
    ;; L2: ALL entries persist to Chronicle (system log).
    (handler-case (%persist-entry-to-chronicle id now content all-tags source-ids)
      (error (e) (%log :warn "memory" "Persist failed for ~A: ~A" id e)))
    ;; L3: User knowledge → palace graph + drawers. IPC outside the memory lock.
    (when (and (%palace-worthy-p class depth)
               (fboundp '%palace-file-memory-entry))
      (handler-case
          (funcall '%palace-file-memory-entry class content
                   :tags all-tags
                   :concepts indexed-concepts
                   :id id)
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

(defun %palace-reconcile-from-memory ()
  "Boot reconciliation. Chronicle (L2) is the durable record; the palace (L3) is a
projection that warm-starts from its own disk journal, then converges here: file
into the palace exactly the chronicle-loaded entries it lacks, keyed strictly on
entry-id. This is idempotent — it never re-files an entry the palace already holds,
so it closes the mid-`memory-put` crash window without ever duplicating a drawer.
Returns the number of entries filed."
  (unless (and (fboundp 'palace-entry-ids)
               (fboundp 'mempalace-port-ready-p) (funcall 'mempalace-port-ready-p)
               (fboundp '%palace-file-memory-entry))
    (return-from %palace-reconcile-from-memory 0))
  (let ((have (funcall 'palace-entry-ids))
        (missing '()))
    ;; Collect under the lock; do IPC filing outside it (mirror memory-put).
    ;; Note: an entry whose content is below the engine's drawer minimum never
    ;; gains a Memory drawer, so it stays "missing" and is re-attempted each boot.
    ;; This is outcome-idempotent (it files no drawer and the count returns 0),
    ;; only a cheap rebuild of its already-deduped nodes — bounded, not a leak.
    (with-memory-lock ()
      (maphash
       (lambda (id entry)
         (when (and (%palace-worthy-p (memory-entry-class entry) (memory-entry-depth entry))
                    (not (gethash (princ-to-string id) have)))
           (push (list id (memory-entry-class entry)
                       (memory-entry-content entry) (memory-entry-tags entry))
                 missing)))
       *memory-store*))
    (let ((filed 0))
      (dolist (m missing)
        (destructuring-bind (id class content tags) m
          (handler-case
              ;; Count only entries that actually produced a drawer. Entries the
              ;; engine declines (e.g. degenerate content) never gain a Memory
              ;; drawer, so they stay "missing" — counting them would make the
              ;; reconciliation report phantom work on every boot.
              (let ((res (funcall '%palace-file-memory-entry class content
                                  :tags tags :concepts nil :id id)))
                (when (getf res :drawer-filed) (incf filed)))
            (error (e) (%log :warn "palace" "reconcile ~A failed: ~A" id e)))))
      (when (> filed 0)
        (%log :info "palace" "Reconciled ~D missing palace entries from chronicle." filed))
      filed)))

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
  "File high-value memory entries into the palace. Called at boot."
  (let ((filed 0))
    (maphash (lambda (id entry)
               (declare (ignore id))
               (when (and (>= (memory-entry-depth entry) 1)
                          (stringp (memory-entry-content entry))
                          (> (length (memory-entry-content entry)) 30)
                          (fboundp '%palace-file-memory-entry))
                 (handler-case
                     (let ((class (memory-entry-class entry))
                           (tags (memory-entry-tags entry)))
                       (funcall '%palace-file-memory-entry class
                                (memory-entry-content entry)
                                :tags tags)
                       (incf filed))
                   (error () nil))))
             *memory-store*)
    (%log :info "mempalace" "Filed ~D entries into the palace." filed)
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

(defun %memory-rank-greater-p (left right)
  "Lexicographic comparison for declarative recall rank vectors."
  (loop for l in left
        for r in right
        when (> l r) return t
        when (< l r) return nil
        finally (return nil)))

(defun %memory-tag-name (tag)
  (string-downcase
   (cond ((symbolp tag) (symbol-name tag))
         ((stringp tag) tag)
         (t (princ-to-string tag)))))

(defun %memory-entry-has-tag-p (entry tag)
  (let ((wanted (%memory-tag-name tag)))
    (some (lambda (entry-tag)
            (string= wanted (%memory-tag-name entry-tag)))
          (memory-entry-tags entry))))

(defun %memory-entry-recall-text (entry)
  "Return the user-knowledge portion of ENTRY.
Interaction prompts echo the recall query and must not inflate lexical rank."
  (let* ((text (%entry-text entry))
         (marker (and (%memory-entry-has-tag-p entry :interaction)
                      (search (format nil "~%A: ") text :test #'char-equal))))
    (if marker
        (subseq text (+ marker 4))
        text)))

(defun %memory-entry-recall-rank (query-words entry)
  "Rank one recall candidate by relevance, provenance, then recency.
With no lexical evidence, all candidates tie so stable sort preserves the
memory-field's semantic ordering."
  (let* ((text (%memory-entry-recall-text entry))
         (entry-words (%split-words text))
         (overlap (length (intersection query-words entry-words :test #'string=))))
    (if (plusp overlap)
        (let* ((precision (/ (float overlap) (max 1 (length entry-words))))
               (recall (/ (float overlap) (max 1 (length query-words))))
               (f1 (/ (* 2.0 precision recall) (max 1.0e-6 (+ precision recall)))))
          (list 1
                f1
                overlap
                (if (%memory-entry-has-tag-p entry :user-stored) 1 0)
                (or (memory-entry-time entry) 0)
                (- (length text))))
        '(0 0 0 0 0 0))))

(defun %rank-memory-entries (query entries &key (dedupe-key #'memory-entry-id))
  "Return unique ENTRIES ordered by the shared declarative recall policy."
  (let ((query-words (%split-words query)))
    (stable-sort
     (remove-duplicates (copy-list entries)
                        :key dedupe-key
                        :test #'string=
                        :from-end t)
     (lambda (left right)
       (%memory-rank-greater-p
        (%memory-entry-recall-rank query-words left)
        (%memory-entry-recall-rank query-words right))))))

(defun %memory-substring-recall (query limit)
  "Return lexically relevant store entries under the shared recall policy."
  (let ((q-words (%split-words (or query "")))
        (matches '()))
    (when q-words
      (with-memory-lock ()
        (maphash (lambda (_ entry)
                   (declare (ignore _))
                   (when (intersection q-words
                                       (%split-words (%memory-entry-recall-text entry))
                                       :test #'string=)
                     (push entry matches)))
                 *memory-store*)))
    (when matches
      (let ((ranked (%rank-memory-entries query matches)))
        (subseq ranked 0 (min limit (length ranked)))))))

(defun %memory-field-recall-entries (query limit)
  "Resolve field activations to memory entries, preserving semantic score order."
  (handler-case
      (when (and (fboundp 'memory-field-port-ready-p)
                 (funcall 'memory-field-port-ready-p))
        ;; Field IPC call OUTSIDE lock — can take up to 90s.
        (let* ((field-result (funcall 'memory-field-recall query :limit (* limit 3)))
               (activations (and (listp field-result) (getf field-result :activations)))
               (scored '()))
          (with-memory-lock ()
            (dolist (activation activations)
              (when (listp activation)
                (dolist (entry-id (getf activation :entries))
                  (when (stringp entry-id)
                    (let ((entry (gethash entry-id *memory-store*)))
                      (when entry
                        (setf (memory-entry-access-count entry)
                              (1+ (or (memory-entry-access-count entry) 0)))
                        (setf (memory-entry-last-access entry) (get-universal-time))
                        (push (cons (or (getf activation :score) 0.0) entry)
                              scored))))))))
          (mapcar #'cdr
                  (sort (remove-duplicates scored
                                           :key (lambda (pair)
                                                  (memory-entry-id (cdr pair)))
                                           :test #'string=
                                           :from-end t)
                        #'> :key #'car))))
    (error () nil)))

(defun memory-recall (query &key (limit 10))
  "Recall through one ranked path.
Field topology and lexical-store candidates are unioned before ranking so a
weak semantic hit cannot starve an exact stored fact. High-depth and recent
entries remain contextual fallbacks only when no relevant candidate exists."
  (let* ((count (if (and (integerp limit) (plusp limit)) limit 10))
         (field (%memory-field-recall-entries query count))
         (lexical (%memory-substring-recall query count))
         (relevant (%rank-memory-entries query (append field lexical)))
         (results (or (and relevant
                           (subseq relevant 0 (min count (length relevant))))
                      (%memory-by-depth count 1)
                      (memory-recent :limit count)))
         (source (cond ((and field lexical) "field+substring")
                       (field "field")
                       (lexical "substring")
                       ((some (lambda (entry) (>= (memory-entry-depth entry) 1))
                              results)
                        "depth-fallback")
                       (results "recent-fallback")
                       (t "none"))))
    (%pipeline-trace :memory-recall
      :query (%clip-prompt query 60)
      :source source
      :result-count (length results))
    results))
