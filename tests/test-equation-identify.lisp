;;; test-equation-identify.lisp — Phase 8 self-rewrite loop, OFFLINE (no LLM/IPC/runtime).
;;;
;;; Exercises the REAL sexp-eval (%reval) + REPL math primitives — the agent evaluating its
;;; own extracted governing law. Run:
;;;   sbcl --noinform --non-interactive --disable-debugger \
;;;     --eval "(load #P\"src/core/boot.lisp\")" \
;;;     --eval "(load #P\"tests/test-equation-identify.lisp\")" \
;;;     --eval "(sb-ext:exit :code (harmonia::run-equation-identify-tests))"

(in-package :harmonia)

(defun %ei-tree-equal (a b &optional (tol 1.0e-4))
  "Structural equality on law s-exprs, tolerant on float coefficients."
  (cond ((and (numberp a) (numberp b)) (< (abs (- a b)) tol))
        ((and (consp a) (consp b)) (and (= (length a) (length b)) (every #'%ei-tree-equal a b)))
        (t (eql a b))))

(defun run-equation-identify-tests ()
  (let ((pass 0) (fail 0))
    (flet ((check (name got expected &key (tol 1.0e-4))
             (let ((ok (cond ((numberp expected) (and (numberp got) (< (abs (- got expected)) tol)))
                             ((consp expected) (%ei-tree-equal got expected tol))
                             (t (equal got expected)))))
               (format t "~:[FAIL~;PASS~]  ~A  => ~S~%" ok name got)
               (if ok (incf pass) (incf fail)))))
      ;; 1. identify a linear (Lorenz dx) law from Θ over a named basis
      (check "identify Lorenz dx law"
             (%identify-readout-law '(10.0 -10.0) '(y x))
             '(+ (* 10.0 y) (* -10.0 x)))
      ;; 2. the agent evals its OWN law through the real sexp-eval: 10*2 + -10*1 = 10
      (check "eval Lorenz dx @ x=1,y=2"
             (%eval-law '(+ (* 10.0 y) (* -10.0 x)) '((x . 1.0) (y . 2.0)))
             10.0)
      ;; 3. identify a TRANSCENDENTAL (Thomas) law: sin(y) - 0.19 x
      (check "identify Thomas dx law"
             (%identify-readout-law '(1.0 -0.19) '((sin y) x))
             '(+ (* 1.0 (sin y)) (* -0.19 x)))
      ;; 4. eval the transcendental law via the NEW sin primitive: sin(pi/2) - 0.19 = 0.81
      (check "eval Thomas dx @ x=1,y=pi/2"
             (%eval-law '(+ (* 1.0 (sin y)) (* -0.19 x)) (list (cons 'x 1.0) (cons 'y (/ pi 2))))
             0.81)
      ;; 5. occam / Kolmogorov-minimality: a sub-threshold term is dropped
      (check "occam drops sub-threshold term"
             (%identify-readout-law '(1.0 0.0001) '((sin y) x))
             '(* 1.0 (sin y)))
      ;; 6. closed loop, gate OPEN: extract -> eval -> bounded least-action descent
      (let ((r (harmonic-rewrite-descend
                 :gate-open t :coefficients '(1.0 -0.19) :basis-names '((sin y) x)
                 :bindings (list (cons 'x 1.0) (cons 'y (/ pi 2)))
                 :current 1.0 :lo 0.1 :hi 5.0 :rate 0.5)))
        (check "loop applied" (getf r :applied) t)
        (check "loop param stays bounded" (getf r :bounded) t)
        ;; target = clamp(|0.81|, 0.1, 5) = 0.81 ; new = 1.0 + 0.5*(0.81-1.0) = 0.905
        (check "loop param descends toward law value" (getf r :param-after) 0.905 :tol 1.0e-3)
        (check "loop reports a positive Solomonoff prior"
               (and (numberp (getf r :solomonoff-prior)) (> (getf r :solomonoff-prior) 0.0)) t))
      ;; 7. closed loop, gate CLOSED: never rewrites when the harmonic gate is shut
      (let ((r (harmonic-rewrite-descend :gate-open nil :current 1.0)))
        (check "gate closed -> not applied" (getf r :applied) nil)
        (check "gate closed -> param unchanged" (getf r :param) 1.0))
      (format t "~%── EQUATION-IDENTIFY: ~D pass / ~D fail ──~%" pass fail)
      (if (zerop fail) 0 1))))
