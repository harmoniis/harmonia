;;; scorer.lisp — Harmonic scoring helpers.

(in-package :harmonia)

(defun harmonic-score (prompt response)
  "Cheap harmony proxy: shorter coherent responses with non-empty content win."
  (let* ((p-len (max 1 (length prompt)))
         (r-len (length response))
         (density (/ (min r-len 1200) (float p-len)))
         (bounded (min 1.0 (/ density 12.0))))
    (max 0.0 bounded)))
