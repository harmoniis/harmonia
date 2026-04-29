;;; memory-field.lisp — Port: Memory field recall via IPC.
;;;
;;; Field propagation on the concept graph for dynamical memory recall.
;;; Replaces substring matching with attractor dynamics and wave propagation.
;;;
;;; All IPC replies from the Rust engine follow the form (:ok :key val :key val ...).
;;; Uses the shared %parse-port-reply from mempalace.lisp (one function, not duplicated).
;;;
;;; HOMOICONIC: all IPC commands are built as Lisp LISTS, then serialized
;;; with %sexp-to-ipc-string. S-expressions are law — no format strings.

(in-package :harmonia)

(defparameter *memory-field-ready* nil)

;;; --- Port lifecycle ---

(defun memory-field-port-ready-p ()
  *memory-field-ready*)

(defun init-memory-field-port ()
  (let ((reply (ipc-call (%sexp-to-ipc-string
                           '(:component "memory-field" :op "init")))))
    (setf *memory-field-ready* (and reply (ipc-reply-ok-p reply)))
    (runtime-log *runtime* :memory-field-init
                 (list :status (if *memory-field-ready* :ok :failed)))
    *memory-field-ready*))

;;; --- Graph and recall ---

(defun memory-field-load-graph ()
  "Serialize the current concept graph and send to the Rust field engine."
  (when (not (memory-field-port-ready-p))
    (return-from memory-field-load-graph nil))
  (let* ((nodes-sexp (%serialize-field-nodes))
         (edges-sexp (%serialize-field-edges)))
    (ipc-call (%sexp-to-ipc-string
               `(:component "memory-field" :op "load-graph"
                 :nodes ,nodes-sexp :edges ,edges-sexp)))))

(defun memory-field-recall (query &key (limit 10))
  "Field-based recall: send query to Rust, get scored activations back.
Returns a plist (:activations (...) :basin (...) :thomas (...)) or nil."
  (when (not (memory-field-port-ready-p))
    (return-from memory-field-recall nil))
  (let* ((concepts (%split-words (or query "")))
         (access-sexp (%field-access-counts concepts))
         (concepts-sexp (%list-to-sexp-strings concepts))
         (reply (ipc-call
                 (%sexp-to-ipc-string
                  `(:component "memory-field" :op "field-recall"
                    :query-concepts ,concepts-sexp
                    :access-counts ,access-sexp
                    :limit ,limit)))))
    (%parse-port-reply reply)))

;;; --- Basin and attractor queries ---

(defun memory-field-basin-status ()
  "Return current attractor basin status as plist."
  (when (not (memory-field-port-ready-p))
    (return-from memory-field-basin-status nil))
  (%parse-port-reply
   (ipc-call (%sexp-to-ipc-string
               '(:component "memory-field" :op "basin-status")))))

(defun memory-field-step-attractors (&key (signal 0.5) (noise 0.5))
  "Step all three attractors. Called during :attractor-sync harmonic phase."
  (when (not (memory-field-port-ready-p))
    (return-from memory-field-step-attractors nil))
  (ipc-call (%sexp-to-ipc-string
              `(:component "memory-field" :op "step-attractors"
                :signal ,signal :noise ,noise))))

(defun memory-field-eigenmode-status ()
  "Return spectral state plist including last-recall :coherence in [0, 1]."
  (when (not (memory-field-port-ready-p))
    (return-from memory-field-eigenmode-status nil))
  (%parse-port-reply
   (ipc-call (%sexp-to-ipc-string
               '(:component "memory-field" :op "eigenmode-status")))))

;;; --- Dreaming — field self-maintenance ---

(defun memory-field-dream ()
  "Trigger field dreaming. Returns plist with :pruned and :crystallized entry IDs."
  (when (not (memory-field-port-ready-p))
    (return-from memory-field-dream nil))
  (%parse-port-reply
   (ipc-call (%sexp-to-ipc-string
               '(:component "memory-field" :op "dream")))))

(defun %apply-dream-results (dream-report)
  "Apply dream results: merge entries (compress), prune only truly redundant,
   crystallize structural entries. Landauer: prefer merge over delete."
  (when (null dream-report) (return-from %apply-dream-results nil))
  (let ((pruned (getf dream-report :pruned))
        (merged (getf dream-report :merged))
        (crystallized (getf dream-report :crystallized))
        (stats (getf dream-report :stats))
        (pruned-count 0)
        (merged-count 0)
        (crystallized-count 0))
    ;; MERGE (primary operation — compression, not destruction).
    ;; Each merge group is a list of entry IDs to compress into one.
    ;; The merged entry preserves meaning at higher depth.
    (when (listp merged)
      (dolist (group merged)
        (when (and (listp group) (>= (length group) 2))
          (let ((texts '())
                (all-tags '())
                (max-depth 0))
            ;; Gather text and tags from all entries in the group.
            (dolist (entry-id group)
              (when (stringp entry-id)
                (let ((entry (gethash entry-id *memory-store*)))
                  (when entry
                    (push (%entry-text entry) texts)
                    (setf all-tags (union all-tags (memory-entry-tags entry) :test #'eq))
                    (setf max-depth (max max-depth (memory-entry-depth entry)))
                    ;; Remove the old entry.
                    (remhash entry-id *memory-store*)))))
            ;; Create compressed entry at depth+1 (crystallized by compression).
            (when texts
              (let ((compressed (format nil "~{~A~^ | ~}"
                                        (mapcar (lambda (txt) (subseq txt 0 (min 100 (length txt))))
                                                (nreverse texts)))))
                (memory-put :system compressed
                            :depth (min 3 (1+ max-depth))
                            :tags (adjoin :compressed (adjoin :dream-merged all-tags) :test #'eq))
                (incf merged-count)))))))
    ;; PRUNE (rare — only when K(m|graph) ~ 0, betweenness = 0).
    (when (listp pruned)
      (dolist (entry-id pruned)
        (when (stringp entry-id)
          (let ((entry (gethash entry-id *memory-store*)))
            (when entry
              (remhash entry-id *memory-store*)
              (incf pruned-count))))))
    ;; CRYSTALLIZE (promote depth of structural skeleton nodes).
    (when (listp crystallized)
      (dolist (entry-id crystallized)
        (when (stringp entry-id)
          (let ((entry (gethash entry-id *memory-store*)))
            (when (and entry (< (memory-entry-depth entry) 3))
              (incf (memory-entry-depth entry))
              (incf crystallized-count))))))
    (%log :info "dream" "Applied: ~D merged, ~D pruned, ~D crystallized (stats: ~A)"
          merged-count pruned-count crystallized-count stats)
    (list :pruned pruned-count :merged merged-count :crystallized crystallized-count)))

;;; --- Warm-start from Chronicle ---

(defun memory-field-restore-basin (basin energy dwell threshold)
  "Restore basin state from Chronicle values for warm boot."
  (when (not (memory-field-port-ready-p))
    (return-from memory-field-restore-basin nil))
  (ipc-call (%sexp-to-ipc-string
              `(:component "memory-field" :op "restore-basin"
                :basin ,basin :coercive-energy ,energy
                :dwell-ticks ,dwell :threshold ,threshold))))

(defun memory-field-warm-start-from-chronicle ()
  "On boot, restore last known basin state from Chronicle. Non-fatal."
  (when (not (memory-field-port-ready-p))
    (return-from memory-field-warm-start-from-chronicle nil))
  (let ((plist (%parse-port-reply
                (ipc-call (%sexp-to-ipc-string
                            '(:component "memory-field" :op "last-field-basin"))))))
    (when plist
      (let ((basin (getf plist :basin))
            (energy (or (getf plist :coercive-energy) 0.0))
            (dwell (or (getf plist :dwell-ticks) 0))
            (threshold (or (getf plist :threshold) 0.35)))
        (when basin
          (memory-field-restore-basin basin energy dwell threshold)
          (runtime-log *runtime* :memory-field-warm-start
                       (list :basin basin :energy energy :dwell dwell)))))))

;;; --- Serialization helpers ---

(defun %serialize-field-nodes ()
  "Serialize *memory-concept-nodes* as sexp for the field engine."
  (let ((items '()))
    (maphash (lambda (_ node)
               (declare (ignore _))
               (push (%sexp-to-ipc-string
                      `(:concept ,(getf node :concept)
                        :domain ,(princ-to-string (getf node :domain))
                        :count ,(getf node :count)
                        :entries ,(getf node :entries)))
                     items))
             *memory-concept-nodes*)
    (format nil "(~{~A~^ ~})" items)))

(defun %serialize-field-edges ()
  "Serialize *memory-concept-edges* as sexp for the field engine.
   Includes directed weights for A-B topological flux computation."
  (let ((items '()))
    (maphash (lambda (_ edge)
               (declare (ignore _))
               (let* ((a (getf edge :a))
                      (b (getf edge :b))
                      (fwd-key (format nil "~A>~A" a b))
                      (rev-key (format nil "~A>~A" b a))
                      (fwd (or (gethash fwd-key *memory-concept-directed-counts*) 0))
                      (rev (or (gethash rev-key *memory-concept-directed-counts*) 0)))
                 (push (%sexp-to-ipc-string
                        `(:a ,a
                          :b ,b
                          :weight ,(getf edge :weight)
                          :interdisciplinary ,(if (getf edge :interdisciplinary) t nil)
                          :forward-weight ,(max 1 fwd)
                          :reverse-weight ,(max 1 rev)))
                       items)))
             *memory-concept-edges*)
    (format nil "(~{~A~^ ~})" items)))

(defun %field-access-counts (concepts)
  "Build access-count sexp for field recall, including last-access time for temporal decay."
  (let ((items '())
        (now (get-universal-time)))
    (dolist (c concepts)
      (let ((node (gethash c *memory-concept-nodes*)))
        (when node
          ;; Find most recent access time from entries linked to this concept.
          (let ((max-access 0))
            (dolist (eid (getf node :entries))
              (let ((entry (gethash eid *memory-store*)))
                (when (and entry (memory-entry-last-access entry))
                  (setf max-access (max max-access (memory-entry-last-access entry))))))
            (push (%sexp-to-ipc-string
                   `(:concept ,c :count ,(getf node :count)
                     :last-access ,(if (> max-access 0) max-access now)))
                  items)))))
    (format nil "(~{~A~^ ~})" items)))

(defun %list-to-sexp-strings (lst)
  "Format a list of strings as sexp string list."
  (%sexp-to-ipc-string (or lst '())))
