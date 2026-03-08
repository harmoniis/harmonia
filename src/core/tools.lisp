;;; tools.lisp — Tool registry and status inspection.

(in-package :harmonia)

(defparameter *tools-config-path*
  (merge-pathnames "../../config/tools.sexp" *boot-file*))

(defun %load-tools-config ()
  (unless (probe-file *tools-config-path*)
    (error "tools config missing: ~A" *tools-config-path*))
  (with-open-file (in *tools-config-path* :direction :input)
    (let ((*read-eval* nil))
      (read in nil '()))))

(defun register-tool (runtime id path)
  (setf (gethash id (runtime-state-tools runtime))
        (list :id id :path path :status :registered :loaded-at nil)))

(defun register-default-tools (runtime)
  (dolist (tool (%load-tools-config))
    (register-tool runtime (car tool) (cdr tool)))
  runtime)

(defun tool-status (&optional (runtime *runtime*))
  (let ((out '()))
    (maphash (lambda (key value)
               (declare (ignore key))
               (push value out))
             (runtime-state-tools runtime))
    (sort out #'string< :key (lambda (item) (getf item :id)))))
