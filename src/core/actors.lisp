;;; actors.lisp — Lightweight actor system for Harmonia.
;;;
;;; Inspired by cl-gserver/Sento: message-driven concurrency where each
;;; actor owns its state, processes messages through a mailbox, and
;;; communicates via tell (fire-and-forget) and ask (request-reply).
;;;
;;; Built on SBCL threads + sb-concurrency mailboxes.
;;; No global mutable state. No blocking. No imperative loops.
;;;
;;; Actors are the atoms of the Harmonia runtime — indivisible units
;;; of computation that embody the homoiconic principle: code as data,
;;; state as message, behavior as function.

(in-package :harmonia)

(require :sb-concurrency)

;;; ─── Actor struct ──────────────────────────────────────────────────────

(defstruct (actor (:constructor %make-actor))
  "An actor: a thread with a mailbox and a receive function.
   The receive function is pure — it takes (message state) and returns new-state.
   Side effects happen inside the receive function, but the actor model
   guarantees single-threaded access to state."
  (name       ""    :type string)
  (mailbox    nil)
  (thread     nil)
  (receive-fn nil   :type (or function null))
  (state      nil)
  (running    t     :type boolean)
  (error-fn   nil   :type (or function null)))

;;; ─── Mailbox message wrapper ───────────────────────────────────────────

(defstruct actor-message
  "A message in an actor's mailbox."
  (tag     nil)                         ; keyword identifying message type
  (payload nil)                         ; message data
  (reply   nil :type (or null sb-concurrency:mailbox)))  ; reply mailbox for ask pattern

;;; ─── Actor creation ────────────────────────────────────────────────────

(defun make-actor (name receive-fn &key (state nil) (error-fn nil))
  "Create and start an actor.

   RECEIVE-FN: (lambda (message state) ...) → new-state
     Called for each message. Must return the new state.
     The message is an actor-message struct with tag, payload, and reply.
     To reply to an ask, call (actor-reply message value).

   STATE: initial actor state (any Lisp value).

   ERROR-FN: (lambda (condition message state) ...) → new-state
     Called when receive-fn signals an error. If nil, errors are logged
     and the actor continues with unchanged state."
  (let* ((mbox (sb-concurrency:make-mailbox :name name))
         (actor (%make-actor :name name
                             :mailbox mbox
                             :receive-fn receive-fn
                             :state state
                             :error-fn error-fn)))
    (setf (actor-thread actor)
          (sb-thread:make-thread
           (lambda () (%actor-loop actor))
           :name (format nil "harmonia-actor:~A" name)))
    actor))

(defun %actor-loop (actor)
  "Main loop for an actor thread. Processes messages from the mailbox
   until the actor is stopped. Each message is handled by the receive
   function, which returns the new state. Errors are caught and handled
   gracefully — the actor never crashes."
  (loop while (actor-running actor)
        do (let ((msg (sb-concurrency:receive-message
                       (actor-mailbox actor)
                       :timeout 0.5)))
             (when msg
               (handler-case
                   (let ((new-state (funcall (actor-receive-fn actor)
                                             msg
                                             (actor-state actor))))
                     (setf (actor-state actor) new-state))
                 (serious-condition (c)
                   (if (actor-error-fn actor)
                       (handler-case
                           (let ((new-state (funcall (actor-error-fn actor)
                                                     c msg (actor-state actor))))
                             (setf (actor-state actor) new-state))
                         (serious-condition (c2)
                           (%log :error (actor-name actor)
                                 "Error handler failed: ~A (original: ~A)"
                                 (ignore-errors (princ-to-string c2))
                                 (ignore-errors (princ-to-string c)))))
                       (%log :error (actor-name actor)
                             "Unhandled error: ~A"
                             (ignore-errors (princ-to-string c)))))))))
  (%log :info (actor-name actor) "Actor stopped."))

;;; ─── Message passing ───────────────────────────────────────────────────

(defun tell (actor tag &optional payload)
  "Send a fire-and-forget message to an actor.
   Returns immediately. The message is queued in the actor's mailbox."
  (when (and actor (actor-running actor))
    (sb-concurrency:send-message
     (actor-mailbox actor)
     (make-actor-message :tag tag :payload payload))
    t))

(defun ask (actor tag &optional payload (timeout 30))
  "Send a request to an actor and wait for the reply.
   Returns the reply value, or nil on timeout.
   TIMEOUT is in seconds."
  (when (and actor (actor-running actor))
    (let ((reply-mbox (sb-concurrency:make-mailbox :name "reply")))
      (sb-concurrency:send-message
       (actor-mailbox actor)
       (make-actor-message :tag tag :payload payload :reply reply-mbox))
      (sb-concurrency:receive-message reply-mbox :timeout timeout))))

(defun actor-reply (message value)
  "Reply to a message that expects a response (from an ask call).
   Call this from within a receive function."
  (when (actor-message-reply message)
    (sb-concurrency:send-message (actor-message-reply message) value)))

;;; ─── Actor lifecycle ───────────────────────────────────────────────────

(defun stop-actor (actor)
  "Gracefully stop an actor. Sends a :stop message and waits for the
   thread to finish."
  (when actor
    (setf (actor-running actor) nil)
    ;; Send a wake-up message so the mailbox receive unblocks
    (ignore-errors
      (sb-concurrency:send-message
       (actor-mailbox actor)
       (make-actor-message :tag :stop)))
    ;; Wait for thread to finish (with timeout)
    (when (and (actor-thread actor)
               (sb-thread:thread-alive-p (actor-thread actor)))
      (handler-case
          (sb-thread:join-thread (actor-thread actor) :timeout 5)
        (sb-thread:join-thread-error () nil)))
    t))

(defun actor-alive-p (actor)
  "Return T if the actor is running."
  (and actor
       (actor-running actor)
       (actor-thread actor)
       (sb-thread:thread-alive-p (actor-thread actor))))

;;; ─── Timer: periodic message sending ───────────────────────────────────

(defun start-timer (actor tag interval-seconds &optional payload)
  "Start a timer that periodically sends a message to an actor.
   Returns a timer-stop function: call it to cancel the timer."
  (let ((running t))
    (sb-thread:make-thread
     (lambda ()
       (loop while running
             do (sleep interval-seconds)
                (when running
                  (tell actor tag payload))))
     :name (format nil "timer:~A/~A" (actor-name actor) tag))
    (lambda () (setf running nil))))

;;; ─── Actor system: manages a set of named actors ───────────────────────

(defstruct (actor-system (:constructor %make-actor-system))
  "A system of named actors with coordinated lifecycle."
  (actors (make-hash-table :test 'equal) :type hash-table)
  (timers nil :type list))

(defun make-actor-system ()
  "Create a new actor system."
  (%make-actor-system))

(defun system-register (system name actor)
  "Register an actor in the system under NAME."
  (setf (gethash name (actor-system-actors system)) actor)
  actor)

(defun system-actor (system name)
  "Look up an actor by name."
  (gethash name (actor-system-actors system)))

(defun system-add-timer (system stop-fn)
  "Register a timer stop function for coordinated shutdown."
  (push stop-fn (actor-system-timers system)))

(defun shutdown-system (system)
  "Shut down all actors and timers in the system."
  ;; Stop timers first
  (dolist (stop-fn (actor-system-timers system))
    (ignore-errors (funcall stop-fn)))
  ;; Stop all actors
  (maphash (lambda (name actor)
             (declare (ignore name))
             (stop-actor actor))
           (actor-system-actors system))
  (clrhash (actor-system-actors system))
  t)
