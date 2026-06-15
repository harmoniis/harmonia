;;;; lambdoma-probe.lisp — the agent never chooses from infinite possibilities; it
;;;; chooses from a BOUNDED, harmonically-ordered set (the lambdoma matrix). Proves the
;;;; canonical %lambdoma-select: relevance is the dominant axis (top is NEVER sacrificed),
;;;; the remainder is organized by FIELD RESONANCE (reused concept-edge graph), and the
;;;; set is bounded. Pure — no runtime needed.
;;;;   sbcl --load src/core/boot.lisp --load scripts/dev/lambdoma-probe.lisp \
;;;;        --eval '(harmonia::lambdoma-probe-run)'

(in-package :harmonia)

(defun lambdoma-probe-run ()
  (let ((pass 0) (fail 0))
    (labels ((g (label ok)
               (if ok (progn (incf pass) (format t "  [PASS] ~A~%" label))
                   (progn (incf fail) (format t "  [FAIL] ~A~%" label)))))
      (format t "~%════════ HARMONIA LAMBDOMA PROBE ════════~%")
      (clrhash *memory-concept-edges*)
      (let* ((e1 (make-memory-entry :id "e1" :content "alpha beta gamma delta" :time 100))
             (e2 (make-memory-entry :id "e2" :content "epsilon zeta" :time 100))
             (e3 (make-memory-entry :id "e3" :content "kappa lambda" :time 100))
             ;; relevance order as delivered by %rank-memory-entries: e1, e2, e3
             (cands (list e1 e2 e3)))

        ;; ── A. BOUNDED, relevance-preserving with no resonance ──
        (format t "~%── A. bounded + relevance dominant ──~%")
        (let ((sel (%lambdoma-select "alpha thing" cands :k 3)))
          (g "never an unbounded list — bounded to k" (<= (length sel) 3))
          (g "top relevance preserved (e1 stays #1) with zero resonance" (eq (first sel) e1)))
        (g "smaller k truncates the matrix" (= (length (%lambdoma-select "alpha thing" cands :k 2)) 2))

        ;; ── B. HARMONY: field resonance organizes the remainder ──
        (format t "~%── B. harmonic resonance from the field graph ──~%")
        ;; Learn an edge: query concept "alpha" resonates with e3's concept "kappa".
        (dotimes (i 6) (%upsert-concept-edge "alpha" "kappa" :cooccur))
        (g "concept edge learned (resonance source > 0)"
           (> (%concept-edge-weight "alpha" "kappa") 0.0))
        (g "%lambdoma-resonance is higher for the field-connected entry (e3 > e2)"
           (> (%lambdoma-resonance (list "alpha") e3) (%lambdoma-resonance (list "alpha") e2)))
        (let ((sel (%lambdoma-select "alpha thing" cands :k 3)))
          (g "top relevance STILL preserved after resonance (e1 #1)" (eq (first sel) e1))
          (g "resonant e3 lifted above non-resonant e2 in the harmonic tail"
             (and (member e3 sel) (member e2 sel)
                  (< (position e3 sel) (position e2 sel)))))

        ;; ── C. degenerate inputs are safe ──
        (format t "~%── C. safety ──~%")
        (g "empty candidates → empty" (null (%lambdoma-select "x" '() :k 5)))
        (g "single candidate → itself" (equal (%lambdoma-select "x" (list e1) :k 5) (list e1))))

      (format t "~%════════ LAMBDOMA PROBE: ~A pass / ~A fail ════════~%" pass fail)
      (sb-ext:exit :code (if (zerop fail) 0 1)))))
