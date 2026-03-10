;;; signalograd.lisp — Port: Signalograd adaptive kernel via CFFI.

(in-package :harmonia)

(defparameter *signalograd-lib* nil)
(defparameter *signalograd-ready* nil)

(cffi:defcfun ("harmonia_signalograd_version" %signalograd-version) :string)
(cffi:defcfun ("harmonia_signalograd_healthcheck" %signalograd-healthcheck) :int)
(cffi:defcfun ("harmonia_signalograd_init" %signalograd-init) :int)
(cffi:defcfun ("harmonia_signalograd_observe" %signalograd-observe) :int
  (observation-sexp :string))
(cffi:defcfun ("harmonia_signalograd_reflect" %signalograd-reflect) :int
  (observation-json :string))
(cffi:defcfun ("harmonia_signalograd_feedback" %signalograd-feedback) :int
  (feedback-sexp :string))
(cffi:defcfun ("harmonia_signalograd_checkpoint" %signalograd-checkpoint) :int
  (path :string))
(cffi:defcfun ("harmonia_signalograd_restore" %signalograd-restore) :int
  (path :string))
(cffi:defcfun ("harmonia_signalograd_status" %signalograd-status) :pointer)
(cffi:defcfun ("harmonia_signalograd_snapshot" %signalograd-snapshot) :pointer)
(cffi:defcfun ("harmonia_signalograd_reset" %signalograd-reset) :int)
(cffi:defcfun ("harmonia_signalograd_last_error" %signalograd-last-error) :pointer)
(cffi:defcfun ("harmonia_signalograd_free_string" %signalograd-free-string) :void
  (ptr :pointer))

(defun %signalograd-read-string (ptr op)
  (if (cffi:null-pointer-p ptr)
      (error "signalograd ~A failed: ~A" op (signalograd-last-error))
      (unwind-protect
           (cffi:foreign-string-to-lisp ptr)
        (%signalograd-free-string ptr))))

(defun signalograd-last-error ()
  (let ((ptr (%signalograd-last-error)))
    (if (cffi:null-pointer-p ptr)
        ""
        (unwind-protect
             (cffi:foreign-string-to-lisp ptr)
          (%signalograd-free-string ptr)))))

(defun signalograd-port-ready-p ()
  *signalograd-ready*)

(defun init-signalograd-port ()
  (ensure-cffi)
  (setf *signalograd-lib*
        (cffi:load-foreign-library (%release-lib-path "libharmonia_signalograd.dylib")))
  (let ((rc (%signalograd-init)))
    (unless (zerop rc)
      (error "signalograd init failed: ~A" (signalograd-last-error)))
    (setf *signalograd-ready* t)
    (runtime-log *runtime* :signalograd-init
                 (list :version (%signalograd-version) :status :ok))
    t))

(defun signalograd-observe (observation-sexp)
  (let ((rc (%signalograd-observe observation-sexp)))
    (unless (zerop rc)
      (error "signalograd observe failed: ~A" (signalograd-last-error)))
    t))

(defun signalograd-reflect (observation)
  (signalograd-observe observation))

(defun signalograd-feedback (feedback-sexp)
  (let ((rc (%signalograd-feedback feedback-sexp)))
    (unless (zerop rc)
      (error "signalograd feedback failed: ~A" (signalograd-last-error)))
    t))

(defun signalograd-checkpoint (path)
  (let ((rc (%signalograd-checkpoint path)))
    (unless (zerop rc)
      (error "signalograd checkpoint failed: ~A" (signalograd-last-error)))
    t))

(defun signalograd-restore (path)
  (let ((rc (%signalograd-restore path)))
    (unless (zerop rc)
      (error "signalograd restore failed: ~A" (signalograd-last-error)))
    t))

(defun signalograd-status ()
  (let ((text (%signalograd-read-string (%signalograd-status) "status")))
    (let ((*read-eval* nil))
      (read-from-string text))))

(defun signalograd-snapshot ()
  (let ((text (%signalograd-read-string (%signalograd-snapshot) "snapshot")))
    (let ((*read-eval* nil))
      (read-from-string text))))

(defun signalograd-reset ()
  (let ((rc (%signalograd-reset)))
    (unless (zerop rc)
      (error "signalograd reset failed: ~A" (signalograd-last-error)))
    t))
