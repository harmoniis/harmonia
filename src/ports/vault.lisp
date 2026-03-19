;;; vault.lisp — Port: secret key-value storage via IPC to harmonia-runtime.
;;; Also provides shared infrastructure used by all ports.

(in-package :harmonia)

;;; --- Shared infrastructure (loaded first, used by all ports) ---

(defun %split-lines (text)
  (let ((parts '())
        (start 0))
    (loop for i = (position #\Newline text :start start)
          do (push (subseq text start (or i (length text))) parts)
          if (null i) do (return)
          do (setf start (1+ i)))
    (remove-if #'(lambda (s) (zerop (length s))) (nreverse parts))))

;;; --- Vault port (via IPC) ---

(defun init-vault-port ()
  (let ((reply (ipc-vault-init)))
    (runtime-log *runtime* :vault-init (list :status (if (ipc-reply-ok-p reply) 0 -1)))
    (ipc-reply-ok-p reply)))

(defun vault-last-error ()
  "")

(defun vault-set-secret (symbol value)
  (ipc-vault-set-secret symbol value))

(defun vault-has-secret-p (symbol)
  (ipc-vault-has-secret-p symbol))

(defun vault-list-symbols ()
  (ipc-vault-list-symbols))
