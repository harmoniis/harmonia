;;; rewrite.lisp — Controlled self-rewrite hooks.

(in-package :harmonia)

(defun maybe-self-rewrite (prompt response)
  "Trigger minimal rewrite bookkeeping when explicitly requested.
   Real code mutation engine can replace this implementation later."
  (when (search "rewrite" (string-downcase prompt))
    (incf (runtime-state-rewrite-count *runtime*))
    (runtime-log *runtime* :rewrite-triggered
                 (list :count (runtime-state-rewrite-count *runtime*)
                       :hint (subseq response 0 (min 80 (length response)))))
    t))
