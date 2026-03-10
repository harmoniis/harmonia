;;; system-commands.lisp — Frontend-agnostic slash command dispatch.
;;;
;;; Intercepts /command inputs before they reach the LLM orchestration
;;; pipeline. Returns a formatted string response, :system-exit, or nil
;;; (fall-through to normal dispatch).

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

;;; ─── Command table ─────────────────────────────────────────────────────

(defparameter *system-commands*
  '("/help" "/exit" "/status"
    "/backends" "/frontends" "/tools"
    "/chronicle" "/metrics" "/security" "/identity")
  "Known system command prefixes.")

(defun %syscmd-known-p (cmd-word)
  "Check if CMD-WORD (lowercase, with slash) matches a known system command."
  (member cmd-word *system-commands* :test #'string=))

;;; ─── Payload extraction ────────────────────────────────────────────────

(defun %syscmd-extract-text (input)
  "Extract the raw text payload from INPUT (string or harmonia-signal)."
  (etypecase input
    (string input)
    (harmonia-signal (or (harmonia-signal-payload input) ""))))

;;; ─── Top-level dispatch ────────────────────────────────────────────────

(defun %maybe-dispatch-system-command (input)
  "Check if INPUT is a system slash command. If so, execute and return
   a string result or :system-exit. If not, return nil."
  (let* ((text (string-trim '(#\Space #\Tab #\Newline) (%syscmd-extract-text input)))
         (text-lower (string-downcase text)))
    ;; Must start with /
    (unless (and (> (length text-lower) 0) (char= (char text-lower 0) #\/))
      (return-from %maybe-dispatch-system-command nil))
    ;; Split into command word and arguments
    (let* ((space-pos (position #\Space text-lower))
           (cmd-word (if space-pos (subseq text-lower 0 space-pos) text-lower))
           (args-str (if space-pos
                         (string-trim '(#\Space #\Tab) (subseq text space-pos))
                         "")))
      (unless (%syscmd-known-p cmd-word)
        (return-from %maybe-dispatch-system-command nil))
      ;; Dispatch
      (handler-case
          (cond
            ((string= cmd-word "/help")       (%syscmd-help))
            ((string= cmd-word "/exit")       (%syscmd-exit))
            ((string= cmd-word "/status")     (%syscmd-status))
            ((string= cmd-word "/backends")   (%syscmd-backends args-str))
            ((string= cmd-word "/frontends")  (%syscmd-frontends args-str))
            ((string= cmd-word "/tools")      (%syscmd-tools))
            ((string= cmd-word "/chronicle")  (%syscmd-chronicle args-str))
            ((string= cmd-word "/metrics")    (%syscmd-metrics))
            ((string= cmd-word "/security")   (%syscmd-security args-str))
            ((string= cmd-word "/identity")   (%syscmd-identity))
            (t nil))
        (error (e)
          (format nil "[system] Error executing ~A: ~A" cmd-word e))))))

;;; ─── /help ─────────────────────────────────────────────────────────────

(defun %syscmd-help ()
  (format nil "~A~%~%~{~A~%~}"
          "Harmonia System Commands"
          (list
           (%syscmd-kv "/help" "Show this help listing")
           (%syscmd-kv "/exit" "Exit the TUI session (TUI only)")
           (%syscmd-kv "/status" "System status overview")
           (%syscmd-kv "/backends" "List configured LLM backends")
           (%syscmd-kv "/backends <name>" "Show specific backend details")
           (%syscmd-kv "/frontends" "List all frontends with status")
           (%syscmd-kv "/frontends <name>" "Show specific frontend details")
           (%syscmd-kv "/tools" "List configured tools")
           (%syscmd-kv "/chronicle" "Chronicle overview (summary + GC)")
           (%syscmd-kv "/chronicle harmony" "Harmony summary")
           (%syscmd-kv "/chronicle delegation" "Delegation report")
           (%syscmd-kv "/chronicle costs" "Cost report")
           (%syscmd-kv "/chronicle graph" "Concept graph overview")
           (%syscmd-kv "/chronicle gc" "GC status")
           (%syscmd-kv "/metrics" "Metrics overview (parallel report)")
           (%syscmd-kv "/security" "Security audit overview")
           (%syscmd-kv "/security posture" "Current posture details")
           (%syscmd-kv "/security errors" "Recent errors from error ring")
           (%syscmd-kv "/identity" "Vault symbol listing and key status"))))

;;; ─── /exit ─────────────────────────────────────────────────────────────

(defun %syscmd-exit ()
  (unless (%syscmd-tui-p)
    (return-from %syscmd-exit
      "[system] /exit is only available from the TUI."))
  :system-exit)

;;; ─── /status ───────────────────────────────────────────────────────────

(defun %syscmd-status ()
  (unless (%syscmd-read-allowed-p)
    (return-from %syscmd-status (%syscmd-denied "/status")))
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
  (unless (%syscmd-read-allowed-p)
    (return-from %syscmd-backends (%syscmd-denied "/backends")))
  (if (and args (> (length args) 0))
      (%syscmd-backend-detail args)
      (%syscmd-backends-list)))

(defun %syscmd-backends-list ()
  (let ((lines '())
        (backends '("anthropic" "openrouter")))
    (flet ((add (text) (push text lines)))
      (add "Configured LLM Backends")
      (add (make-string 40 :initial-element #\-))
      (dolist (name backends)
        (let ((has-key (ignore-errors
                         (vault-has-secret
                          (if (string-equal name "anthropic")
                              "ANTHROPIC_API_KEY"
                              "OPENROUTER_API_KEY")))))
          (add (format nil "  ~A~30T~A"
                       name
                       (if has-key "[key present]" "[key missing]"))))))
    (format nil "~{~A~%~}" (nreverse lines))))

(defun %syscmd-backend-detail (name)
  (let ((lines '())
        (name-trimmed (string-trim '(#\Space #\Tab) name)))
    (flet ((add (text) (push text lines)))
      (add (format nil "Backend: ~A" name-trimmed))
      (add (make-string 40 :initial-element #\-))
      (let* ((key-symbol (cond
                           ((string-equal name-trimmed "anthropic") "ANTHROPIC_API_KEY")
                           ((string-equal name-trimmed "openrouter") "OPENROUTER_API_KEY")
                           (t (format nil "~A_API_KEY" (string-upcase name-trimmed)))))
             (has-key (ignore-errors (vault-has-secret key-symbol))))
        (add (%syscmd-kv "API key:" (if has-key "present" "missing"))))
      ;; Config dump for this backend
      (let ((config (ignore-errors (config-dump name-trimmed))))
        (if config
            (progn
              (add "  Configuration:")
              (dolist (line (if (listp config) config (list config)))
                (add (format nil "    ~A" line))))
            (add "  No additional configuration."))))
    (format nil "~{~A~%~}" (nreverse lines))))

;;; ─── /frontends ────────────────────────────────────────────────────────

(defun %syscmd-frontends (args)
  (unless (%syscmd-read-allowed-p)
    (return-from %syscmd-frontends (%syscmd-denied "/frontends")))
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
  (unless (%syscmd-read-allowed-p)
    (return-from %syscmd-tools (%syscmd-denied "/tools")))
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
  (unless (%syscmd-read-allowed-p)
    (return-from %syscmd-chronicle (%syscmd-denied "/chronicle")))
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
  (unless (%syscmd-read-allowed-p)
    (return-from %syscmd-metrics (%syscmd-denied "/metrics")))
  (let ((report (ignore-errors (parallel-report))))
    (format nil "~A~%~A~%~A"
            "Metrics Overview"
            (make-string 40 :initial-element #\-)
            (or report "(unavailable)"))))

;;; ─── /security ─────────────────────────────────────────────────────────

(defun %syscmd-security (args)
  (unless (%syscmd-read-allowed-p)
    (return-from %syscmd-security (%syscmd-denied "/security")))
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

;;; ─── /identity ─────────────────────────────────────────────────────────

(defun %syscmd-identity ()
  (unless (%syscmd-read-allowed-p)
    (return-from %syscmd-identity (%syscmd-denied "/identity")))
  (let ((lines '()))
    (flet ((add (text) (push text lines)))
      (add "Identity & Vault")
      (add (make-string 40 :initial-element #\-))
      ;; Vault symbols
      (let ((symbols (ignore-errors (vault-list-symbols))))
        (add (format nil "Vault symbols (~D):" (length (or symbols '()))))
        (if symbols
            (dolist (sym symbols)
              (let ((present (ignore-errors (vault-has-secret sym))))
                (add (format nil "  ~A~30T~A" sym (if present "[set]" "[empty]")))))
            (add "  (none)")))
      (add "")
      ;; Key status for known backends
      (add "Backend key status:")
      (dolist (key-name '("ANTHROPIC_API_KEY" "OPENROUTER_API_KEY"))
        (let ((has (ignore-errors (vault-has-secret key-name))))
          (add (format nil "  ~A~30T~A" key-name (if has "present" "missing"))))))
    (format nil "~{~A~%~}" (nreverse lines))))
