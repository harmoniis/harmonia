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
  (ipc-call (build-ipc-sexp :component "vault" :op "init")))

(defun ipc-vault-set-secret (symbol value)
  (let ((reply (ipc-call
                (build-ipc-sexp :component "vault" :op "set-secret" :symbol symbol :value value))))
    (if (ipc-reply-ok-p reply) t
        (error "Vault set failed: ~A" reply))))

(defun ipc-vault-has-secret-p (symbol)
  (let ((reply (ipc-call
                (build-ipc-sexp :component "vault" :op "has-secret" :symbol symbol))))
    (and reply (search ":result t" reply) t)))

(defun ipc-vault-list-symbols ()
  (let ((reply (ipc-call (build-ipc-sexp :component "vault" :op "list-symbols"))))
    (if (and reply (ipc-reply-ok-p reply))
        (ipc-extract-string-list reply ":symbols")
        '())))

;;; ─── Config Store ───────────────────────────────────────────────────

(defun ipc-config-init ()
  (ipc-call (build-ipc-sexp :component "config" :op "init")))

(defun ipc-config-get (component scope key)
  (let ((reply (ipc-call
                (build-ipc-sexp :component "config" :op "get" :component component :scope scope :key key))))
    (ipc-extract-value reply)))

(defun ipc-config-get-or (component scope key default)
  (let ((reply (ipc-call
                (build-ipc-sexp :component "config" :op "get-or" :component component :scope scope :key key :default default))))
    (or (ipc-extract-value reply) default)))

(defun ipc-config-set (component scope key value)
  (ipc-call
   (build-ipc-sexp :component "config" :op "set" :component component :scope scope :key key :value value)))

;;; ─── Chronicle ──────────────────────────────────────────────────────

(defun ipc-chronicle-init ()
  (ipc-call (build-ipc-sexp :component "chronicle" :op "init")))

(defun ipc-chronicle-query (sql)
  (let ((reply (ipc-call
                (build-ipc-sexp :component "chronicle" :op "query" :sql sql))))
    (ipc-extract-value reply)))

(defun ipc-chronicle-harmony-summary ()
  (ipc-extract-value
   (ipc-call (build-ipc-sexp :component "chronicle" :op "harmony-summary"))))

(defun ipc-chronicle-dashboard ()
  (ipc-extract-value
   (ipc-call (build-ipc-sexp :component "chronicle" :op "dashboard"))))

(defun ipc-chronicle-gc ()
  (ipc-call (build-ipc-sexp :component "chronicle" :op "gc")))

(defun ipc-chronicle-gc-status ()
  (ipc-extract-value
   (ipc-call (build-ipc-sexp :component "chronicle" :op "gc-status"))))

(defun ipc-chronicle-cost-report ()
  (ipc-extract-value
   (ipc-call (build-ipc-sexp :component "chronicle" :op "cost-report"))))

(defun ipc-chronicle-delegation-report ()
  (ipc-extract-value
   (ipc-call (build-ipc-sexp :component "chronicle" :op "delegation-report"))))

(defun ipc-chronicle-full-digest ()
  (ipc-extract-value
   (ipc-call (build-ipc-sexp :component "chronicle" :op "full-digest"))))

;;; ─── Gateway ────────────────────────────────────────────────────────

(defun ipc-gateway-poll ()
  (ipc-call (build-ipc-sexp :component "gateway" :op "poll")))

(defun ipc-gateway-send (frontend channel payload)
  (ipc-call
   (build-ipc-sexp :component "gateway" :op "send" :frontend frontend :channel channel :payload payload)))

;;; ─── Signalograd ────────────────────────────────────────────────────

(defun ipc-signalograd-init ()
  (ipc-call (build-ipc-sexp :component "signalograd" :op "init")))

(defun ipc-signalograd-observe (observation-sexp)
  (ipc-call
   (build-ipc-sexp :component "signalograd" :op "observe" :observation observation-sexp)))

(defun ipc-signalograd-status ()
  (ipc-extract-value
   (ipc-call (build-ipc-sexp :component "signalograd" :op "status"))))

(defun ipc-signalograd-snapshot ()
  (ipc-extract-value
   (ipc-call (build-ipc-sexp :component "signalograd" :op "snapshot"))))

(defun ipc-signalograd-feedback (feedback-sexp)
  (ipc-call
   (build-ipc-sexp :component "signalograd" :op "feedback" :feedback feedback-sexp)))

(defun ipc-signalograd-reset ()
  (ipc-call (build-ipc-sexp :component "signalograd" :op "reset")))

;;; ─── Tailnet ────────────────────────────────────────────────────────

(defun ipc-tailnet-start ()
  (ipc-call (build-ipc-sexp :component "tailnet" :op "start")))

(defun ipc-tailnet-poll ()
  (ipc-call (build-ipc-sexp :component "tailnet" :op "poll")))

(defun ipc-tailnet-discover ()
  (ipc-call (build-ipc-sexp :component "tailnet" :op "discover")))

(defun ipc-tailnet-stop ()
  (ipc-call (build-ipc-sexp :component "tailnet" :op "stop")))

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
  "Escape a string for safe embedding in sexp double-quoted values.
Escapes: backslash, double-quote, newline, carriage-return, tab, and
all control characters (codes 0-31). This prevents frame corruption
when values contain newlines or binary data."
  (unless (stringp s)
    (return-from sexp-escape-lisp ""))
  (with-output-to-string (out)
    (loop for c across s
          for code = (char-code c)
          do (cond
               ((char= c #\\)       (write-string "\\\\" out))
               ((char= c #\")       (write-string "\\\"" out))
               ((char= c #\Newline) (write-string "\\n" out))
               ((char= c #\Return)  (write-string "\\r" out))
               ((char= c #\Tab)     (write-string "\\t" out))
               ((< code 32)         (format out "\\x~2,'0X" code)) ; control chars
               (t                   (write-char c out))))))

(defun build-ipc-sexp (&rest pairs)
  "Build a properly-escaped sexp string from keyword-value pairs.
Example: (build-ipc-sexp :component \"vault\" :op \"set-secret\" :symbol sym :value val)
Strings are automatically escaped via sexp-escape-lisp and quoted.
Numbers, nil, t, and keywords are printed as-is (unquoted)."
  (with-output-to-string (out)
    (write-char #\( out)
    (loop for (key val . rest) on pairs by #'cddr
          for first = t then nil
          do (progn
               (unless first (write-char #\Space out))
               ;; Write keyword
               (format out "~(~S~)" key)  ; lowercase keyword
               (write-char #\Space out)
               ;; Write value: strings get escaped+quoted, everything else as-is
               (cond
                 ((stringp val)
                  (write-char #\" out)
                  (write-string (sexp-escape-lisp val) out)
                  (write-char #\" out))
                 ((null val)    (write-string "nil" out))
                 ((eq val t)    (write-string "t" out))
                 ((keywordp val) (format out "~(~S~)" val))
                 ((numberp val) (format out "~A" val))
                 (t             (format out "~A" val)))))
    (write-char #\) out)))

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
