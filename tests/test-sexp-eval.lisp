;;; test-sexp-eval.lisp — Deterministic tests for the restricted Lisp evaluator.
;;;
;;; These tests run WITHOUT an LLM — they test the evaluator directly
;;; with mocked inputs. Every assumption is validated.

;; Simulate the harmonia package environment for standalone testing.
;; In production, this file would be loaded into the running SBCL.

(in-package :harmonia)

;;; ═══════════════════════════════════════════════════════════════════════
;;; TEST FRAMEWORK (minimal, no dependencies)
;;; ═══════════════════════════════════════════════════════════════════════

(defvar *test-pass* 0)
(defvar *test-fail* 0)

(defmacro test-assert (name expr)
  `(handler-case
       (if ,expr
           (progn (incf *test-pass*)
                  (format t "  ✓ ~A~%" ,name))
           (progn (incf *test-fail*)
                  (format t "  ✗ ~A — assertion failed~%" ,name)))
     (error (c)
       (incf *test-fail*)
       (format t "  ✗ ~A — error: ~A~%" ,name c))))

(defun run-sexp-eval-tests ()
  (setf *test-pass* 0 *test-fail* 0)
  (format t "~%═══ SEXP-EVAL TESTS ═══════════════════════════════════~%")

  ;; ── Basic evaluation ──────────────────────────────────────────────
  (format t "~%── Atoms ──~%")
  (test-assert "nil evaluates to nil"
    (null (%reval nil '())))
  (test-assert "t evaluates to t"
    (eq t (%reval t '())))
  (test-assert "number evaluates to itself"
    (= 42 (%reval 42 '())))
  (test-assert "string evaluates to itself"
    (string= "hello" (%reval "hello" '())))
  (test-assert "keyword evaluates to itself"
    (eq :foo (%reval :foo '())))

  ;; ── Variable lookup ───────────────────────────────────────────────
  (format t "~%── Variables ──~%")
  (test-assert "bound variable resolves"
    (= 99 (%reval 'x '((x . 99)))))
  (test-assert "unbound variable signals error"
    (handler-case (progn (%reval 'unbound '()) nil)
      (error () t)))

  ;; ── Let ───────────────────────────────────────────────────────────
  (format t "~%── Let ──~%")
  (test-assert "let binds and evaluates body"
    (= 10 (%reval '(let ((x 10)) x) '())))
  (test-assert "let multiple bindings"
    (= 30 (%reval '(let ((a 10) (b 20)) (+ a b)) '())))
  (test-assert "let nested"
    (= 15 (%reval '(let ((x 5)) (let ((y 10)) (+ x y))) '())))

  ;; ── If / When / Unless ────────────────────────────────────────────
  (format t "~%── Conditionals ──~%")
  (test-assert "if true branch"
    (= 1 (%reval '(if t 1 2) '())))
  (test-assert "if false branch"
    (= 2 (%reval '(if nil 1 2) '())))
  (test-assert "when true"
    (= 42 (%reval '(when t 42) '())))
  (test-assert "when false"
    (null (%reval '(when nil 42) '())))
  (test-assert "unless false"
    (= 42 (%reval '(unless nil 42) '())))

  ;; ── Quote ─────────────────────────────────────────────────────────
  (format t "~%── Quote ──~%")
  (test-assert "quote returns form literally"
    (equal '(a b c) (%reval '(quote (a b c)) '())))

  ;; ── Arithmetic / Comparison ───────────────────────────────────────
  (format t "~%── Comparison ──~%")
  (test-assert "> works"
    (eq t (%reval '(> 5 3) '())))
  (test-assert "< works"
    (eq t (%reval '(< 3 5) '())))
  (test-assert "= works"
    (eq t (%reval '(= 7 7) '())))
  (test-assert "not works"
    (eq t (%reval '(not nil) '())))

  ;; ── String operations ─────────────────────────────────────────────
  (format t "~%── Strings ──~%")
  (test-assert "format works"
    (string= "hello world" (%reval '(format "~A ~A" "hello" "world") '())))
  (test-assert "length works"
    (= 5 (%reval '(length "hello") '())))
  (test-assert "concatenate works"
    (string= "ab" (%reval '(concatenate "a" "b") '())))
  (test-assert "string-downcase works"
    (string= "hello" (%reval '(string-downcase "HELLO") '())))

  ;; ── List operations ───────────────────────────────────────────────
  (format t "~%── Lists ──~%")
  (test-assert "list creates list"
    (equal '(1 2 3) (%reval '(list 1 2 3) '())))
  (test-assert "first extracts first"
    (= 1 (%reval '(first (list 1 2 3)) '())))
  (test-assert "getf extracts property"
    (= 42 (%reval '(getf (list :a 42 :b 99) :a) '())))

  ;; ── Respond throws ────────────────────────────────────────────────
  (format t "~%── Respond ──~%")
  (test-assert "respond throws repl-respond"
    (string= "hello user"
             (catch 'repl-respond
               (%reval '(respond "hello user") '()))))
  (test-assert "respond from let binding"
    (string= "x is 42"
             (catch 'repl-respond
               (%reval '(let ((x 42))
                          (respond (format "x is ~A" x)))
                       '()))))

  ;; ── Composition: let + if + respond ───────────────────────────────
  (format t "~%── Composition ──~%")
  (test-assert "composed let + if + respond"
    (string= "big"
             (catch 'repl-respond
               (%reval '(let ((x 100))
                          (if (> x 50)
                              (respond "big")
                              (respond "small")))
                       '()))))

  ;; ── Detection: is-sexp-output-p ───────────────────────────────────
  (format t "~%── Detection ──~%")
  (test-assert "detects (recall ...)"
    (%is-sexp-output-p "(recall \"hello\")"))
  (test-assert "detects (let ...)"
    (%is-sexp-output-p "(let ((x 1)) x)"))
  (test-assert "detects (respond ...)"
    (%is-sexp-output-p "(respond \"hello\")"))
  (test-assert "rejects plain text"
    (not (%is-sexp-output-p "Hello, I am Harmonia")))
  (test-assert "rejects (I think...)"
    (not (%is-sexp-output-p "(I think this is great)")))
  (test-assert "rejects empty"
    (not (%is-sexp-output-p "")))
  (test-assert "rejects nil"
    (not (%is-sexp-output-p nil)))

  ;; ── Eval all forms ────────────────────────────────────────────────
  (format t "~%── Eval all forms ──~%")
  (test-assert "eval-all-forms handles single form"
    (let ((result (%eval-all-forms "(format \"hello\")")))
      (search "hello" result)))
  (test-assert "eval-all-forms handles multiple forms"
    (let ((result (%eval-all-forms "(format \"a\") (format \"b\")")))
      (and (search "a" result) (search "b" result))))

  ;; ── Denied operations ─────────────────────────────────────────────
  (format t "~%── Safety ──~%")
  (test-assert "eval is denied"
    (handler-case
        (progn (%reval '(eval '(+ 1 2)) '()) nil)
      (error () t)))
  (test-assert "load is denied"
    (handler-case
        (progn (%reval '(load "/etc/passwd") '()) nil)
      (error () t)))
  (test-assert "defun is denied"
    (handler-case
        (progn (%reval '(defun foo () 42) '()) nil)
      (error () t)))
  (test-assert "setf is denied"
    (handler-case
        (progn (%reval '(setf x 42) '()) nil)
      (error () t)))

  ;; ── Results ───────────────────────────────────────────────────────
  (format t "~%═══ RESULTS: ~D passed, ~D failed ═══════════════════~%"
          *test-pass* *test-fail*)
  (= *test-fail* 0))
