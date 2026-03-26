;;; ipc-client.lisp — Solid IPC client for harmonia-runtime.
;;;
;;; Architecture: fresh connection per call. No cached sockets.
;;;
;;; Unix domain sockets have negligible connection overhead (~0.1ms).
;;; A fresh connection per call eliminates ALL stale socket problems:
;;; - No broken pipe from SIGKILL (stale pool is gone)
;;; - No frame interleaving from parallel threads (each has its own socket)
;;; - No partial-read corruption from timeouts
;;; - No health-check needed (if connect succeeds, socket is alive)
;;;
;;; Protocol: [4 bytes u32 big-endian length][sexp payload]

(in-package :harmonia)

(require :sb-bsd-sockets)

;;; ─── Socket path ──────────────────────────────────────────────────────

(defparameter *ipc-socket-path* nil)

(defun %ipc-socket-path ()
  "Resolve the runtime socket path."
  (or *ipc-socket-path*
      (setf *ipc-socket-path*
            (or (%boot-env "HARMONIA_RUNTIME_SOCKET")
                (let ((state (or (%boot-env "HARMONIA_STATE_ROOT")
                                 (let ((home (%boot-env "HOME")))
                                   (when home
                                     (concatenate 'string home "/.harmoniis/harmonia"))))))
                  (when state
                    (concatenate 'string state "/runtime.sock")))
                (concatenate 'string
                             (string-right-trim "/" (%boot-env "TMPDIR" "/tmp"))
                             "/harmonia/runtime.sock")))))

;;; ─── Connection: fresh per call ───────────────────────────────────────

(defun %ipc-connect ()
  "Open a fresh connection. Returns stream or nil. Never caches."
  (let ((path (%ipc-socket-path)))
    (unless (and path (probe-file path))
      (return-from %ipc-connect nil))
    (handler-case
        (let ((socket (make-instance 'sb-bsd-sockets:local-socket :type :stream)))
          (sb-bsd-sockets:socket-connect socket path)
          (sb-bsd-sockets:socket-make-stream socket
                                             :element-type '(unsigned-byte 8)
                                             :input t :output t :buffering :full))
      (error (e)
        (%log :warn "ipc" "Connect failed: ~A" e)
        nil))))

;;; ─── Frame I/O ────────────────────────────────────────────────────────

(defun %ipc-write-frame (stream sexp-string)
  "Write a length-prefixed sexp frame."
  (let* ((bytes (sb-ext:string-to-octets sexp-string :external-format :utf-8))
         (len (length bytes))
         (header (make-array 4 :element-type '(unsigned-byte 8))))
    (setf (aref header 0) (ldb (byte 8 24) len))
    (setf (aref header 1) (ldb (byte 8 16) len))
    (setf (aref header 2) (ldb (byte 8  8) len))
    (setf (aref header 3) (ldb (byte 8  0) len))
    (write-sequence header stream)
    (write-sequence bytes stream)
    (force-output stream)))

(defun %ipc-read-frame (stream)
  "Read a length-prefixed sexp frame. Signals error on protocol violations."
  (let ((header (make-array 4 :element-type '(unsigned-byte 8))))
    (let ((n (read-sequence header stream)))
      (when (< n 4)
        (error "IPC: short header (~D/4 bytes)" n)))
    (let ((len (+ (ash (aref header 0) 24)
                  (ash (aref header 1) 16)
                  (ash (aref header 2)  8)
                  (aref header 3))))
      (when (> len (* 10 1024 1024))
        (error "IPC: frame too large (~D bytes)" len))
      (let ((buf (make-array len :element-type '(unsigned-byte 8))))
        (let ((n (read-sequence buf stream)))
          (when (< n len)
            (error "IPC: short body (~D/~D bytes)" n len)))
        (sb-ext:octets-to-string buf :external-format :utf-8)))))

;;; ─── High-level IPC call ──────────────────────────────────────────────

(defparameter *ipc-call-timeout-seconds* 90
  "Maximum seconds for an IPC call. High because LLM calls can be slow.
User interrupts via ESC, not timeout.")

(defun ipc-call (sexp-string)
  "Send sexp to harmonia-runtime, return reply. Fresh connection per call.
Returns reply string or nil. Retries once on failure. Thread-safe."
  (handler-case
      (sb-sys:with-deadline (:seconds *ipc-call-timeout-seconds*)
        (labels ((attempt (retried)
                   (let ((stream (%ipc-connect)))
                     (unless stream
                       (return-from attempt nil))
                     (handler-case
                         (unwind-protect
                             (progn
                               (%ipc-write-frame stream sexp-string)
                               (%ipc-read-frame stream))
                           ;; Always close — never reuse.
                           (ignore-errors (close stream)))
                       (error (e)
                         (ignore-errors (close stream))
                         (if retried
                             (progn
                               (%log :warn "ipc" "Failed after retry: ~A" e)
                               nil)
                             (attempt t)))))))
          (attempt nil)))
    (sb-sys:deadline-timeout ()
      (%log :warn "ipc" "Timeout (~Ds): ~A"
            *ipc-call-timeout-seconds*
            (subseq sexp-string 0 (min 80 (length sexp-string))))
      nil)))

(defun ipc-cast (sexp-string)
  "Fire-and-forget sexp to runtime. No reply expected."
  (handler-case
      (let ((stream (%ipc-connect)))
        (when stream
          (unwind-protect
              (%ipc-write-frame stream sexp-string)
            (ignore-errors (close stream)))))
    (error () nil)))

;;; ─── Convenience ──────────────────────────────────────────────────────

(defun ipc-available-p ()
  "Check if the runtime socket exists."
  (let ((path (%ipc-socket-path)))
    (and path (probe-file path) t)))

(defun ipc-parse-sexp-reply (reply)
  "Parse sexp reply. Returns nil on failure."
  (when (and reply (stringp reply) (> (length reply) 0))
    (handler-case
        (let ((*read-eval* nil))
          (read-from-string reply))
      (error () nil))))

(defun ipc-reply-ok-p (reply)
  "Check if reply starts with (:ok ...)."
  (and reply (stringp reply)
       (>= (length reply) 4)
       (string= (subseq reply 0 4) "(:ok")))

(defun ipc-reply-error-p (reply)
  "Check if reply contains :error."
  (and reply (stringp reply)
       (search ":error" reply)))

(defun ipc-extract-value (reply)
  "Extract the :result value from an IPC reply string."
  (when (and reply (stringp reply))
    (let ((pos (search ":result " reply)))
      (when pos
        (let ((start (+ pos 8)))
          (if (and (< start (length reply))
                   (char= (char reply start) #\"))
              ;; Quoted string value
              (let ((end (position #\" reply :start (1+ start))))
                (when end
                  (subseq reply (1+ start) end)))
              ;; Unquoted value — take until closing paren
              (let ((end (position #\) reply :start start)))
                (when end
                  (string-trim '(#\Space) (subseq reply start end))))))))))

;;; ──��� Legacy compat (old pool functions, now no-ops) ───────────────────

(defun %ipc-ensure-connection ()
  "Legacy compat. Returns a fresh connection."
  (%ipc-connect))

(defun %ipc-disconnect ()
  "Legacy compat. No-op — connections are not pooled."
  nil)
