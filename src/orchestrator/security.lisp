;;; security.lisp — Policy gate, invariant guards, admin intent verification.

(in-package :harmonia)

;;; --- Wave 1.5: Deterministic Policy Gate ---

(defparameter *privileged-ops*
  '("vault-set" "vault-delete" "config-set" "harmony-policy-set"
    "matrix-set-edge" "matrix-set-node" "matrix-reset-defaults"
    "model-policy-upsert" "model-policy-set-weight"
    "codemode-run"
    "parallel-set-width" "parallel-set-price")
  "Operations that require privileged access. Deterministic binary gate, not scored.")

(defun %security-label-weight (label)
  (case label
    (:owner 1.0d0)
    (:authenticated 0.8d0)
    (:anonymous 0.4d0)
    (t 0.1d0)))

(defun %route-or-error (from to &optional (originating-signal *current-originating-signal*))
  "Route check with security-aware context when an originating signal exists."
  (if (and originating-signal (harmonia-signal-p originating-signal))
      (harmonic-matrix-route-with-context-or-error
       from to
       :security-weight (%security-label-weight (harmonia-signal-security-label originating-signal))
       :dissonance (or (harmonia-signal-dissonance originating-signal) 0.0d0))
      (harmonic-matrix-route-or-error from to)))

(defun %security-log (action op signal reason)
  "Log a security event to the harmonic matrix."
  (handler-case

      (security-note-event :frontend (and signal (harmonia-signal-frontend signal)

    (error () nil))
                         :injection-count (if (and signal
                                                   (numberp (harmonia-signal-dissonance signal))
                                                   (> (harmonia-signal-dissonance signal) 0.0))
                                              1
                                              0)))
  (handler-case

      (harmonic-matrix-log-event "security-kernel" (string-downcase (symbol-name action)

    (error () nil))
                                op
                                (if signal
                                    (format nil "frontend=~A label=~A taint=~A"
                                            (harmonia-signal-frontend signal)
                                            (harmonia-signal-security-label signal)
                                            (harmonia-signal-taint signal))
                                    "internal")
                                (eq action :allowed) reason)))

(defun %policy-gate (op originating-signal &optional prompt)
  "Deterministic gate for privileged operations. Returns T if allowed, signals error if denied.
   Non-privileged ops always pass. Privileged ops require untainted owner/authenticated origin."
  ;; Non-privileged ops: allow (harmonic routing still applies)
  (unless (member op *privileged-ops* :test #'string-equal)
    (return-from %policy-gate t))
  ;; Privileged ops: check origin
  (when (and originating-signal (harmonia-signal-p originating-signal))
    (let ((label (harmonia-signal-security-label originating-signal))
          (taint (harmonia-signal-taint originating-signal)))
      ;; External tainted signals cannot trigger privileged ops
      (when (member taint '(:external :tool-output :memory-recall))
        (%security-log :denied op originating-signal "tainted origin")
        (error "privileged operation ~A denied: tainted signal origin (~A)" op taint))
      ;; Only owner/authenticated can trigger privileged ops
      (unless (member label '(:owner :authenticated))
        (%security-log :denied op originating-signal "insufficient trust")
        (error "privileged operation ~A denied: security-label ~A" op label))))
  ;; Privileged operations that require admin intent must provide valid signature.
  (when (%admin-intent-required-p op)
    (unless prompt
      (%security-log :denied op originating-signal "missing prompt for admin-intent")
      (error "privileged operation ~A denied: missing admin-intent prompt context" op))
    (%validate-admin-intent op prompt originating-signal))
  (when (%trace-level-p :standard)
    (trace-event "policy-gate" :chain
                 :metadata (list :op op
                                 :allowed t
                                 :taint (and originating-signal
                                             (harmonia-signal-p originating-signal)
                                             (harmonia-signal-taint originating-signal))
                                 :security-label (and originating-signal
                                                      (harmonia-signal-p originating-signal)
                                                      (harmonia-signal-security-label originating-signal)))))
  (%security-log :allowed op originating-signal "passed")
  t)

;;; --- Wave 4.4: Invariant guards (hardcoded, non-configurable) ---

(defun %invariant-guard (op args-plist)
  "Reject mutations that would weaken security invariants, even with valid admin signature."
  (cond
    ;; Prevent setting vault edge min_harmony below 0.30
    ((and (string-equal op "matrix-set-edge")
          (string-equal (getf args-plist :to) "vault"))
     (let ((min-val (getf args-plist :min-harmony)))
       (when (and min-val (numberp min-val) (< min-val 0.30))
         (error "invariant guard: vault edge min_harmony cannot be set below 0.30 (got ~A)" min-val))))
    ;; Prevent setting dissonance-weight below 0.05
    ((and (string-equal op "harmony-policy-set")
          (search "dissonance-weight" (or (getf args-plist :path) "")))
     (let ((val (getf args-plist :value)))
       (when (and val (numberp val) (< val 0.05))
         (error "invariant guard: dissonance-weight cannot be set below 0.05 (got ~A)" val)))))
  t)

;;; --- Admin intent helpers ---

(defun %admin-intent-required-p (op)
  (let ((required (or (harmony-policy-ref "security/admin-intent-required-for" '()) '())))
    (or (member (intern (string-upcase op) :keyword) required)
        (member op required :test #'string-equal))))

(defun %admin-intent-params (prompt)
  "Canonical signing string: prompt tokens without leading `tool` and without `sig=`."
  (let ((tokens (%split-by-char (or prompt "") #\Space)))
    (format nil "~{~A~^ ~}"
            (remove-if (lambda (tok)
                         (or (string-equal tok "tool")
                             (%starts-with-p tok "sig=")))
                       tokens))))

(defun %validate-admin-intent (op prompt originating-signal)
  (let* ((sig (or (%extract-tag-value prompt "sig") ""))
         (ts-raw (or (%extract-tag-value prompt "ts") ""))
         (ts (handler-case (parse-integer ts-raw :junk-allowed nil) (error () nil)))
         (max-age-ms 300000)
         (age (if ts (abs (- (%unix-time-ms) ts)) most-positive-fixnum))
         (pubkey-symbol (or (harmony-policy-ref "security/admin-intent-pubkey-symbol"
                                                "admin-ed25519-pubkey")
                            "admin-ed25519-pubkey")))
    (unless (> (length sig) 0)
      (%security-log :denied op originating-signal "missing admin-intent signature")
      (error "privileged operation ~A denied: missing sig=<ed25519-hex>" op))
    (when (or (null ts) (> age max-age-ms))
      (%security-log :denied op originating-signal "stale/missing admin-intent timestamp")
      (error "privileged operation ~A denied: ts missing or older than ~D ms" op max-age-ms))
    (unless (admin-intent-verify-with-vault op (%admin-intent-params prompt) sig pubkey-symbol)
      (%security-log :denied op originating-signal "admin-intent signature invalid")
      (error "privileged operation ~A denied: invalid admin intent signature" op))
    t))
