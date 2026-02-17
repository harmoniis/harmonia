;;; dna.lisp — Immutable alignment anchor.

(in-package :harmonia)

(defparameter *dna*
  '(:creator "harmoniis"
    :prime-directive "Seek harmony through minimal, composable orchestration."
    :laws (1 2 3 4 5 6 7 8)
    :immutable-files ("src/dna/dna.lisp")))

(defun dna-valid-p ()
  (and (equal (getf *dna* :creator) "harmoniis")
       (member 7 (getf *dna* :laws))
       (member 8 (getf *dna* :laws))))
