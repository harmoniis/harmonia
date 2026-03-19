;;; ipc-ports.lisp — IPC-based port implementations.
;;;
;;; Each function here replaces a CFFI wrapper from the old port files.
;;; They send sexp commands over the Unix domain socket to harmonia-runtime
;;; and return the parsed result.
;;;
;;; The old CFFI port files (vault.lisp, baseband.lisp, etc.) still exist
;;; and still define the public API functions (vault-set-secret, gateway-poll,
;;; etc.). This file provides the IPC transport that those functions delegate
;;; to when CFFI is unavailable (which is now always the case).
;;;
;;; Pattern: each function calls (ipc-call ...) and parses the reply.

(in-package :harmonia)

;;; ─── IPC transport predicate ────────────────────────────────────────

(defparameter *use-ipc-transport* t
  "When true, all port calls route through IPC instead of CFFI.
   This is the default and only supported mode after the FFI removal.")

;;; ─── Vault ──────────────────────────────────────────────────────────

(defun ipc-vault-init ()
  (ipc-call "(:component \"vault\" :op \"init\")"))

(defun ipc-vault-set-secret (symbol value)
  (let ((reply (ipc-call
                (format nil "(:component \"vault\" :op \"set-secret\" :symbol \"~A\" :value \"~A\")"
                        (sexp-escape-lisp symbol) (sexp-escape-lisp value)))))
    (if (ipc-reply-ok-p reply) t
        (error "Vault set failed: ~A" reply))))

(defun ipc-vault-has-secret-p (symbol)
  (let ((reply (ipc-call
                (format nil "(:component \"vault\" :op \"has-secret\" :symbol \"~A\")"
                        (sexp-escape-lisp symbol)))))
    (and reply (search ":result t" reply) t)))

(defun ipc-vault-list-symbols ()
  (let ((reply (ipc-call "(:component \"vault\" :op \"list-symbols\")")))
    (if (and reply (ipc-reply-ok-p reply))
        (ipc-extract-string-list reply ":symbols")
        '())))

;;; ─── Config Store ───────────────────────────────────────────────────

(defun ipc-config-init ()
  (ipc-call "(:component \"config\" :op \"init\")"))

(defun ipc-config-get (component scope key)
  (let ((reply (ipc-call
                (format nil "(:component \"config\" :op \"get\" :component \"~A\" :scope \"~A\" :key \"~A\")"
                        (sexp-escape-lisp component) (sexp-escape-lisp scope) (sexp-escape-lisp key)))))
    (ipc-extract-value reply)))

(defun ipc-config-get-or (component scope key default)
  (let ((reply (ipc-call
                (format nil "(:component \"config\" :op \"get-or\" :component \"~A\" :scope \"~A\" :key \"~A\" :default \"~A\")"
                        (sexp-escape-lisp component) (sexp-escape-lisp scope)
                        (sexp-escape-lisp key) (sexp-escape-lisp default)))))
    (or (ipc-extract-value reply) default)))

(defun ipc-config-set (component scope key value)
  (ipc-call
   (format nil "(:component \"config\" :op \"set\" :component \"~A\" :scope \"~A\" :key \"~A\" :value \"~A\")"
           (sexp-escape-lisp component) (sexp-escape-lisp scope)
           (sexp-escape-lisp key) (sexp-escape-lisp value))))

;;; ─── Chronicle ──────────────────────────────────────────────────────

(defun ipc-chronicle-init ()
  (ipc-call "(:component \"chronicle\" :op \"init\")"))

(defun ipc-chronicle-query (sql)
  (let ((reply (ipc-call
                (format nil "(:component \"chronicle\" :op \"query\" :sql \"~A\")"
                        (sexp-escape-lisp sql)))))
    (ipc-extract-value reply)))

(defun ipc-chronicle-harmony-summary ()
  (ipc-extract-value
   (ipc-call "(:component \"chronicle\" :op \"harmony-summary\")")))

(defun ipc-chronicle-dashboard ()
  (ipc-extract-value
   (ipc-call "(:component \"chronicle\" :op \"dashboard\")")))

(defun ipc-chronicle-gc ()
  (ipc-call "(:component \"chronicle\" :op \"gc\")"))

(defun ipc-chronicle-gc-status ()
  (ipc-extract-value
   (ipc-call "(:component \"chronicle\" :op \"gc-status\")")))

(defun ipc-chronicle-cost-report ()
  (ipc-extract-value
   (ipc-call "(:component \"chronicle\" :op \"cost-report\")")))

(defun ipc-chronicle-delegation-report ()
  (ipc-extract-value
   (ipc-call "(:component \"chronicle\" :op \"delegation-report\")")))

(defun ipc-chronicle-full-digest ()
  (ipc-extract-value
   (ipc-call "(:component \"chronicle\" :op \"full-digest\")")))

;;; ─── Gateway ────────────────────────────────────────────────────────

(defun ipc-gateway-poll ()
  (ipc-call "(:component \"gateway\" :op \"poll\")"))

(defun ipc-gateway-send (frontend channel payload)
  (ipc-call
   (format nil "(:component \"gateway\" :op \"send\" :frontend \"~A\" :channel \"~A\" :payload \"~A\")"
           (sexp-escape-lisp frontend) (sexp-escape-lisp channel) (sexp-escape-lisp payload))))

;;; ─── Signalograd ────────────────────────────────────────────────────

(defun ipc-signalograd-init ()
  (ipc-call "(:component \"signalograd\" :op \"init\")"))

(defun ipc-signalograd-observe (observation-sexp)
  (ipc-call
   (format nil "(:component \"signalograd\" :op \"observe\" :observation \"~A\")"
           (sexp-escape-lisp observation-sexp))))

(defun ipc-signalograd-status ()
  (ipc-extract-value
   (ipc-call "(:component \"signalograd\" :op \"status\")")))

(defun ipc-signalograd-snapshot ()
  (ipc-extract-value
   (ipc-call "(:component \"signalograd\" :op \"snapshot\")")))

(defun ipc-signalograd-feedback (feedback-sexp)
  (ipc-call
   (format nil "(:component \"signalograd\" :op \"feedback\" :feedback \"~A\")"
           (sexp-escape-lisp feedback-sexp))))

(defun ipc-signalograd-reset ()
  (ipc-call "(:component \"signalograd\" :op \"reset\")"))

;;; ─── Tailnet ────────────────────────────────────────────────────────

(defun ipc-tailnet-start ()
  (ipc-call "(:component \"tailnet\" :op \"start\")"))

(defun ipc-tailnet-poll ()
  (ipc-call "(:component \"tailnet\" :op \"poll\")"))

(defun ipc-tailnet-discover ()
  (ipc-call "(:component \"tailnet\" :op \"discover\")"))

(defun ipc-tailnet-stop ()
  (ipc-call "(:component \"tailnet\" :op \"stop\")"))

;;; ─── Actor Protocol (via IPC) ───────────────────────────────────────

(defun ipc-actor-register (kind)
  (let ((reply (ipc-call (format nil "(:register :kind \"~A\")" (sexp-escape-lisp kind)))))
    (when (ipc-reply-ok-p reply)
      (ipc-extract-u64 reply ":id"))))

(defun ipc-actor-heartbeat (id bytes-delta)
  (ipc-cast (format nil "(:heartbeat :id ~D :bytes-delta ~D)" id bytes-delta)))

(defun ipc-actor-post (source target payload-sexp)
  (ipc-cast (format nil "(:post :source ~D :target ~D :payload \"~A\")"
                    source target (sexp-escape-lisp payload-sexp))))

(defun ipc-actor-drain ()
  (or (ipc-call "(:drain)") "()"))

(defun ipc-actor-state (id)
  (ipc-call (format nil "(:state :id ~D)" id)))

(defun ipc-actor-list ()
  (or (ipc-call "(:list)") "()"))

(defun ipc-actor-deregister (id)
  (ipc-call (format nil "(:deregister :id ~D)" id)))

;;; ─── Helpers ────────────────────────────────────────────────────────

(defun sexp-escape-lisp (s)
  "Escape backslash and double-quote for embedding in sexp strings."
  (with-output-to-string (out)
    (loop for c across (or s "")
          do (case c
               (#\\ (write-string "\\\\" out))
               (#\" (write-string "\\\"" out))
               (t   (write-char c out))))))

(defun ipc-extract-value (reply)
  "Extract the :result value from an IPC reply like (:ok :result \"...\")."
  (when (and reply (stringp reply))
    (let ((pos (search ":result" reply)))
      (when pos
        (let* ((after (subseq reply (+ pos 8)))
               (trimmed (string-trim '(#\Space #\Tab) after)))
          (cond
            ;; nil value
            ((string= trimmed "nil)") nil)
            ;; Quoted string
            ((and (> (length trimmed) 0) (char= (char trimmed 0) #\"))
             (let ((inner (subseq trimmed 1)))
               (let ((end (position #\" inner)))
                 (when end (subseq inner 0 end)))))
            ;; Bare value
            (t (let ((end (position #\) trimmed)))
                 (if end (subseq trimmed 0 end) trimmed)))))))))

(defun ipc-extract-u64 (reply key)
  "Extract a numeric value after KEY from an IPC reply."
  (when (and reply (stringp reply))
    (let ((pos (search key reply)))
      (when pos
        (let* ((after (subseq reply (+ pos (length key))))
               (trimmed (string-trim '(#\Space #\Tab) after))
               (num-str (with-output-to-string (out)
                          (loop for c across trimmed
                                while (digit-char-p c)
                                do (write-char c out)))))
          (when (> (length num-str) 0)
            (parse-integer num-str)))))))

(defun ipc-extract-string-list (reply key)
  "Extract a list of strings from an IPC reply like (:ok :symbols (\"a\" \"b\"))."
  (declare (ignore key))
  ;; Simple extraction — parse the reply sexp
  (let ((parsed (ipc-parse-sexp-reply reply)))
    (when (and (listp parsed) (getf parsed :symbols))
      (let ((syms (getf parsed :symbols)))
        (if (listp syms) syms '())))))
