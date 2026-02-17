;;; store.lisp — Minimal memory substrate (evolvable).

(in-package :harmonia)

(defparameter *memory-store* (make-hash-table :test 'equal))
(defparameter *memory-seq* 0)

(defun memory-put (type content)
  (incf *memory-seq*)
  (let ((id (format nil "~A-~A-~A" type (get-universal-time) *memory-seq*)))
    (setf (gethash id *memory-store*)
          (list :id id :type type :content content :time (get-universal-time)))
    id))

(defun memory-recent (&key (limit 5))
  (let ((values '()))
    (maphash (lambda (_ entry)
               (declare (ignore _))
               (push entry values))
             *memory-store*)
    (subseq (sort values #'> :key (lambda (entry) (getf entry :time)))
            0
            (min limit (length values)))))

(defun memory-reset ()
  (setf *memory-store* (make-hash-table :test 'equal))
  (setf *memory-seq* 0)
  t)
