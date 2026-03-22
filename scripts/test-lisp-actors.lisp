;;; test-lisp-actors.lisp — Unit tests for the Harmonia actor system.

(in-package :harmonia)

(defvar *test-pass* 0)
(defvar *test-fail* 0)

(defun %test (name result)
  (if result
      (progn (incf *test-pass*)
             (format t "  PASS  ~A~%" name))
      (progn (incf *test-fail*)
             (format t "  FAIL  ~A~%" name))))

(format t "~%=== Harmonia Actor System Tests ===~%~%")

;;; ─── Test 1: Runtime start ────────────────────────────────────────
(let ((rt (start :run-loop nil)))
  (%test "runtime starts" (not (null rt)))
  (%test "tools registered" (> (hash-table-count (runtime-state-tools rt)) 0))
  (%test "cycle is 0" (= (runtime-state-cycle rt) 0))

  ;;; ─── Test 2: Sequential tick ──────────────────────────────────────
  (tick :runtime rt)
  (%test "tick increments cycle" (= (runtime-state-cycle rt) 1))
  (tick :runtime rt)
  (%test "second tick" (= (runtime-state-cycle rt) 2))

  ;;; ─── Test 3: Actor creation and ask ───────────────────────────────
  (let ((echo (make-actor "test-echo"
               (lambda (msg state)
                 (when (actor-message-reply msg)
                   (actor-reply msg (actor-message-payload msg)))
                 state)
               :state nil)))
    (%test "actor alive" (actor-alive-p echo))
    (let ((reply (ask echo :echo "hello" 3)))
      (%test "ask returns payload" (equal reply "hello")))
    (let ((reply (ask echo :echo 42 3)))
      (%test "ask returns number" (eql reply 42)))

    ;;; ─── Test 4: Actor tell (fire-and-forget) ────────────────────────
    (%test "tell returns t" (tell echo :fire "bang"))
    (sleep 0.1)

    ;;; ─── Test 5: Actor stop ──────────────────────────────────────────
    (stop-actor echo)
    (sleep 0.5)
    (%test "actor stopped" (not (actor-alive-p echo))))

  ;;; ─── Test 6: Actor with state ────────────────────────────────────
  (let ((counter (make-actor "test-counter"
                  (lambda (msg state)
                    (case (actor-message-tag msg)
                      (:inc
                       (let ((new (1+ state)))
                         (when (actor-message-reply msg)
                           (actor-reply msg new))
                         new))
                      (:get
                       (when (actor-message-reply msg)
                         (actor-reply msg state))
                       state)
                      (t state)))
                  :state 0)))
    (ask counter :inc nil 3)
    (ask counter :inc nil 3)
    (ask counter :inc nil 3)
    (let ((val (ask counter :get nil 3)))
      (%test "actor state management" (eql val 3)))
    (stop-actor counter)
    (sleep 0.3))

  ;;; ─── Test 7: Actor error handling ────────────────────────────────
  (let ((error-count 0))
    (let ((faulty (make-actor "test-faulty"
                   (lambda (msg state)
                     (declare (ignore state))
                     (when (eq (actor-message-tag msg) :crash)
                       (error "intentional crash"))
                     (when (actor-message-reply msg)
                       (actor-reply msg :ok))
                     nil)
                   :error-fn (lambda (c msg state)
                               (declare (ignore c msg))
                               (incf error-count)
                               state))))
      (tell faulty :crash)
      (sleep 0.2)
      (%test "error handler called" (= error-count 1))
      ;; Actor should still be alive after error
      (%test "actor survives error" (actor-alive-p faulty))
      (let ((reply (ask faulty :ping nil 3)))
        (%test "actor responds after error" (eq reply :ok)))
      (stop-actor faulty)
      (sleep 0.3)))

  ;;; ─── Test 8: Actor system ────────────────────────────────────────
  (let ((sys (make-actor-system)))
    (system-register sys "a"
      (make-actor "sys-a"
        (lambda (msg state)
          (when (actor-message-reply msg)
            (actor-reply msg (1+ state)))
          (1+ state))
        :state 0))
    (system-register sys "b"
      (make-actor "sys-b"
        (lambda (msg state)
          (when (actor-message-reply msg)
            (actor-reply msg (* state 2)))
          (* state 2))
        :state 1))
    (%test "system lookup a" (not (null (system-actor sys "a"))))
    (%test "system lookup b" (not (null (system-actor sys "b"))))
    (%test "system lookup nil" (null (system-actor sys "x")))
    (let ((ra (ask (system-actor sys "a") :tick nil 3))
          (rb (ask (system-actor sys "b") :tick nil 3)))
      (%test "system actor a responds" (eql ra 1))
      (%test "system actor b responds" (eql rb 2)))
    (shutdown-system sys)
    (sleep 0.5)
    (%test "system shutdown" t))

  ;;; ─── Test 9: Timer ───────────────────────────────────────────────
  (let* ((count 0)
         (timed (make-actor "test-timed"
                  (lambda (msg state)
                    (declare (ignore msg))
                    (incf count)
                    state)
                  :state nil))
         (stop-fn (start-timer timed :tick 0.1)))
    (sleep 0.55)
    (funcall stop-fn)
    (sleep 0.1)
    (%test "timer fires repeatedly" (>= count 4))
    (let ((count-after count))
      (sleep 0.3)
      (%test "timer stops" (= count count-after)))
    (stop-actor timed)
    (sleep 0.3))

  ;;; ─── Test 10: Thread-safe queue ──────────────────────────────────
  (let ((rt2 (make-runtime-state)))
    (%queue-push rt2 "a")
    (%queue-push rt2 "b")
    (%queue-push rt2 "c")
    (%test "queue pop order" (equal (%queue-pop rt2) "a"))
    (%test "queue pop second" (equal (%queue-pop rt2) "b"))
    (%test "queue pop third" (equal (%queue-pop rt2) "c"))
    (%test "queue empty" (null (%queue-pop rt2))))

  ;;; ─── Test 11: Thread-safe outbound queue ─────────────────────────
  (%outbound-push (list :x 1))
  (%outbound-push (list :y 2))
  (let ((batch (%outbound-drain)))
    (%test "outbound drain length" (= (length batch) 2))
    (%test "outbound drain empties" (null (%outbound-drain))))

  ;;; ─── Test 12: IPC still works ────────────────────────────────────
  (let ((reply (ipc-call "(:list)")))
    (%test "IPC call returns" (not (null reply)))
    (%test "IPC reply is sexp" (and reply (> (length reply) 0))))

  (stop rt))

;;; ─── Summary ──────────────────────────────────────────────────────
(format t "~%=== Results: ~D passed, ~D failed ===~%~%" *test-pass* *test-fail*)
(sb-ext:exit :code (if (= *test-fail* 0) 0 1))
