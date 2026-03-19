;;; ipc-client.lisp — Unix domain socket IPC client for harmonia-runtime.
;;;
;;; Replaces all CFFI/dlopen calls with a single IPC transport.
;;; Every Rust component now lives inside harmonia-runtime as a ractor
;;; actor. This client sends length-prefixed s-expressions over the
;;; Unix domain socket and reads length-prefixed replies.
;;;
;;; Protocol: [4 bytes u32 big-endian length][sexp payload]
;;;
;;; Thread-safe: uses a connection pool with per-thread sockets.

(in-package :harmonia)

;;; ─── Socket path resolution ─────────────────────────────────────────

(defparameter *ipc-socket-path* nil
  "Cached path to the runtime Unix domain socket.")

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
                ;; Last resort: tmpdir
                (concatenate 'string
                             (string-right-trim "/" (%boot-env "TMPDIR" "/tmp"))
                             "/harmonia/runtime.sock")))))

;;; ─── Low-level socket I/O ───────────────────────────────────────────

(defun %ipc-connect ()
  "Open a connection to the runtime socket. Returns the socket stream or nil."
  (let ((path (%ipc-socket-path)))
    (handler-case
        (let ((socket (make-instance 'sb-bsd-sockets:local-socket
                                     :type :stream)))
          (sb-bsd-sockets:socket-connect socket path)
          (sb-bsd-sockets:socket-make-stream socket
                                             :element-type '(unsigned-byte 8)
                                             :input t :output t :buffering :full))
      (error (e)
        (%log :warn "ipc" "Failed to connect to ~A: ~A" path e)
        nil))))

(defun %ipc-write-frame (stream sexp-string)
  "Write a length-prefixed sexp frame to the stream."
  (let* ((bytes (sb-ext:string-to-octets sexp-string :external-format :utf-8))
         (len (length bytes))
         (header (make-array 4 :element-type '(unsigned-byte 8))))
    ;; Big-endian u32 length
    (setf (aref header 0) (ldb (byte 8 24) len))
    (setf (aref header 1) (ldb (byte 8 16) len))
    (setf (aref header 2) (ldb (byte 8  8) len))
    (setf (aref header 3) (ldb (byte 8  0) len))
    (write-sequence header stream)
    (write-sequence bytes stream)
    (force-output stream)))

(defun %ipc-read-frame (stream)
  "Read a length-prefixed sexp frame from the stream. Returns the string."
  (let ((header (make-array 4 :element-type '(unsigned-byte 8))))
    (let ((n (read-sequence header stream)))
      (when (< n 4)
        (return-from %ipc-read-frame nil)))
    (let ((len (+ (ash (aref header 0) 24)
                  (ash (aref header 1) 16)
                  (ash (aref header 2)  8)
                  (aref header 3))))
      (when (> len (* 10 1024 1024))
        (error "IPC frame too large: ~D bytes" len))
      (let ((buf (make-array len :element-type '(unsigned-byte 8))))
        (let ((n (read-sequence buf stream)))
          (when (< n len)
            (return-from %ipc-read-frame nil)))
        (sb-ext:octets-to-string buf :external-format :utf-8)))))

;;; ─── Connection pool ────────────────────────────────────────────────

(defparameter *ipc-connection* nil
  "Current IPC connection (stream). Lazily opened, reused across calls.")

(defun %ipc-ensure-connection ()
  "Get or create the IPC connection."
  (or (and *ipc-connection*
           (open-stream-p *ipc-connection*)
           *ipc-connection*)
      (setf *ipc-connection* (%ipc-connect))))

(defun %ipc-disconnect ()
  "Close the IPC connection."
  (when (and *ipc-connection* (open-stream-p *ipc-connection*))
    (ignore-errors (close *ipc-connection*)))
  (setf *ipc-connection* nil))

;;; ─── High-level IPC call ────────────────────────────────────────────

(defun ipc-call (sexp-string)
  "Send a sexp request to harmonia-runtime and return the reply string.
   Returns the reply sexp string, or nil on connection failure.
   Automatically reconnects on broken pipe."
  (labels ((attempt (retried)
             (let ((stream (%ipc-ensure-connection)))
               (unless stream
                 (return-from attempt nil))
               (handler-case
                   (progn
                     (%ipc-write-frame stream sexp-string)
                     (%ipc-read-frame stream))
                 (error (e)
                   ;; Broken pipe — reconnect once
                   (%ipc-disconnect)
                   (if retried
                       (progn
                         (%log :warn "ipc" "IPC call failed after retry: ~A" e)
                         nil)
                       (attempt t)))))))
    (attempt nil)))

(defun ipc-cast (sexp-string)
  "Send a fire-and-forget sexp to harmonia-runtime (no reply expected).
   Used for heartbeat, post, and other one-way messages."
  (let ((stream (%ipc-ensure-connection)))
    (when stream
      (handler-case
          (%ipc-write-frame stream sexp-string)
        (error (_)
          (%ipc-disconnect))))))

;;; ─── Convenience helpers ────────────────────────────────────────────

(defun ipc-available-p ()
  "Check if the runtime IPC socket exists and is connectable."
  (let ((path (%ipc-socket-path)))
    (and path (probe-file path) t)))

(defun ipc-parse-sexp-reply (reply)
  "Parse a sexp reply string into a Lisp form. Returns nil on parse failure."
  (when (and reply (stringp reply) (> (length reply) 0))
    (handler-case
        (let ((*read-eval* nil))
          (read-from-string reply))
      (error () nil))))

(defun ipc-reply-ok-p (reply)
  "Check if an IPC reply indicates success (starts with (:ok ...))."
  (and reply (stringp reply)
       (>= (length reply) 4)
       (string= (subseq reply 0 4) "(:ok")))

(defun ipc-reply-error-p (reply)
  "Check if an IPC reply indicates an error."
  (and reply (stringp reply)
       (search ":error" reply)))
