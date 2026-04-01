;;; memory-field.lisp — Port: Memory field recall via IPC.
;;;
;;; Field propagation on the concept graph for dynamical memory recall.
;;; Replaces substring matching with attractor dynamics and wave propagation.
;;;
;;; All IPC replies from the Rust engine follow the form (:ok :key val :key val ...).
;;; The leading :ok is a status marker, not a plist key. %parse-field-reply strips
;;; it so callers receive a clean plist: (:key val :key val ...).

(in-package :harmonia)

(defparameter *memory-field-ready* nil)

;;; ─── Reply parsing ──────────────────────────────────────────────────

(defun %parse-field-reply (reply)
  "Parse an IPC reply sexp, stripping the leading :ok status marker.
Returns a plist on success, nil on failure. Safe: no eval, no crash."
  (when (and reply (stringp reply) (ipc-reply-ok-p reply))
    (let ((*read-eval* nil))
      (handler-case
          (let ((parsed (read-from-string reply)))
            ;; Rust returns (:ok :key val ...). Strip :ok to get a valid plist.
            (cond
              ((and (listp parsed) (eq (car parsed) :ok))
               (cdr parsed))
              ((listp parsed) parsed)
              (t nil)))
        (error () nil)))))

;;; ─── Port lifecycle ─────────────────────────────────────────────────

(defun memory-field-port-ready-p ()
  *memory-field-ready*)

(defun init-memory-field-port ()
  (let ((reply (ipc-call "(:component \"memory-field\" :op \"init\")")))
    (setf *memory-field-ready* (and reply (ipc-reply-ok-p reply)))
    (runtime-log *runtime* :memory-field-init
                 (list :status (if *memory-field-ready* :ok :failed)))
    *memory-field-ready*))

;;; ─── Graph and recall ───────────────────────────────────────────────

(defun memory-field-load-graph ()
  "Serialize the current concept graph and send to the Rust field engine."
  (when (not (memory-field-port-ready-p))
    (return-from memory-field-load-graph nil))
  (let* ((nodes-sexp (%serialize-field-nodes))
         (edges-sexp (%serialize-field-edges))
         (sexp (format nil "(:component \"memory-field\" :op \"load-graph\" :nodes ~A :edges ~A)"
                       nodes-sexp edges-sexp)))
    (ipc-call sexp)))

(defun memory-field-recall (query &key (limit 10))
  "Field-based recall: send query to Rust, get scored activations back.
Returns a plist (:activations (...) :basin (...) :thomas (...)) or nil."
  (when (not (memory-field-port-ready-p))
    (return-from memory-field-recall nil))
  (let* ((concepts (%split-words (or query "")))
         (access-sexp (%field-access-counts concepts))
         (concepts-sexp (%list-to-sexp-strings concepts))
         (sexp (format nil "(:component \"memory-field\" :op \"field-recall\" :query-concepts ~A :access-counts ~A :limit ~D)"
                       concepts-sexp access-sexp limit))
         (reply (ipc-call sexp)))
    (%parse-field-reply reply)))

;;; ─── Basin and attractor queries ────────────────────────────────────

(defun memory-field-basin-status ()
  "Return current attractor basin status as plist."
  (when (not (memory-field-port-ready-p))
    (return-from memory-field-basin-status nil))
  (%parse-field-reply
   (ipc-call "(:component \"memory-field\" :op \"basin-status\")")))

(defun memory-field-step-attractors (&key (signal 0.5) (noise 0.5))
  "Step all three attractors. Called during :attractor-sync harmonic phase."
  (when (not (memory-field-port-ready-p))
    (return-from memory-field-step-attractors nil))
  (ipc-call (format nil "(:component \"memory-field\" :op \"step-attractors\" :signal ~F :noise ~F)"
                    signal noise)))

;;; ─── Dreaming — field self-maintenance ──────────────────────────────

(defun memory-field-dream ()
  "Trigger field dreaming. Returns plist with :pruned and :crystallized entry IDs."
  (when (not (memory-field-port-ready-p))
    (return-from memory-field-dream nil))
  (%parse-field-reply
   (ipc-call "(:component \"memory-field\" :op \"dream\")")))

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
                                        (mapcar (lambda (t) (subseq t 0 (min 100 (length t))))
                                                (nreverse texts)))))
                (memory-put :system compressed
                            :depth (min 3 (1+ max-depth))
                            :tags (adjoin :compressed (adjoin :dream-merged all-tags) :test #'eq))
                (incf merged-count)))))))
    ;; PRUNE (rare — only when K(m|graph) ≈ 0, betweenness = 0).
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

;;; ─── Warm-start from Chronicle ──────────────────────────────────────

(defun memory-field-restore-basin (basin energy dwell threshold)
  "Restore basin state from Chronicle values for warm boot."
  (when (not (memory-field-port-ready-p))
    (return-from memory-field-restore-basin nil))
  (ipc-call (format nil "(:component \"memory-field\" :op \"restore-basin\" :basin \"~A\" :coercive-energy ~F :dwell-ticks ~D :threshold ~F)"
                    (sexp-escape-lisp basin) energy dwell threshold)))

(defun memory-field-warm-start-from-chronicle ()
  "On boot, restore last known basin state from Chronicle. Non-fatal."
  (when (not (memory-field-port-ready-p))
    (return-from memory-field-warm-start-from-chronicle nil))
  (let ((plist (%parse-field-reply
                (ipc-call "(:component \"memory-field\" :op \"last-field-basin\")"))))
    (when plist
      (let ((basin (getf plist :basin))
            (energy (or (getf plist :coercive-energy) 0.0))
            (dwell (or (getf plist :dwell-ticks) 0))
            (threshold (or (getf plist :threshold) 0.35)))
        (when basin
          (memory-field-restore-basin basin energy dwell threshold)
          (runtime-log *runtime* :memory-field-warm-start
                       (list :basin basin :energy energy :dwell dwell)))))))

;;; ─── Serialization helpers ──────────────────────────────────────────

(defun %serialize-field-nodes ()
  "Serialize *memory-concept-nodes* as sexp for the field engine."
  (let ((items '()))
    (maphash (lambda (_ node)
               (declare (ignore _))
               (push (format nil "(:concept \"~A\" :domain \"~A\" :count ~D :entries ~A)"
                             (sexp-escape-lisp (getf node :concept))
                             (getf node :domain)
                             (getf node :count)
                             (%list-to-sexp-strings (getf node :entries)))
                     items))
             *memory-concept-nodes*)
    (format nil "(~{~A~^ ~})" items)))

(defun %serialize-field-edges ()
  "Serialize *memory-concept-edges* as sexp for the field engine."
  (let ((items '()))
    (maphash (lambda (_ edge)
               (declare (ignore _))
               (push (format nil "(:a \"~A\" :b \"~A\" :weight ~D :interdisciplinary ~A)"
                             (sexp-escape-lisp (getf edge :a))
                             (sexp-escape-lisp (getf edge :b))
                             (getf edge :weight)
                             (if (getf edge :interdisciplinary) "t" "nil"))
                     items))
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
            (push (format nil "(:concept \"~A\" :count ~D :last-access ~D)"
                          (sexp-escape-lisp c) (getf node :count)
                          (if (> max-access 0) max-access now))
                  items)))))
    (format nil "(~{~A~^ ~})" items)))

(defun %list-to-sexp-strings (lst)
  "Format a list of strings as sexp string list."
  (format nil "(~{\"~A\"~^ ~})" (mapcar #'sexp-escape-lisp (or lst '()))))
