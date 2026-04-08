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

(defun memory-put (class content &key (depth 0) (tags '()) (source-ids '()))
  "Store a memory entry. Write filter rejects duplicates and noise.
Thread-safe: RAM mutations under lock, Chronicle persist outside lock."
  ;; Write filter: reject entries that add no information.
  (unless (%memory-should-store-p class content depth)
    (return-from memory-put nil))
  (let (id now all-tags)
    ;; Lock scope: RAM mutations only. No IPC under lock.
    (with-memory-lock ()
      (incf *memory-seq*)
      (setf now (get-universal-time))
      (setf id (format nil "~A-~A-~A" class now *memory-seq*))
      (setf all-tags (adjoin class (or tags '()) :test #'eq))
      (let ((entry (make-memory-entry :id id
                                       :time now
                                       :class class
                                       :depth depth
                                       :content content
                                       :tags all-tags
                                       :source-ids source-ids
                                       :access-count 0
                                       :last-access nil)))
        (setf (gethash id *memory-store*) entry)
        (%push-class-id class id)
        (%index-entry-concepts id class depth content :tags all-tags)))
    ;; Persist to Chronicle OUTSIDE the lock — IPC can take 90s, must not block other memory ops.
    (handler-case (%persist-entry-to-chronicle id now content all-tags source-ids)
      (error (e) (%log :warn "memory" "Persist failed for ~A: ~A" id e)))
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

(defun memory-record-orchestration (prompt response tool score latency-ms &key harmony)
  (let ((daily-id
          (memory-put :daily
                      (list :prompt prompt
                            :response response
                            :score score
                            :tool tool
                            :latency-ms latency-ms
                            :harmony harmony
                            :channel :human)
                      :depth 0
                      :tags (list :interaction :orchestration))))
    (%upsert-concept-edge (string-downcase tool) "memory" :tool-memory)
    daily-id))

(defun memory-recall (query &key (limit 10))
  "ONE recall function. Field first, recent entries as fallback. No class filter.
The field topology decides relevance. If field unavailable, return recent entries.
Thread-safe: field IPC outside lock, hash-table reads/writes under lock."
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
                (mapcar #'cdr
                  (subseq (sort (remove-duplicates all
                                  :key (lambda (p) (memory-entry-id (cdr p)))
                                  :test #'string=)
                                #'> :key #'car)
                          0 (min limit (length all)))))))
        (error () nil))
      ;; Fallback: high-depth entries first (crystallized = structural/identity), then any recent.
      ;; No class filter — depth is the field's way of saying "this matters".
      (%memory-by-depth limit 1)
      (memory-recent :limit limit)))

;; Legacy compat — old callers use memory-layered-recall
(defun memory-layered-recall (query &key (limit 10) (dive nil))
  (declare (ignore dive))
  (memory-recall query :limit limit))
