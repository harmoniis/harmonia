;;; system-commands.lisp — System command handler implementations.
;;;
;;; The gateway (Rust) intercepts ALL /commands from ALL frontends before
;;; they reach the Lisp orchestrator. Commands requiring Lisp runtime state
;;; are delegated back via a registered callback (%gateway-command-dispatch).
;;;
;;; This file provides:
;;;   1. Handler implementations for commands that need Lisp state.
;;;   2. The gateway dispatch callback that routes delegated commands.
;;;   3. A legacy fallback (%maybe-dispatch-system-command) for any
;;;      commands that slip through without the gateway callback.

(in-package :harmonia)

;;; ─── Security helpers ──────────────────────────────────────────────────

(defun %syscmd-owner-p ()
  "True when the current originator is the owner (TUI / nil signal) or has :owner label."
  (or (null *current-originating-signal*)
      (and (harmonia-signal-p *current-originating-signal*)
           (eq :owner (harmonia-signal-security-label *current-originating-signal*)))))

(defun %syscmd-read-allowed-p ()
  "True when the current originator may read system status (owner or authenticated)."
  (or (null *current-originating-signal*)
      (and (harmonia-signal-p *current-originating-signal*)
           (member (harmonia-signal-security-label *current-originating-signal*)
                   '(:owner :authenticated)))))

(defun %syscmd-tui-p ()
  "True when the input came from the TUI (no originating signal)."
  (null *current-originating-signal*))

(defun %syscmd-denied (command)
  (format nil "[system] Permission denied: ~A requires elevated access." command))

;;; ─── Text formatting helpers ───────────────────────────────────────────

(defun %syscmd-section (title body)
  "Format a titled section."
  (format nil "~A~%~A~%~A" title (make-string (length title) :initial-element #\-) body))

(defun %syscmd-kv (key value &optional (width 24))
  "Format a key-value line with alignment."
  (format nil "  ~VA ~A" width key value))

(defun %syscmd-sexp-block (data)
  "Pretty-print an s-expression to a string."
  (with-output-to-string (s)
    (let ((*print-pretty* t)
          (*print-right-margin* 80))
      (write data :stream s))))

;;; ─── Payload extraction ────────────────────────────────────────────────

(defun %syscmd-extract-text (input)
  "Extract the raw text payload from INPUT (string or harmonia-signal)."
  (etypecase input
    (string input)
    (harmonia-signal (or (harmonia-signal-payload input) ""))))

;;; ─── Gateway callback dispatch ─────────────────────────────────────────
;;; Called by the Rust gateway via the registered command query callback.
;;; The gateway has already enforced security, so handlers here are called
;;; in an owner-trust context (*current-originating-signal* = nil).

(defun %gateway-dispatch-command (command args)
  "Dispatch a delegated system command from the gateway.
   Returns a response string, or nil if unrecognised."
  (let ((args-str (or args "")))
    (handler-case
        (cond
          ((string= command "/status")     (%syscmd-status))
          ((string= command "/backends")   (%syscmd-backends args-str))
          ((string= command "/frontends")  (%syscmd-frontends args-str))
          ((string= command "/tools")      (%syscmd-tools))
          ((string= command "/chronicle")  (%syscmd-chronicle args-str))
          ((string= command "/metrics")    (%syscmd-metrics))
          ((string= command "/security")   (%syscmd-security args-str))
          ((string= command "/feedback")   (%syscmd-feedback args-str))
          ((string= command "/exit")       ":system-exit")
          (t nil))
      (error (e)
        (format nil "[system] Error executing ~A: ~A" command e)))))

(defun %payment-policy-config-value (key)
  (config-get-for "payment-auth" key "payment-auth"))

(defun %payment-policy-split-rails (raw)
  (let ((trimmed (string-trim '(#\Space #\Tab #\Newline #\Return) (or raw ""))))
    (labels ((consume (start acc)
               (let ((comma (position #\, trimmed :start start)))
                 (if comma
                     (consume (1+ comma)
                              (let ((value (string-downcase
                                            (string-trim '(#\Space #\Tab #\Newline #\Return)
                                                         (subseq trimmed start comma)))))
                                (if (zerop (length value)) acc (append acc (list value)))))
                     (let ((value (string-downcase
                                   (string-trim '(#\Space #\Tab #\Newline #\Return)
                                                (subseq trimmed start)))))
                       (if (zerop (length value)) acc (append acc (list value))))))))
      (if (zerop (length trimmed))
          '("webcash" "voucher" "bitcoin")
          (consume 0 '())))))

(defun %payment-policy-plist-to-sexp (value)
  (cond
    ((null value) nil)
    ((stringp value) value)
    ((and (listp value) (getf value :mode))
     (with-output-to-string (s)
       (let ((*print-pretty* nil)
             (*print-readably* nil))
         (write value :stream s))))
    (t nil)))

(defun %gateway-dispatch-payment-policy (summary)
  "Return a payment policy s-expression, or nil to defer to Rust defaults."
  (let* ((*read-eval* nil)
         (summary-sexp (ignore-errors (read-from-string summary)))
         (requested-action (string-downcase
                            (or (and (listp summary-sexp)
                                     (getf summary-sexp :requested-action))
                                ""))))
    (when (and summary-sexp (fboundp 'payment-policy-decide))
      (let ((custom (%payment-policy-plist-to-sexp
                     (ignore-errors (payment-policy-decide summary-sexp)))))
        (when custom
          (return-from %gateway-dispatch-payment-policy custom))))
    (when (zerop (length requested-action))
      (return-from %gateway-dispatch-payment-policy "(:mode :free)"))
    (let* ((mode (string-downcase
                  (or (%payment-policy-config-value
                       (format nil "~A-mode" requested-action))
                      "free")))
           (price (%payment-policy-config-value
                   (format nil "~A-price" requested-action)))
           (unit (or (%payment-policy-config-value
                      (format nil "~A-unit" requested-action))
                     "wats"))
           (rails (%payment-policy-split-rails
                   (%payment-policy-config-value
                    (format nil "~A-allowed-rails" requested-action)))))
      (cond
        ((string= mode "deny")
         (format nil
                 "(:mode :deny :code \"payment_denied\" :message \"Payment denied for ~A.\")"
                 requested-action))
        ((or (null price)
             (zerop (length (string-trim '(#\Space #\Tab #\Newline #\Return) price))))
         "(:mode :free)")
        (t
         (format nil
                 "(:mode :pay :action \"~A\" :price \"~A\" :unit \"~A\" :allowed-rails (~{~S~^ ~}) :policy-id \"config-store\")"
                 requested-action
                 (string-trim '(#\Space #\Tab #\Newline #\Return) price)
                 (string-trim '(#\Space #\Tab #\Newline #\Return) unit)
                 rails))))))

;;; ─── Legacy fallback dispatch ──────────────────────────────────────────
;;; Kept for backward compatibility: if a command somehow reaches
;;; orchestrate-once without being intercepted by the gateway, this
;;; handles it. In the unified architecture, this should never trigger.

(defparameter *system-commands*
  '("/help" "/exit" "/status"
    "/backends" "/frontends" "/tools"
    "/chronicle" "/metrics" "/security"
    "/feedback" "/wallet" "/identity")
  "All known system command prefixes (gateway + delegated).")

(defun %syscmd-known-p (cmd-word)
  "Check if CMD-WORD (lowercase, with slash) matches a known system command."
  (member cmd-word *system-commands* :test #'string=))

(defun %maybe-dispatch-system-command (input)
  "Legacy fallback: intercept /commands that the gateway missed.
   In normal operation the gateway handles everything; this is defense-in-depth."
  (let* ((text (string-trim '(#\Space #\Tab #\Newline) (%syscmd-extract-text input)))
         (text-lower (string-downcase text)))
    (unless (and (> (length text-lower) 0) (char= (char text-lower 0) #\/))
      (return-from %maybe-dispatch-system-command nil))
    (let* ((space-pos (position #\Space text-lower))
           (cmd-word (if space-pos (subseq text-lower 0 space-pos) text-lower))
           (args-str (if space-pos
                         (string-trim '(#\Space #\Tab) (subseq text space-pos))
                         "")))
      (unless (%syscmd-known-p cmd-word)
        (return-from %maybe-dispatch-system-command nil))
      ;; Try the gateway dispatch (covers all commands including wallet/identity)
      (handler-case
          (%gateway-dispatch-command cmd-word args-str)
        (error (e)
          (format nil "[system] Error executing ~A: ~A" cmd-word e))))))

;;; ─── /exit ─────────────────────────────────────────────────────────────

(defun %syscmd-exit ()
  (unless (%syscmd-tui-p)
    (return-from %syscmd-exit
      "[system] /exit is only available from the TUI."))
  :system-exit)

;;; ─── /feedback ────────────────────────────────────────────────────────

(defun %syscmd-feedback (args)
  (let ((note (string-trim '(#\Space #\Tab #\Newline) (or args ""))))
    (when (zerop (length note))
      (return-from %syscmd-feedback
        "[system] Usage: /feedback <note about what to improve or preserve>."))
    (let ((event (%presentation-maybe-record-feedback note
                                                      :source :explicit
                                                      :explicit-p t
                                                      :runtime *runtime*)))
      (if event
          (format nil "[system] Feedback recorded for ~A (~{~A~^, ~})."
                  (or (getf event :response-id) "latest response")
                  (or (getf event :tags) '(:presentation)))
          "[system] Feedback recorded."))))

;;; ─── /status ───────────────────────────────────────────────────────────

(defun %syscmd-status ()
  (let ((lines '()))
    (flet ((add (text) (push text lines)))
      (add "System Status")
      (add (make-string 40 :initial-element #\-))
      ;; Runtime
      (if *runtime*
          (progn
            (add (%syscmd-kv "Cycle:" (or (ignore-errors (runtime-state-cycle *runtime*)) "?")))
            (add (%syscmd-kv "Phase:" (or (ignore-errors (runtime-state-harmonic-phase *runtime*)) "?")))
            (add (%syscmd-kv "Environment:" (or (ignore-errors (runtime-state-environment *runtime*)) "?"))))
          (add "  Runtime not initialized."))
      ;; Security
      (add (%syscmd-kv "Security posture:" (or (ignore-errors *security-posture*) "?")))
      (add (%syscmd-kv "Total tick errors:" (or (ignore-errors *tick-error-count*) "?")))
      (add (%syscmd-kv "Consecutive errors:" (or (ignore-errors *consecutive-tick-errors*) "?")))
      ;; Signalograd
      (let ((signalograd (ignore-errors (and (fboundp 'signalograd-status) (signalograd-status)))))
        (when (and signalograd (listp signalograd))
          (add (%syscmd-kv "Signalograd cycle:" (or (getf signalograd :cycle) "?")))
          (add (%syscmd-kv "Signalograd conf:" (format nil "~,3f" (or (getf signalograd :confidence) 0.0))))))
      ;; Router
      (add (%syscmd-kv "Router alive:" (if (ignore-errors (router-healthcheck)) "yes" "no")))
      ;; Frontends summary
      (let ((frontends (ignore-errors (gateway-list-frontends))))
        (add (%syscmd-kv "Frontends loaded:" (length (or frontends '())))))
      ;; Tools summary
      (let ((tools (ignore-errors (tool-runtime-list))))
        (add (%syscmd-kv "Native tools loaded:" (length (or tools '()))))
        (when *runtime*
          (add (%syscmd-kv "Registered tools:" (ignore-errors (hash-table-count (runtime-state-tools *runtime*))))))))
    (format nil "~{~A~%~}" (nreverse lines))))

;;; ─── /backends ─────────────────────────────────────────────────────────

(defun %syscmd-backends (args)
  (if (and args (> (length args) 0))
      (%syscmd-backend-detail args)
      (%syscmd-backends-list)))

(defun %syscmd-backends-list ()
  (let ((lines '())
        (backends (ignore-errors (backend-list-backends))))
    (flet ((add (text) (push text lines)))
      (add "Configured LLM Backends")
      (add (make-string 40 :initial-element #\-))
      (if (and backends (listp backends))
          (dolist (backend backends)
            (add (format nil "  ~A~30T~A"
                         (or (getf backend :id) "unknown")
                         (if (getf backend :healthy) "[healthy]" "[unhealthy]"))))
          (add "  No provider backends registered.")))
    (format nil "~{~A~%~}" (nreverse lines))))

(defun %syscmd-backend-detail (name)
  (let ((lines '())
        (name-trimmed (string-trim '(#\Space #\Tab) name))
        (status (ignore-errors (backend-backend-status (string-trim '(#\Space #\Tab) name)))))
    (flet ((add (text) (push text lines)))
      (add (format nil "Backend: ~A" name-trimmed))
      (add (make-string 40 :initial-element #\-))
      (if (and status (listp status))
          (loop for (k v) on status by #'cddr
                do (add (%syscmd-kv (format nil "~A:" k) (format nil "~A" v))))
          (add "  No provider status available.")))
    (format nil "~{~A~%~}" (nreverse lines))))

;;; ─── /frontends ────────────────────────────────────────────────────────

(defun %syscmd-frontends (args)
  (if (and args (> (length args) 0))
      (%syscmd-frontend-detail (string-trim '(#\Space #\Tab) args))
      (%syscmd-frontends-list)))

(defun %syscmd-frontends-list ()
  (let ((lines '())
        (frontends (ignore-errors (gateway-list-frontends))))
    (flet ((add (text) (push text lines)))
      (add "Registered Frontends")
      (add (make-string 40 :initial-element #\-))
      (if (and frontends (listp frontends))
          (dolist (fe frontends)
            (let* ((name (if (listp fe) (or (getf fe :name) (car fe)) fe))
                   (name-str (format nil "~A" name))
                   (crashes (or (ignore-errors (gateway-crash-count name-str)) 0)))
              (add (format nil "  ~A~30Tcrashes: ~D" name-str crashes))))
          (add "  No frontends registered.")))
    (format nil "~{~A~%~}" (nreverse lines))))

(defun %syscmd-frontend-detail (name)
  (let ((lines '())
        (status (ignore-errors (gateway-frontend-status name))))
    (flet ((add (text) (push text lines)))
      (add (format nil "Frontend: ~A" name))
      (add (make-string 40 :initial-element #\-))
      (if status
          (progn
            (when (listp status)
              (loop for (k v) on status by #'cddr
                    do (add (%syscmd-kv (format nil "~A:" k) (format nil "~A" v))))))
          (add "  No status available (frontend may not be registered)."))
      (let ((crashes (ignore-errors (gateway-crash-count name))))
        (add (%syscmd-kv "Crash count:" (or crashes "?")))))
    (format nil "~{~A~%~}" (nreverse lines))))

;;; ─── /tools ────────────────────────────────────────────────────────────

(defun %syscmd-tools ()
  (let ((lines '()))
    (flet ((add (text) (push text lines)))
      (add "Configured Tools")
      (add (make-string 40 :initial-element #\-))
      ;; Native tool-runtime libs
      (let ((native (ignore-errors (tool-runtime-list))))
        (add "  Native libraries:")
        (if native
            (dolist (name native)
              (add (format nil "    ~A" name)))
            (add "    (none)")))
      ;; Registered orchestrator tools
      (when *runtime*
        (add "")
        (add "  Registered tools:")
        (let ((tools (ignore-errors (tool-status *runtime*))))
          (if tools
              (dolist (tool tools)
                (let ((name (if (listp tool) (or (getf tool :name) (car tool)) tool)))
                  (add (format nil "    ~A" name))))
              (add "    (none)")))))
    (format nil "~{~A~%~}" (nreverse lines))))

;;; ─── /chronicle ────────────────────────────────────────────────────────

(defun %syscmd-chronicle (args)
  (let ((sub (string-downcase (string-trim '(#\Space #\Tab) args))))
    (cond
      ((string= sub "")        (%syscmd-chronicle-overview))
      ((string= sub "harmony") (%syscmd-chronicle-harmony))
      ((string= sub "delegation") (%syscmd-chronicle-delegation))
      ((string= sub "costs")   (%syscmd-chronicle-costs))
      ((string= sub "graph")   (%syscmd-chronicle-graph))
      ((string= sub "gc")      (%syscmd-chronicle-gc))
      (t (format nil "[system] Unknown chronicle sub-command: ~A~%Use /chronicle for overview." sub)))))

(defun %syscmd-chronicle-overview ()
  (let ((lines '()))
    (flet ((add (text) (push text lines)))
      (add "Chronicle Overview")
      (add (make-string 40 :initial-element #\-))
      ;; Summary
      (let ((summary (ignore-errors (chronicle-harmony-summary))))
        (add "Harmony Summary:")
        (add (if summary (%syscmd-sexp-block summary) "  (unavailable)")))
      (add "")
      ;; GC status
      (let ((gc (ignore-errors (chronicle-gc-status))))
        (add "GC Status:")
        (add (if gc (%syscmd-sexp-block gc) "  (unavailable)"))))
    (format nil "~{~A~%~}" (nreverse lines))))

(defun %syscmd-chronicle-harmony ()
  (let ((data (ignore-errors (chronicle-harmony-summary))))
    (if data
        (format nil "~A~%~A~%~A"
                "Chronicle: Harmony Summary"
                (make-string 40 :initial-element #\-)
                (%syscmd-sexp-block data))
        "[system] Harmony summary unavailable.")))

(defun %syscmd-chronicle-delegation ()
  (let ((data (ignore-errors (chronicle-delegation-report))))
    (if data
        (format nil "~A~%~A~%~A"
                "Chronicle: Delegation Report"
                (make-string 40 :initial-element #\-)
                (%syscmd-sexp-block data))
        "[system] Delegation report unavailable.")))

(defun %syscmd-chronicle-costs ()
  (let ((data (ignore-errors (chronicle-cost-report))))
    (if data
        (format nil "~A~%~A~%~A"
                "Chronicle: Cost Report"
                (make-string 40 :initial-element #\-)
                (%syscmd-sexp-block data))
        "[system] Cost report unavailable.")))

(defun %syscmd-chronicle-graph ()
  (let ((lines '()))
    (flet ((add (text) (push text lines)))
      (add "Chronicle: Concept Graph")
      (add (make-string 40 :initial-element #\-))
      (let ((domains (ignore-errors (chronicle-graph-domains))))
        (add "Domains:")
        (add (if domains (%syscmd-sexp-block domains) "  (unavailable)")))
      (add "")
      (let ((central (ignore-errors (chronicle-graph-central :limit 10))))
        (add "Central Concepts:")
        (add (if central (%syscmd-sexp-block central) "  (unavailable)"))))
    (format nil "~{~A~%~}" (nreverse lines))))

(defun %syscmd-chronicle-gc ()
  (let ((data (ignore-errors (chronicle-gc-status))))
    (if data
        (format nil "~A~%~A~%~A"
                "Chronicle: GC Status"
                (make-string 40 :initial-element #\-)
                (%syscmd-sexp-block data))
        "[system] GC status unavailable.")))

;;; ─── /metrics ──────────────────────────────────────────────────────────

(defun %syscmd-metrics ()
  (let ((report (ignore-errors (parallel-report))))
    (format nil "~A~%~A~%~A"
            "Metrics Overview"
            (make-string 40 :initial-element #\-)
            (or report "(unavailable)"))))

;;; ─── /security ─────────────────────────────────────────────────────────

(defun %syscmd-security (args)
  (let ((sub (string-downcase (string-trim '(#\Space #\Tab) args))))
    (cond
      ((string= sub "")        (%syscmd-security-overview))
      ((string= sub "posture") (%syscmd-security-posture))
      ((string= sub "errors")  (%syscmd-security-errors))
      (t (format nil "[system] Unknown security sub-command: ~A~%Use /security for overview." sub)))))

(defun %syscmd-security-overview ()
  (let ((lines '()))
    (flet ((add (text) (push text lines)))
      (add "Security Audit Overview")
      (add (make-string 40 :initial-element #\-))
      (add (%syscmd-kv "Posture:" (or (ignore-errors *security-posture*) "?")))
      (add (%syscmd-kv "Total tick errors:" (or (ignore-errors *tick-error-count*) "?")))
      (add (%syscmd-kv "Consecutive errors:" (or (ignore-errors *consecutive-tick-errors*) "?")))
      (add (%syscmd-kv "Security events:" (or (ignore-errors *security-event-count*) "?")))
      (add "")
      ;; Introspection
      (let ((runtime-info (ignore-errors (introspect-runtime))))
        (when runtime-info
          (add "Runtime Introspection:")
          (loop for (k v) on runtime-info by #'cddr
                do (add (%syscmd-kv (format nil "~A:" k) (format nil "~A" v))))))
      (add "")
      ;; Libraries
      (let ((libs (ignore-errors (introspect-libs))))
        (add (format nil "Loaded libraries: ~D" (length (or libs '()))))))
    (format nil "~{~A~%~}" (nreverse lines))))

(defun %syscmd-security-posture ()
  (let ((lines '()))
    (flet ((add (text) (push text lines)))
      (add "Security Posture")
      (add (make-string 40 :initial-element #\-))
      (add (%syscmd-kv "Current posture:" (or (ignore-errors *security-posture*) "?")))
      (add (%syscmd-kv "Event count:" (or (ignore-errors *security-event-count*) "?")))
      (add (%syscmd-kv "Tick errors:" (or (ignore-errors *tick-error-count*) "?")))
      (add (%syscmd-kv "Consecutive errors:" (or (ignore-errors *consecutive-tick-errors*) "?")))
      ;; Per-frontend injection counts
      (add "")
      (add "Per-frontend injection counts:")
      (ignore-errors
        (maphash (lambda (fe count)
                   (add (format nil "  ~A: ~D" fe count)))
                 *security-injection-counts*)))
    (format nil "~{~A~%~}" (nreverse lines))))

(defun %syscmd-security-errors ()
  (let ((errors (ignore-errors (introspect-recent-errors 20))))
    (let ((lines '()))
      (flet ((add (text) (push text lines)))
        (add "Recent Errors")
        (add (make-string 40 :initial-element #\-))
        (if errors
            (dolist (err errors)
              (add (format nil "  ~A" err)))
            (add "  No recent errors.")))
      (format nil "~{~A~%~}" (nreverse lines)))))
