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
  "Build access-count sexp for field recall."
  (let ((items '()))
    (dolist (c concepts)
      (let ((node (gethash c *memory-concept-nodes*)))
        (when node
          (push (format nil "(:concept \"~A\" :count ~D)"
                        (sexp-escape-lisp c) (getf node :count))
                items))))
    (format nil "(~{~A~^ ~})" items)))

(defun %list-to-sexp-strings (lst)
  "Format a list of strings as sexp string list."
  (format nil "(~{\"~A\"~^ ~})" (mapcar #'sexp-escape-lisp (or lst '()))))
