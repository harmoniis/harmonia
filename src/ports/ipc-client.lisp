;;; ipc-client.lisp — Cross-platform IPC client for harmonia-runtime.
;;;
;;; Architecture: fresh connection per call. No cached sockets.
;;;
;;; Transport: interprocess crate (GenericNamespaced)
;;;   Linux:       abstract namespace Unix socket
;;;   macOS/BSD:   /tmp/{name} filesystem Unix socket
;;;   Windows:     \\.\pipe\{name} named pipe (deferred)
;;;
;;; Security: 64-byte hex nonce token sent before first sexp frame.
;;;
;;; Protocol: [token:64 bytes][4 bytes u32 BE length][sexp payload]

(in-package :harmonia)

(require :sb-bsd-sockets)

;;; ─── FNV-1a 64-bit hash ────────────────────────────────────────────
;;; Deterministic hash matching Rust's ipc::fnv1a_64.

(defun %fnv1a-64 (string)
  "FNV-1a 64-bit hash of a UTF-8 string. Matches Rust implementation."
  (let ((hash #xCBF29CE484222325))
    (loop for byte across (sb-ext:string-to-octets string :external-format :utf-8)
          do (setf hash (logxor hash byte))
             (setf hash (logand #xFFFFFFFFFFFFFFFF
                                (* hash #x100000001B3))))
    hash))

;;; ─── IPC name resolution ───────────────────────────────────────────

(defparameter *ipc-name* nil
  "Cached IPC name string.")

(defun %ipc-name ()
  "Compute the interprocess socket name matching Rust's ipc::ipc_name()."
  (or *ipc-name*
      (setf *ipc-name*
            (or (%boot-env "HARMONIA_RUNTIME_SOCKET")
                (let ((sr (or (%boot-env "HARMONIA_STATE_ROOT")
                              (let ((home (%boot-env "HOME")))
                                (when home
                                  (concatenate 'string home "/.harmoniis/harmonia")))
                              (concatenate 'string
                                           (string-right-trim "/" (%boot-env "TMPDIR" "/tmp"))
                                           "/harmonia"))))
                  (format nil "harmonia-runtime-~16,'0X" (%fnv1a-64 sr)))))))

(defun %ipc-connect-path ()
  "Platform-specific connectable path from the IPC name.
On Linux: abstract namespace (null-byte prefix).
On macOS/FreeBSD: /tmp/{name} filesystem socket."
  (let ((name (%ipc-name)))
    ;; If HARMONIA_RUNTIME_SOCKET was set to a filesystem path, use it directly
    (when (and (> (length name) 0) (char= (char name 0) #\/))
      (return-from %ipc-connect-path name))
    ;; Platform-conditional transport
    #+linux
    (concatenate 'string (string (code-char 0)) name)  ; abstract namespace
    #+(or darwin freebsd)
    (concatenate 'string "/tmp/" name)                  ; filesystem socket
    #-(or linux darwin freebsd)
    (progn
      (%log :warn "ipc" "Unsupported platform for IPC")
      nil)))

;;; ─── Security token ────────────────────────────────────────────────

(defparameter *ipc-token* nil
  "Cached 64-byte hex nonce token.")

(defun %ipc-token ()
  "Read the IPC nonce token from env var or disk."
  (or *ipc-token*
      (setf *ipc-token*
            (or (%boot-env "HARMONIA_IPC_TOKEN")
                (let ((sr (or (%boot-env "HARMONIA_STATE_ROOT")
                              (let ((home (%boot-env "HOME")))
                                (when home
                                  (concatenate 'string home "/.harmoniis/harmonia")))
                              (concatenate 'string
                                           (string-right-trim "/" (%boot-env "TMPDIR" "/tmp"))
                                           "/harmonia"))))
                  (let ((path (concatenate 'string sr "/ipc.token")))
                    (when (probe-file path)
                      (string-trim '(#\Space #\Newline #\Return)
                                   (with-open-file (f path) (read-line f nil ""))))))))))

(defun %ipc-send-token (stream)
  "Send the 64-byte hex nonce token before the first sexp frame."
  (let ((token (%ipc-token)))
    (when (and token (= (length token) 64))
      (let ((bytes (sb-ext:string-to-octets token :external-format :ascii)))
        (write-sequence bytes stream)
        (force-output stream)
        t))))

;;; ─── Connection: fresh per call ───────────────────────────────────────

(defun %ipc-connect ()
  "Open a fresh connection with nonce handshake. Returns stream or nil."
  (let ((path (%ipc-connect-path)))
    (unless path
      (return-from %ipc-connect nil))
    ;; For filesystem paths, check the socket exists
    (when (and (> (length path) 0)
               (char/= (char path 0) (code-char 0))  ; not abstract namespace
               (not (probe-file path)))
      (return-from %ipc-connect nil))
    (handler-case
        (let ((socket (make-instance 'sb-bsd-sockets:local-socket :type :stream)))
          (sb-bsd-sockets:socket-connect socket path)
          (let ((stream (sb-bsd-sockets:socket-make-stream
                         socket
                         :element-type '(unsigned-byte 8)
                         :input t :output t :buffering :full)))
            ;; Send nonce token before any sexp frames
            (unless (%ipc-send-token stream)
              (handler-case (close stream) (error () nil))
              (return-from %ipc-connect nil))
            stream))
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
                           (handler-case (close stream) (error () nil)))
                       (error (e)
                         (handler-case (close stream) (error () nil))
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
            (handler-case (close stream) (error () nil)))))
    (error () nil)))

;;; ─── Convenience ──────────────────────────────────────────────────────

(defun ipc-available-p ()
  "Check if the runtime IPC endpoint is reachable."
  (let ((path (%ipc-connect-path)))
    (and path
         (or (and (> (length path) 0)
                  (char= (char path 0) (code-char 0)))  ; abstract namespace always "exists"
             (probe-file path))
         t)))

(defun ipc-parse-sexp-reply (reply)
  "Parse sexp reply. Returns nil on failure."
  (when (and reply (stringp reply) (> (length reply) 0))
    (handler-case
        (let ((*read-eval* nil))
          (read-from-string reply))
      (error () nil))))

(defun ipc-reply-ok-p (reply)
  "Check if reply starts with (:ok followed by space, paren, or end.
Rejects false matches like (:ok-evil or (:okay."
  (and reply (stringp reply)
       (>= (length reply) 4)
       (string= (subseq reply 0 4) "(:ok")
       (or (= (length reply) 4)
           (let ((next (char reply 4)))
             (or (char= next #\Space) (char= next #\))
                 (char= next #\Newline) (char= next #\Tab))))))

(defun ipc-reply-error-p (reply)
  "Check if reply contains :error as a keyword (not inside a value string)."
  (and reply (stringp reply)
       (let ((pos (search ":error" reply)))
         (and pos
              ;; Ensure :error is followed by space, quote, or paren (not :error-code)
              (let ((after (+ pos 6)))
                (or (>= after (length reply))
                    (let ((next (char reply after)))
                      (or (char= next #\Space) (char= next #\")
                          (char= next #\) )))))))))

;; ipc-extract-value is defined in ipc-ports.lisp (more robust: handles nil, trims, bare tokens).

;;; ─── Cache invalidation ───────────────────────────────────────────────

(defun ipc-reset ()
  "Clear cached IPC name, path, and token. Call after runtime restart."
  (setf *ipc-name* nil
        *ipc-token* nil))
