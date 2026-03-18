;;; harmony-policy.lisp — Runtime-loadable constants for harmonic evolution.

(in-package :harmonia)

(defparameter *harmony-policy-config-path*
  (merge-pathnames "../../config/harmony-policy.sexp" *boot-file*))
(defparameter *harmony-policy-state-path* nil)

(defun %harmony-policy-resolve-state-path ()
  "Resolve state path lazily (config-store not available at load time)."
  (or *harmony-policy-state-path*
      (setf *harmony-policy-state-path*
            (or (and (fboundp 'config-get-for)
                     (config-get-for "harmony-policy" "path"))
                (let ((root (or (and (fboundp 'config-get-for)
                                     (config-get-for "harmony-policy" "state-root" "global"))
                                (%tmpdir-state-root))))
                  (concatenate 'string root "/harmony-policy.sexp"))))))
(defparameter *harmony-policy* '())

(defun %harmony-policy-read-file (path)
  (with-open-file (in path :direction :input)
    (let ((*read-eval* nil))
      (read in nil nil))))

(defun harmony-policy-load ()
  (let* ((state-path (%harmony-policy-resolve-state-path))
         (src (cond
                ((probe-file state-path)
                 (%harmony-policy-read-file state-path))
                ((probe-file *harmony-policy-config-path*)
                 (%harmony-policy-read-file *harmony-policy-config-path*))
                (t
                 (error "harmony policy config missing: ~A" *harmony-policy-config-path*)))))
    (setf *harmony-policy* (copy-tree src))
    *harmony-policy*))

(defun harmony-policy-save ()
  (let ((state-path (%harmony-policy-resolve-state-path)))
    (ensure-directories-exist state-path)
    (with-open-file (out state-path :direction :output :if-exists :supersede :if-does-not-exist :create)
      (let ((*print-pretty* t))
        (prin1 *harmony-policy* out)
        (terpri out)))
    state-path))

(defun harmony-policy-get ()
  (or *harmony-policy* (harmony-policy-load)))

(defun %harmony-path-keys (path)
  (let* ((parts (loop with out = '()
                      with start = 0
                      for i = (position #\/ path :start start)
                      do (push (subseq path start (or i (length path))) out)
                      if (null i) do (return (nreverse out))
                      do (setf start (1+ i)))))
    (mapcar (lambda (p) (intern (string-upcase p) :keyword)) parts)))

(defun harmony-policy-ref (path &optional default)
  (let ((node (harmony-policy-get)))
    (dolist (k (%harmony-path-keys path) node)
      (setf node (if (listp node) (getf node k) nil))
      (when (null node)
        (return default)))))

(defun harmony-policy-number (path default)
  (let ((v (harmony-policy-ref path default)))
    (if (numberp v) (float v) (float default))))

(defun %harmony-policy-set-in-plist (plist keys value)
  (if (null (rest keys))
      (progn
        (setf (getf plist (first keys)) value)
        plist)
      (let* ((k (first keys))
             (cur (or (getf plist k) '())))
        (unless (listp cur) (setf cur '()))
        (setf (getf plist k)
              (%harmony-policy-set-in-plist cur (rest keys) value))
        plist)))

(defun harmony-policy-set (path value &key (persist t))
  (setf *harmony-policy*
        (%harmony-policy-set-in-plist (copy-tree (harmony-policy-get))
                                      (%harmony-path-keys path)
                                      value))
  (when persist
    (harmony-policy-save))
  *harmony-policy*)
