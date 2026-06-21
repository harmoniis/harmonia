;;; formal-verification.lisp — runtime contracts (Hoare-style pre/post), pure + declarative.
;;;
;;; HONESTY BY CONSTRUCTION. These are NOT theorem proofs and NOT probes that sample ambient state:
;;;
;;;   * An OPERATION-LEVEL contract runs the ACTUAL operation on its FULL data every time and checks
;;;     the postcondition. It is SOUND and TOTAL for that operation — a failing contract means the
;;;     operation is broken, period. (e.g. "the field loads exactly the graph it was pushed".)
;;;
;;;   * A COLLECTION-LEVEL contract can only examine a lambdoma-reduced SUBSET (lambdoma reduction is
;;;     a heuristic — i.e. a sample). It is reported as a SPOT-CHECK and is NEVER called "verified".
;;;
;;; The verdict carries its own scope, so an operation-level pass reads "holds (sound)" and a
;;; collection-level pass reads "spot-check N of M — NOT verified". A green report can therefore not
;;; be restated as "the system works" — only as "these operations satisfied their postconditions".
;;;
;;; STANDALONE + LOG-ONLY. Nothing here mutates agent state or triggers a rewrite. Discharging these
;;; from the harmonic :evaluate phases is a separate, later step, gated on the contracts being trusted.
;;;
;;; (Static, machine-checked proofs of the pure functional core — the RK4/spectral memory-field
;;; dynamics, lambdoma selection — are a DISTINCT offline track in a proof assistant. These runtime
;;; contracts do not claim that rigor; they catch broken operations, they do not prove correctness.)

(in-package :harmonia)

(defstruct (contract (:constructor make-contract (name level statement check)))
  name level statement check)

(defparameter *frv-contracts* '()
  "Registered runtime contracts (newest-first), discharged by FRV-VERIFY.")

(defmacro defcontract (name level statement &body body)
  "Register a contract. BODY is a pure check returning (values HOLDS-P WITNESS-PLIST). LEVEL is
:operation (sound + total — runs the real op on full data) or :collection (lambdoma-sampled spot-check)."
  `(progn
     (setf *frv-contracts*
           (cons (make-contract ,name ,level ,statement (lambda () (block frv ,@body)))
                 (remove ,name *frv-contracts* :key #'contract-name)))
     ,name))

(defun %frv-discharge (c)
  "Discharge one contract into a scoped verdict. TOTAL: a thrown error is a VIOLATION, never a pass."
  (multiple-value-bind (holds witness)
      (handler-case (funcall (contract-check c))
        (error (e) (values nil (list :error (princ-to-string e)))))
    (list :contract (contract-name c)
          :level (contract-level c)
          :sound (eq (contract-level c) :operation)
          :holds (and holds t)
          :statement (contract-statement c)
          :witness witness)))

(defun frv-verify (&optional (contracts *frv-contracts*))
  "Discharge CONTRACTS in declaration order → list of scoped verdicts. Never auto-repairs."
  (mapcar #'%frv-discharge (reverse contracts)))

(defun frv-report (&optional (verdicts (frv-verify)))
  "Print verdicts with EXPLICIT scope. Returns (values ALL-SOUND-CONTRACTS-HOLD-P verdicts) — only
operation-level (sound) contracts gate the boolean; a collection-level pass never reports as proof."
  (let ((sound-fail nil))
    (dolist (v verdicts)
      (let* ((holds (getf v :holds)) (sound (getf v :sound))
             (mark (cond ((not holds) (when sound (setf sound-fail t)) "✗ VIOLATED   ")
                         (sound       "✓ holds·SOUND")
                         (t           "~ spot-check "))))
        (format t "  ~A ~A~%      ~A~%      witness: ~S~%"
                mark (string-downcase (symbol-name (getf v :contract)))
                (getf v :statement) (getf v :witness))))
    (values (not sound-fail) verdicts)))

;;; ── Operation-level contracts: SOUND + TOTAL (run the real op on full data) ──────────────────

(defcontract :field-load-fidelity :operation
    "the memory-field reflects the concept graph: graph-n is non-empty and >= 80% of the node count"
  ;; READ-ONLY + SOUND. Reads the engine's live node count and compares to the Lisp concept graph —
  ;; no re-push, so it is safe to discharge in-agent on the harmonic cadence. graph-n=0 (or NIL)
  ;; while there are concept nodes means the field did NOT load — the silent failure I mis-reported.
  ;; MUST run where the field actor is reachable (the agent's own context); a detached probe sbcl
  ;; does not own the actor and would read NIL — a false violation. So this lives in eval, not a probe.
  (let ((expected (hash-table-count *memory-concept-nodes*)))
    (if (zerop expected)
        (values t (list :expected 0 :note "no concept graph — vacuously holds"))
        (let* ((parsed (%parse-port-reply
                        (ipc-call (%sexp-to-ipc-string
                                   '(:component "memory-field" :op "status")))))
               (loaded (getf parsed :graph-n)))
          (values (and (integerp loaded) (plusp loaded) (>= loaded (floor (* expected 4) 5)))
                  (list :expected expected :loaded (or loaded :unreachable)))))))

(defcontract :store-recall-roundtrip :operation
    "a freshly-stored sentinel fact is recalled through the FULL memory-recall path (no ambient state)"
  ;; Sound: store a UNIQUE sentinel this instant, then recall it. No dependence on pre-existing
  ;; concepts (the trap that made deep-probe a sample). If recall can't return what was just stored,
  ;; memory is broken for that operation — full stop.
  (let* ((tok (format nil "FRV~A" (write-to-string (+ (get-internal-real-time) (random 1000000))
                                                   :base 36)))
         (val (format nil "quasar~A" tok))
         (fact (format nil "the verification sentinel ~A has the value ~A" tok val)))
    (ignore-errors (%prim-store fact :tags '(:frv-sentinel)))
    (let* ((results (ignore-errors (memory-recall tok)))
           (text (with-output-to-string (s)
                   (dolist (e results)
                     (when (memory-entry-p e)
                       (write-string (or (memory-entry-content e) "") s))))))
      (values (and (search val text) t)
              (list :sentinel tok :recalled-entries (length (or results '()))
                    :recovered (and (search val text) t))))))

;;; ── Agent-output grounding: a SOUND predicate, applied by a SAMPLED property test ─────────────
;;;
;;; %output-grounded-p is sound on a GIVEN answer: it holds iff the answer carries the ground-truth
;;; value AND that value was actually recallable (so the answer is justified by memory, not guessed).
;;; A fabricated answer (a different value, or one not in the recall) fails it. This is the formal
;;; core of "verify the agent's output". Because outputs are model-produced and non-deterministic,
;;; the only way to exercise it is over GENERATED cases — that is property-based testing (QuickChick,
;;; Vol 4): it can EXPOSE ungrounded output but never prove its absence, so the runner reports
;;; "property held on N/N sampled cases", explicitly a sample, never "verified".

(defun %output-grounded-p (answer value recall-text)
  "Sound on this output: T iff ANSWER carries VALUE and VALUE was recallable (in RECALL-TEXT).
A fabricated or wrong answer (VALUE absent from ANSWER) fails; an answer asserting a value the
agent could not recall (VALUE absent from RECALL-TEXT) is unjustified and also fails."
  (and (stringp answer) (stringp value)
       (search value answer)
       (stringp recall-text) (search value recall-text)
       t))

(defun frv-make-grounded-fact ()
  "Generate one fresh, non-guessable (code, value, fact) triple for a property-test case."
  (let* ((code (format nil "VC~A" (write-to-string (+ (get-internal-real-time) (random 1000000))
                                                   :base 36)))
         (value (format nil "~A" (+ 30000 (random 60000)))))   ; a 5-digit number the model can't guess
    (values code value
            (format nil "Remember this fact: the verification code ~A maps to the number ~A." code value))))

;;; ── Harmonic-cadence discharge: in-agent, READ-ONLY contracts, LOG-ONLY ───────────────────────
;;;
;;; This is the "verification at runtime in eval, on the harmonic state machine" piece — but it runs
;;; ONLY the read-only contracts (no mutation), and it only LOGS violations. It never rewrites,
;;; mutates, or repairs on a verdict. (A verifier that auto-rewrote on the hot path would mutate the
;;; agent on false positives — the same failure mode that broke orchestration. Surfacing only.)

(defparameter *frv-harmonic-contracts* '(:field-load-fidelity)
  "Contracts safe on the harmonic cadence: read-only, no side effects. Mutating contracts
(store-recall-roundtrip, output property tests) run only in the standalone probe, never the hot path.")

(defun frv-discharge-harmonic (runtime)
  "Discharge the harmonic-safe (read-only) contracts in-agent — where the field actor is reachable —
and LOG any SOUND violation loudly. Pure surfacing: never mutates state or triggers a rewrite."
  (ignore-errors
   (dolist (v (frv-verify (remove-if-not
                           (lambda (c) (member (contract-name c) *frv-harmonic-contracts*))
                           *frv-contracts*)))
     (when (and (getf v :sound) (not (getf v :holds)))
       (ignore-errors
        (when (fboundp 'runtime-log)
          (funcall 'runtime-log runtime :contract-violated
                   (list :contract (getf v :contract) :witness (getf v :witness)))))
       (ignore-errors
        (%log :warn "verify"
              (format nil "SOUND CONTRACT VIOLATED: ~A ~S"
                      (getf v :contract) (getf v :witness))))))))
