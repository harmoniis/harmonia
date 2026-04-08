;;; system-commands.lisp — System command handler implementations.
;;; Gateway intercepts /commands from all frontends; commands needing Lisp state
;;; are delegated back via %gateway-command-dispatch.

(in-package :harmonia)

(defmacro %syscmd-with-lines (&body body)
  "Build multi-line response. Binds (add text), (kv label value), (divider)."
  `(let ((lines '()))
     (flet ((add (text) (push text lines))
            (kv (label value &optional (width 24))
              (push (format nil "  ~VA ~A" width label value) lines))
            (divider (&optional (char #\-))
              (push (make-string 40 :initial-element char) lines)))
       (declare (ignorable #'kv #'divider))
       ,@body
       (format nil "~{~A~%~}" (nreverse lines)))))

(defun %syscmd-owner-p ()
  (or (null *current-originating-signal*)
      (and (harmonia-signal-p *current-originating-signal*)
           (eq :owner (harmonia-signal-security-label *current-originating-signal*)))))

(defun %syscmd-read-allowed-p ()
  (or (null *current-originating-signal*)
      (and (harmonia-signal-p *current-originating-signal*)
           (member (harmonia-signal-security-label *current-originating-signal*)
                   '(:owner :authenticated)))))

(defun %syscmd-tui-p ()
  (null *current-originating-signal*))

(defun %syscmd-denied (command)
  (format nil "[system] Permission denied: ~A requires elevated access." command))

(defun %syscmd-section (title body)
  (format nil "~A~%~A~%~A" title (make-string (length title) :initial-element #\-) body))

(defun %syscmd-kv (key value &optional (width 24))
  (format nil "  ~VA ~A" width key value))

(defun %syscmd-sexp-block (data)
  (with-output-to-string (s)
    (let ((*print-pretty* t) (*print-right-margin* 80))
      (write data :stream s))))

(defun %syscmd-extract-text (input)
  (etypecase input
    (string input)
    (harmonia-signal (or (harmonia-signal-payload input) ""))))

;;; --- Gateway callback dispatch ---

(defun %gateway-dispatch-command (command args)
  (let ((args-str (or args "")))
    (handler-case
        (cond
          ((string= command "/status")     (%syscmd-status))
          ((string= command "/diagnose")   (%syscmd-diagnose))
          ((string= command "/backends")   (%syscmd-backends args-str))
          ((string= command "/frontends")  (%syscmd-frontends args-str))
          ((string= command "/tools")      (%syscmd-tools))
          ((string= command "/chronicle")  (%syscmd-chronicle args-str))
          ((string= command "/metrics")    (%syscmd-metrics))
          ((string= command "/security")   (%syscmd-security args-str))
          ((string= command "/feedback")   (%syscmd-feedback args-str))
          ((string= command "/route")      (%syscmd-route args-str))
          ((string= command "/exit")       ":system-exit")
          (t nil))
      (error (e) (format nil "[system] Error executing ~A: ~A" command e)))))

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
       (let ((*print-pretty* nil) (*print-readably* nil))
         (write value :stream s))))
    (t nil)))

(defun %gateway-dispatch-payment-policy (summary)
  (let* ((*read-eval* nil)
         (summary-sexp (handler-case (read-from-string summary) (error () nil)))
         (requested-action (string-downcase
                            (or (and (listp summary-sexp)
                                     (getf summary-sexp :requested-action)) ""))))
    (when (and summary-sexp (fboundp 'payment-policy-decide))
      (let ((custom (%payment-policy-plist-to-sexp
                     (handler-case (payment-policy-decide summary-sexp) (error () nil)))))
        (when custom (return-from %gateway-dispatch-payment-policy custom))))
    (when (zerop (length requested-action))
      (return-from %gateway-dispatch-payment-policy "(:mode :free)"))
    (let* ((mode (string-downcase
                  (or (%payment-policy-config-value (format nil "~A-mode" requested-action)) "free")))
           (price (%payment-policy-config-value (format nil "~A-price" requested-action)))
           (unit (or (%payment-policy-config-value (format nil "~A-unit" requested-action)) "wats"))
           (rails (%payment-policy-split-rails
                   (%payment-policy-config-value (format nil "~A-allowed-rails" requested-action)))))
      (cond
        ((string= mode "deny")
         (format nil "(:mode :deny :code \"payment_denied\" :message \"Payment denied for ~A.\")"
                 requested-action))
        ((or (null price) (zerop (length (string-trim '(#\Space #\Tab #\Newline #\Return) price))))
         "(:mode :free)")
        (t (format nil "(:mode :pay :action \"~A\" :price \"~A\" :unit \"~A\" :allowed-rails (~{~S~^ ~}) :policy-id \"config-store\")"
                   requested-action
                   (string-trim '(#\Space #\Tab #\Newline #\Return) price)
                   (string-trim '(#\Space #\Tab #\Newline #\Return) unit)
                   rails))))))

;;; --- Legacy fallback dispatch ---

(defparameter *system-commands*
  '("/help" "/exit" "/status" "/diagnose" "/backends" "/frontends" "/tools"
    "/chronicle" "/metrics" "/security" "/feedback" "/wallet" "/identity"
    "/auto" "/eco" "/premium" "/free" "/route"))

(defun %syscmd-known-p (cmd-word)
  (member cmd-word *system-commands* :test #'string=))

(defun %maybe-dispatch-system-command (input)
  "Legacy fallback: intercept /commands the gateway missed."
  (let* ((text (string-trim '(#\Space #\Tab #\Newline) (%syscmd-extract-text input)))
         (text-lower (string-downcase text)))
    (unless (and (> (length text-lower) 0) (char= (char text-lower 0) #\/))
      (return-from %maybe-dispatch-system-command nil))
    (let* ((space-pos (position #\Space text-lower))
           (cmd-word (if space-pos (subseq text-lower 0 space-pos) text-lower))
           (args-str (if space-pos (string-trim '(#\Space #\Tab) (subseq text space-pos)) "")))
      (unless (%syscmd-known-p cmd-word)
        (return-from %maybe-dispatch-system-command nil))
      (handler-case (%gateway-dispatch-command cmd-word args-str)
        (error (e) (format nil "[system] Error executing ~A: ~A" cmd-word e))))))

(defun %syscmd-exit ()
  (unless (%syscmd-tui-p)
    (return-from %syscmd-exit "[system] /exit is only available from the TUI."))
  :system-exit)

(defun %syscmd-feedback (args)
  (let ((note (string-trim '(#\Space #\Tab #\Newline) (or args ""))))
    (when (zerop (length note))
      (return-from %syscmd-feedback
        "[system] Usage: /feedback <note about what to improve or preserve>."))
    (let ((event (%presentation-maybe-record-feedback note :source :explicit
                                                      :explicit-p t :runtime *runtime*)))
      (if event
          (format nil "[system] Feedback recorded for ~A (~{~A~^, ~})."
                  (or (getf event :response-id) "latest response")
                  (or (getf event :tags) '(:presentation)))
          "[system] Feedback recorded."))))

;;; --- /status ---

(defun %syscmd-status ()
  (%syscmd-with-lines
    (add "System Status") (divider)
    (add "") (add "Phoenix Supervisor:")
    (let ((health (handler-case (%phoenix-health) (error () nil))))
      (add (if health (format nil "  ~A" health)
                "  Phoenix health endpoint unreachable (127.0.0.1:9100)")))
    (add "") (add "SBCL Runtime:")
    (if *runtime*
        (progn
          (kv "Cycle:" (or (handler-case (runtime-state-cycle *runtime*) (error () nil)) "?"))
          (kv "Phase:" (or (handler-case (runtime-state-harmonic-phase *runtime*) (error () nil)) "?"))
          (kv "Environment:" (or (handler-case (runtime-state-environment *runtime*) (error () nil)) "?")))
        (add "  Runtime not initialized."))
    (kv "Security posture:" (or (handler-case *security-posture* (error () nil)) "?"))
    (kv "Total tick errors:" (or (handler-case *tick-error-count* (error () nil)) "?"))
    (kv "Consecutive errors:" (or (handler-case *consecutive-tick-errors* (error () nil)) "?"))
    (let ((sg (handler-case (and (fboundp 'signalograd-status) (signalograd-status)) (error () nil))))
      (when (and sg (listp sg))
        (kv "Signalograd cycle:" (or (getf sg :cycle) "?"))
        (kv "Signalograd conf:" (format nil "~,3f" (or (getf sg :confidence) 0.0)))))
    (kv "Router alive:" (if (handler-case (router-healthcheck) (error () nil)) "yes" "no"))
    (kv "Frontends loaded:" (length (or (handler-case (gateway-list-frontends) (error () nil)) '())))
    (let ((tools (handler-case (tool-runtime-list) (error () nil))))
      (kv "Native tools loaded:" (length (or tools '())))
      (when *runtime*
        (kv "Registered tools:" (handler-case (hash-table-count (runtime-state-tools *runtime*)) (error () nil)))))))

;;; --- /diagnose ---

(defun %syscmd-diagnose ()
  (%syscmd-with-lines
    (add "Self-Diagnosis Report") (divider #\=)
    (add "") (add "Phoenix Supervisor:") (divider)
    (let ((health (handler-case (%phoenix-health) (error () nil))))
      (add (if health (format nil "  ~A" health)
                "  UNREACHABLE -- Phoenix health endpoint at 127.0.0.1:9100 not responding.")))
    (add "") (add "SBCL Runtime:") (divider)
    (if *runtime*
        (progn
          (kv "Cycle:" (or (handler-case (runtime-state-cycle *runtime*) (error () nil)) "?"))
          (kv "Phase:" (or (handler-case (runtime-state-harmonic-phase *runtime*) (error () nil)) "?"))
          (kv "Environment:" (or (handler-case (runtime-state-environment *runtime*) (error () nil)) "?"))
          (kv "Uptime (s):" (or (handler-case (- (get-universal-time) (runtime-state-started-at *runtime*)) (error () nil)) "?"))
          (kv "Queue depth:" (or (handler-case (length (runtime-state-prompt-queue *runtime*)) (error () nil)) "?"))
          (kv "Tool count:" (or (handler-case (hash-table-count (runtime-state-tools *runtime*)) (error () nil)) "?")))
        (add "  Runtime not initialized."))
    (add "") (add "Loaded Modules:") (divider)
    (let ((libs (handler-case (introspect-libs) (error () nil))))
      (if libs
          (dolist (lib libs)
            (add (format nil "  ~A  status=~A  crashes=~A"
                         (getf lib :name) (or (getf lib :status) "?") (or (getf lib :crash-count) 0))))
          (add "  No modules registered.")))
    (add "") (add "Recent Errors (last 10):") (divider)
    (let ((errors (handler-case (introspect-recent-errors 10) (error () nil))))
      (if errors (dolist (err errors) (add (format nil "  ~A" err)))
          (add "  No recent errors.")))
    (add "")
    (kv "Security posture:" (or (handler-case *security-posture* (error () nil)) "?"))
    (kv "Total tick errors:" (or (handler-case *tick-error-count* (error () nil)) "?"))
    (kv "Router alive:" (if (handler-case (router-healthcheck) (error () nil)) "yes" "no"))
    (let ((sg (handler-case (and (fboundp 'signalograd-status) (signalograd-status)) (error () nil))))
      (when (and sg (listp sg))
        (kv "Signalograd cycle:" (or (getf sg :cycle) "?"))
        (kv "Signalograd conf:" (format nil "~,3f" (or (getf sg :confidence) 0.0)))))))

;;; --- /backends ---

(defun %syscmd-backends (args)
  (if (and args (> (length args) 0)) (%syscmd-backend-detail args) (%syscmd-backends-list)))

(defun %syscmd-backends-list ()
  (let ((backends (handler-case (backend-list-backends) (error () nil))))
    (%syscmd-with-lines
      (add "Configured LLM Backends") (divider)
      (if (and backends (listp backends))
          (dolist (b backends)
            (add (format nil "  ~A~30T~A" (or (getf b :id) "unknown")
                         (if (getf b :healthy) "[healthy]" "[unhealthy]"))))
          (add "  No provider backends registered.")))))

(defun %syscmd-backend-detail (name)
  (let* ((name-trimmed (string-trim '(#\Space #\Tab) name))
         (status (handler-case (backend-backend-status name-trimmed) (error () nil))))
    (%syscmd-with-lines
      (add (format nil "Backend: ~A" name-trimmed)) (divider)
      (if (and status (listp status))
          (loop for (k v) on status by #'cddr
                do (kv (format nil "~A:" k) (format nil "~A" v)))
          (add "  No provider status available.")))))

;;; --- /frontends ---

(defun %syscmd-frontends (args)
  (if (and args (> (length args) 0))
      (%syscmd-frontend-detail (string-trim '(#\Space #\Tab) args))
      (%syscmd-frontends-list)))

(defun %syscmd-frontends-list ()
  (let ((frontends (handler-case (gateway-list-frontends) (error () nil))))
    (%syscmd-with-lines
      (add "Registered Frontends") (divider)
      (if (and frontends (listp frontends))
          (dolist (fe frontends)
            (let* ((name (format nil "~A" (if (listp fe) (or (getf fe :name) (car fe)) fe)))
                   (crashes (or (handler-case (gateway-crash-count name) (error () nil)) 0)))
              (add (format nil "  ~A~30Tcrashes: ~D" name crashes))))
          (add "  No frontends registered.")))))

(defun %syscmd-frontend-detail (name)
  (let ((status (handler-case (gateway-frontend-status name) (error () nil))))
    (%syscmd-with-lines
      (add (format nil "Frontend: ~A" name)) (divider)
      (when (and status (listp status))
        (loop for (k v) on status by #'cddr
              do (kv (format nil "~A:" k) (format nil "~A" v))))
      (unless status (add "  No status available (frontend may not be registered)."))
      (kv "Crash count:" (or (handler-case (gateway-crash-count name) (error () nil)) "?")))))

;;; --- /tools ---

(defun %syscmd-tools ()
  (%syscmd-with-lines
    (add "Configured Tools") (divider)
    (let ((native (handler-case (tool-runtime-list) (error () nil))))
      (add "  Native libraries:")
      (if native (dolist (name native) (add (format nil "    ~A" name)))
          (add "    (none)")))
    (when *runtime*
      (add "") (add "  Registered tools:")
      (let ((tools (handler-case (tool-status *runtime*) (error () nil))))
        (if tools
            (dolist (tool tools)
              (add (format nil "    ~A" (if (listp tool) (or (getf tool :name) (car tool)) tool))))
            (add "    (none)"))))))

;;; --- /chronicle ---

(defun %syscmd-chronicle (args)
  (let ((sub (string-downcase (string-trim '(#\Space #\Tab) args))))
    (cond
      ((string= sub "")          (%syscmd-chronicle-overview))
      ((string= sub "harmony")   (%syscmd-chronicle-harmony))
      ((string= sub "delegation") (%syscmd-chronicle-delegation))
      ((string= sub "costs")     (%syscmd-chronicle-costs))
      ((string= sub "graph")     (%syscmd-chronicle-graph))
      ((string= sub "gc")        (%syscmd-chronicle-gc))
      (t (format nil "[system] Unknown chronicle sub-command: ~A~%Use /chronicle for overview." sub)))))

(defun %syscmd-chronicle-overview ()
  (%syscmd-with-lines
    (add "Chronicle Overview") (divider)
    (let ((summary (handler-case (chronicle-harmony-summary) (error () nil))))
      (add "Harmony Summary:")
      (add (if summary (%syscmd-sexp-block summary) "  (unavailable)")))
    (add "")
    (let ((gc (handler-case (chronicle-gc-status) (error () nil))))
      (add "GC Status:")
      (add (if gc (%syscmd-sexp-block gc) "  (unavailable)")))))

(defmacro %syscmd-chronicle-section (title fn unavailable-msg)
  `(let ((data (handler-case (,fn) (error () nil))))
     (if data
         (format nil "~A~%~A~%~A" ,title (make-string 40 :initial-element #\-) (%syscmd-sexp-block data))
         ,unavailable-msg)))

(defun %syscmd-chronicle-harmony ()
  (%syscmd-chronicle-section "Chronicle: Harmony Summary" chronicle-harmony-summary
                             "[system] Harmony summary unavailable."))

(defun %syscmd-chronicle-delegation ()
  (%syscmd-chronicle-section "Chronicle: Delegation Report" chronicle-delegation-report
                             "[system] Delegation report unavailable."))

(defun %syscmd-chronicle-costs ()
  (%syscmd-chronicle-section "Chronicle: Cost Report" chronicle-cost-report
                             "[system] Cost report unavailable."))

(defun %syscmd-chronicle-gc ()
  (%syscmd-chronicle-section "Chronicle: GC Status" chronicle-gc-status
                             "[system] GC status unavailable."))

(defun %syscmd-chronicle-graph ()
  (%syscmd-with-lines
    (add "Chronicle: Concept Graph") (divider)
    (let ((domains (handler-case (chronicle-graph-domains) (error () nil))))
      (add "Domains:")
      (add (if domains (%syscmd-sexp-block domains) "  (unavailable)")))
    (add "")
    (let ((central (handler-case (chronicle-graph-central :limit 10) (error () nil))))
      (add "Central Concepts:")
      (add (if central (%syscmd-sexp-block central) "  (unavailable)")))))

;;; --- /metrics ---

(defun %syscmd-metrics ()
  (format nil "~A~%~A~%~A" "Metrics Overview" (make-string 40 :initial-element #\-)
          (or (handler-case (parallel-report) (error () nil)) "(unavailable)")))

;;; --- /security ---

(defun %syscmd-security (args)
  (let ((sub (string-downcase (string-trim '(#\Space #\Tab) args))))
    (cond
      ((string= sub "")        (%syscmd-security-overview))
      ((string= sub "posture") (%syscmd-security-posture))
      ((string= sub "errors")  (%syscmd-security-errors))
      (t (format nil "[system] Unknown security sub-command: ~A~%Use /security for overview." sub)))))

(defun %syscmd-security-overview ()
  (%syscmd-with-lines
    (add "Security Audit Overview") (divider)
    (kv "Posture:" (or (handler-case *security-posture* (error () nil)) "?"))
    (kv "Total tick errors:" (or (handler-case *tick-error-count* (error () nil)) "?"))
    (kv "Consecutive errors:" (or (handler-case *consecutive-tick-errors* (error () nil)) "?"))
    (kv "Security events:" (or (handler-case *security-event-count* (error () nil)) "?"))
    (add "")
    (let ((runtime-info (handler-case (introspect-runtime) (error () nil))))
      (when runtime-info
        (add "Runtime Introspection:")
        (loop for (k v) on runtime-info by #'cddr
              do (kv (format nil "~A:" k) (format nil "~A" v)))))
    (add "")
    (add (format nil "Loaded libraries: ~D" (length (or (handler-case (introspect-libs) (error () nil)) '()))))))

(defun %syscmd-security-posture ()
  (%syscmd-with-lines
    (add "Security Posture") (divider)
    (kv "Current posture:" (or (handler-case *security-posture* (error () nil)) "?"))
    (kv "Event count:" (or (handler-case *security-event-count* (error () nil)) "?"))
    (kv "Tick errors:" (or (handler-case *tick-error-count* (error () nil)) "?"))
    (kv "Consecutive errors:" (or (handler-case *consecutive-tick-errors* (error () nil)) "?"))
    (add "") (add "Per-frontend injection counts:")
    (handler-case
        (maphash (lambda (fe count) (add (format nil "  ~A: ~D" fe count)))
                 *security-injection-counts*)
      (error () nil))))

(defun %syscmd-security-errors ()
  (let ((errors (handler-case (introspect-recent-errors 20) (error () nil))))
    (%syscmd-with-lines
      (add "Recent Errors") (divider)
      (if errors (dolist (err errors) (add (format nil "  ~A" err)))
          (add "  No recent errors.")))))

;;; --- /route ---

(defun %syscmd-route (args)
  (declare (ignore args))
  (%load-routing-tier)
  (let* ((pool (%tier-model-pool *routing-tier*))
         (scores (handler-case (%load-swarm-scores) (error () nil))))
    (%syscmd-with-lines
      (add "Routing Status") (divider)
      (kv "Active tier:" (symbol-name *routing-tier*))
      (kv "Pool size:" (format nil "~D models" (length pool)))
      (add "") (add "Model Pool:")
      (if pool
          (dolist (m pool)
            (let ((profile (%profile-by-id m)))
              (add (format nil "  ~A  tier=~A cost=~A quality=~A"
                           m (or (and profile (getf profile :tier)) "?")
                           (or (and profile (getf profile :cost)) "?")
                           (or (and profile (getf profile :quality)) "?")))))
          (add "  (no models -- CLI only)"))
      (add "") (add "Signalograd Routing Deltas:")
      (dolist (pair '(("price" :price) ("speed" :speed) ("success" :success) ("reasoning" :reasoning)))
        (kv (format nil "  ~A-delta:" (first pair))
             (format nil "~,4f" (signalograd-routing-weight (second pair) 0.0 *runtime*))))
      (when scores
        (add "") (add "Tier Success Rates:")
        (dolist (tier-kw '(:eco :premium :auto))
          (let ((tier-models (%tier-model-pool tier-kw))
                (total 0) (success-sum 0.0))
            (dolist (s scores)
              (when (member (getf s :model-id) tier-models :test #'string=)
                (incf total) (incf success-sum (or (getf s :success-rate) 0.0))))
            (when (> total 0)
              (add (format nil "  ~A: ~,2f% (~D models tracked)"
                           (symbol-name tier-kw) (* 100.0 (/ success-sum total)) total)))))))))
