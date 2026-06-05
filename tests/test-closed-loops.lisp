;;; test-closed-loops.lisp — Deterministic tests for the dynamics closed-loop wires.
;;;
;;; Verifies the four loops that were unwired before the systemic fix:
;;;   1. Signalograd projection broadcast and apply.
;;;   2. Memory-field basin / dream broadcast handlers.
;;;   3. Harmony :logistic-r-delta clamp via sanitize-proposal.
;;;   4. %step-logistic actually moves runtime-state-harmonic-r.
;;;
;;; Runs without LLM, without IPC, without Rust runtime — pure Lisp determinism.

(in-package :harmonia)

(unless (boundp '*test-pass*) (defparameter *test-pass* 0))
(unless (boundp '*test-fail*) (defparameter *test-fail* 0))

(defmacro closed-loop-assert (name expr)
  `(handler-case
       (if ,expr
           (progn (incf *test-pass*) (format t "  ✓ ~A~%" ,name))
           (progn (incf *test-fail*) (format t "  ✗ ~A — assertion failed~%" ,name)))
     (error (c)
       (incf *test-fail*) (format t "  ✗ ~A — error: ~A~%" ,name c))))

(defun %close-to (a b &optional (eps 1.0e-6))
  (< (abs (- a b)) eps))

(defun run-closed-loop-tests ()
  (setf *test-pass* 0 *test-fail* 0)
  (format t "~%═══ CLOSED-LOOP WIRING TESTS ══════════════════════════~%")

  ;; ── Sanitize-proposal: logistic-r-delta clamp ─────────────────────
  (format t "~%── signalograd sanitize: logistic-r-delta ──~%")

  (let ((sanitized (%signalograd-sanitize-proposal
                    '(:cycle 1 :confidence 0.5
                      :harmony (:logistic-r-delta 0.5)))))
    (closed-loop-assert
        "logistic-r-delta clamped to harmony/logistic-r-delta-max (default 0.02)"
        (%close-to (getf (getf sanitized :harmony) :logistic-r-delta) 0.02)))

  (let ((sanitized (%signalograd-sanitize-proposal
                    '(:cycle 1 :harmony (:logistic-r-delta -0.7)))))
    (closed-loop-assert "negative delta clamped symmetrically to -0.02"
        (%close-to (getf (getf sanitized :harmony) :logistic-r-delta) -0.02)))

  (let ((sanitized (%signalograd-sanitize-proposal
                    '(:cycle 1 :harmony (:logistic-r-delta 0.005)))))
    (closed-loop-assert "in-range delta passes through"
        (%close-to (getf (getf sanitized :harmony) :logistic-r-delta) 0.005)))

  (let ((sanitized (%signalograd-sanitize-proposal '(:cycle 1))))
    (closed-loop-assert "missing harmony section yields zero delta"
        (%close-to (getf (getf sanitized :harmony) :logistic-r-delta) 0.0)))

  ;; ── signalograd-apply-proposal end-to-end ─────────────────────────
  (format t "~%── signalograd-apply-proposal stores projection ──~%")

  (let* ((rt (make-runtime-state-fresh))
         (proposal '(:signalograd-proposal :cycle 17 :confidence 0.62
                     :stability 0.5 :novelty 0.3
                     :harmony (:logistic-r-delta 0.011))))
    (signalograd-apply-proposal proposal :runtime rt)
    (closed-loop-assert "projection stored after apply"
        (and (listp (runtime-state-signalograd-projection rt))
             (= 17 (getf (runtime-state-signalograd-projection rt) :cycle))))
    (closed-loop-assert "signalograd-logistic-r-delta reads from runtime"
        (%close-to (signalograd-logistic-r-delta rt) 0.011)))

  (let ((rt (make-runtime-state-fresh)))
    (closed-loop-assert "signalograd-logistic-r-delta defaults to 0 with no projection"
        (%close-to (signalograd-logistic-r-delta rt) 0.0)))

  ;; ── %step-logistic actually moves r ───────────────────────────────
  (format t "~%── %step-logistic applies r delta ──~%")

  (let ((rt (make-runtime-state-fresh)))
    (signalograd-apply-proposal
     '(:signalograd-proposal :cycle 1 :harmony (:logistic-r-delta 0.01))
     :runtime rt)
    (let ((r0 (runtime-state-harmonic-r rt)))
      (let ((*runtime* rt)) (%step-logistic rt))
      (closed-loop-assert "r moved by the applied delta"
          (%close-to (runtime-state-harmonic-r rt) (+ r0 0.01)))))

  (let ((rt (make-runtime-state-fresh)))
    ;; No projection applied → delta = 0 → r unchanged.
    (let ((r0 (runtime-state-harmonic-r rt)))
      (let ((*runtime* rt)) (%step-logistic rt))
      (closed-loop-assert "r unchanged when no projection is present"
          (%close-to (runtime-state-harmonic-r rt) r0))))

  (let ((rt (make-runtime-state-fresh)))
    ;; Drive r toward the upper edge with many positive deltas; verify clamp.
    (signalograd-apply-proposal
     '(:signalograd-proposal :cycle 1 :harmony (:logistic-r-delta 0.02))
     :runtime rt)
    (loop repeat 200 do (let ((*runtime* rt)) (%step-logistic rt)))
    (let ((edge 3.56995) (window 0.4))
      (closed-loop-assert "r clamped to upper bound (edge + window)"
          (%close-to (runtime-state-harmonic-r rt) (+ edge window)))))

  (let ((rt (make-runtime-state-fresh)))
    (signalograd-apply-proposal
     '(:signalograd-proposal :cycle 1 :harmony (:logistic-r-delta -0.02))
     :runtime rt)
    (loop repeat 200 do (let ((*runtime* rt)) (%step-logistic rt)))
    (let ((edge 3.56995) (window 0.4))
      (closed-loop-assert "r clamped to lower bound (edge - window)"
          (%close-to (runtime-state-harmonic-r rt) (- edge window)))))

  ;; ── Memory-field signals plumbed into observation packet ────────
  (format t "~%── observation packet carries field + palace signals ──~%")

  (closed-loop-assert "field-stability is 0 with no basin info"
      (%close-to (%signalograd-field-stability '()) 0.0))

  (closed-loop-assert "field-stability saturates as dwell grows"
      (let ((s (%signalograd-field-stability
                '(:field-basin (:dwell-ticks 200)))))
        (and (> s 0.85) (<= s 1.0))))

  (closed-loop-assert "field-stability mid-range at dwell=20"
      (%close-to (%signalograd-field-stability
                  '(:field-basin (:dwell-ticks 20))) 0.5 0.01))

  (closed-loop-assert "palace-density returns 0 when port offline"
      (%close-to (%signalograd-palace-density) 0.0))

  (let* ((rt (make-runtime-state-fresh))
         (ctx '(:cycle 1
                :field-basin (:dwell-ticks 60)
                :map (:concept-nodes nil :concept-edges nil)))
         (sexp (%signalograd-observation-sexp ctx rt)))
    (closed-loop-assert ":field-basin-stability appears in observation sexp"
        (search ":field-basin-stability" sexp))
    (closed-loop-assert ":field-recall-strength appears in observation sexp"
        (search ":field-recall-strength" sexp))
    (closed-loop-assert ":field-eigenmode-coherence appears in observation sexp"
        (search ":field-eigenmode-coherence" sexp))
    (closed-loop-assert ":datamine-success-rate appears in observation sexp"
        (search ":datamine-success-rate" sexp))
    (closed-loop-assert ":datamine-avg-latency appears in observation sexp"
        (search ":datamine-avg-latency" sexp))
    (closed-loop-assert ":palace-graph-density appears in observation sexp"
        (search ":palace-graph-density" sexp)))

  (closed-loop-assert "field-coherence is 0 when port offline"
      (%close-to (%signalograd-field-coherence) 0.0))

  (closed-loop-assert "datamine-stats returns plist with both keys when offline"
      (let ((s (%signalograd-datamine-stats)))
        (and (%close-to (getf s :success-rate) 0.0)
             (%close-to (getf s :avg-latency-ms) 0.0))))

  ;; ── Palace filing grows graph structure ─────────────────────────────
  (format t "~%── palace memory filing builds graph ──~%")

  (let ((old-ready *mempalace-ready*)
        (old-add-node (symbol-function 'palace-add-node))
        (old-add-edge (symbol-function 'palace-add-edge))
        (old-file-drawer (symbol-function 'palace-file-drawer)))
    (unwind-protect
         (let ((node-seq 0)
               (node-ids (make-hash-table :test 'equal))
               (nodes '())
               (edges '())
               (drawers '()))
           (setf *mempalace-ready* t)
           (setf (symbol-function 'palace-add-node)
                 (lambda (kind label domain)
                   (let ((id (or (gethash label node-ids)
                                 (setf (gethash label node-ids)
                                       (prog1 node-seq (incf node-seq))))))
                     (push (list kind label domain id) nodes)
                     (list :id id :kind kind :label label :domain domain))))
           (setf (symbol-function 'palace-add-edge)
                 (lambda (source target kind weight)
                   (push (list source target kind weight) edges)
                   (list :source source :target target :kind kind :weight weight)))
           (setf (symbol-function 'palace-file-drawer)
                 (lambda (content room-id &key tags)
                   (push (list content room-id tags) drawers)
                   (list :id 1 :room room-id :size (length content))))
           (%palace-file-memory-entry
            :daily
            "Music harmony meets Lisp Rust code inside memory dream recall."
            :tags '(:interaction)
            :concepts '("music" "harmony" "lisp" "rust" "memory" "dream"))
           (closed-loop-assert "palace filing creates at least one wing node"
               (find "wing" nodes :key #'first :test #'string=))
           (closed-loop-assert "palace filing creates concept nodes"
               (find "concept" nodes :key #'first :test #'string=))
           (closed-loop-assert "palace filing creates a tunnel node for cross-domain memory"
               (find "tunnel" nodes :key #'first :test #'string=))
           (closed-loop-assert "palace filing creates typed edges"
               (and edges (find "contains" edges :key #'third :test #'string=)))
           (closed-loop-assert "palace filing writes one drawer"
               (= 1 (length drawers))))
      (setf (symbol-function 'palace-add-node) old-add-node)
      (setf (symbol-function 'palace-add-edge) old-add-edge)
      (setf (symbol-function 'palace-file-drawer) old-file-drawer)
      (setf *mempalace-ready* old-ready)))

  ;; ── REPL recall returns concise stored facts ──────────────────────
  (format t "~%── REPL recall is concise and direct ──~%")

  (let ((field (%prim-field)))
    (closed-loop-assert "field is concise declarative context"
        (and (stringp field)
             (search "(:FIELD " field)
             (search ":PALACE " field)
             (<= (length field) 240)
             (null (position #\Newline field))
             (null (search "GLOBAL CONTEXT:" field))
             (null (search "CHAIN:" field)))))

  (let ((status (%prim-status)))
    (closed-loop-assert "runtime status includes live palace readiness"
        (and (stringp status)
             (search "(:STATUS " status)
             (search ":PALACE " status)
             (null (position #\Newline status))
             (<= (length status) 240))))

  (let ((frame (%compute-repl-frame)))
    (closed-loop-assert "REPL frame has no copyable answer placeholder"
        (null (search "your answer" frame :test #'char-equal)))
    (closed-loop-assert "REPL frame teaches complete field response"
        (search "(respond (field))" frame :test #'char-equal)))

  (let ((*memory-concept-nodes* (make-hash-table :test 'equal))
        (*memory-concept-edges* (make-hash-table :test 'equal))
        (*memory-concept-directed-counts* (make-hash-table :test 'equal))
        (*memory-store* (make-hash-table :test 'equal)))
    (%index-entry-concepts "ENTRY-1" :daily 0 "memory agent"
                           :tags '(:user-stored))
    (let* ((nodes (%serialize-field-nodes))
           (edges (%serialize-field-edges))
           (access-counts (%field-access-counts '("memory" "agent")))
           (command (%sexp-to-ipc-string
                     `(:component "memory-field" :op "load-graph"
                       :nodes ,nodes :edges ,edges)))
           (recall-command (%sexp-to-ipc-string
                            `(:component "memory-field" :op "field-recall"
                              :query-concepts ("memory" "agent")
                              :access-counts ,access-counts
                              :limit 2))))
      (closed-loop-assert "memory-field graph command carries nested node data once"
          (and (listp nodes)
               (listp (first nodes))
               (search ":nodes ((:concept " command)
               (search ":concept \"memory\"" command)
               (search ":domain \"cognitive\"" command)
               (null (search ":nodes \"(" command))
               (null (search "\\\"memory\\\"" command))))
      (closed-loop-assert "memory-field graph command carries nested edge data once"
          (and (listp edges)
               (listp (first edges))
               (search ":edges ((:a " command)
               (null (search ":edges \"(" command))))
      (closed-loop-assert "memory-field recall command carries nested query data once"
          (and (listp access-counts)
               (search ":query-concepts (\"memory\" \"agent\")" recall-command)
               (search ":access-counts ((:concept " recall-command)
               (null (search ":query-concepts \"(" recall-command))
               (null (search ":access-counts \"(" recall-command))))))

  ;; ── Chronicle actor protocol and REPL completion accounting ──────
  (format t "~%── delegation completion accounting ──~%")

  (let ((old-ipc-call (symbol-function 'ipc-call))
        (commands '()))
    (unwind-protect
         (progn
           (setf (symbol-function 'ipc-call)
                 (lambda (command)
                   (push command commands)
                   "(:ok)"))
           (chronicle-record-delegation
            :task-hint "general" :model "test/free" :backend "repl"
            :escalated nil :success t)
           (chronicle-record-harmonic
            '(:plan (:state-machine :observe :ready t)
              :projection (:convergent-p t)))
           (chronicle-record-signalograd-event "test" :accepted t)
           (chronicle-record-ouroboros-event "test" :success t)
           (let ((delegation (find-if (lambda (command)
                                        (search ":op \"record-delegation\"" command))
                                      commands))
                 (harmonic (find-if (lambda (command)
                                      (search ":op \"record-harmonic\"" command))
                                    commands))
                 (signalograd (find-if (lambda (command)
                                         (search ":op \"record-signalograd-event\"" command))
                                       commands))
                 (ouroboros (find-if (lambda (command)
                                       (search ":op \"record-ouroboros-event\"" command))
                                     commands)))
             (closed-loop-assert "chronicle delegation uses the actor's model-chosen key"
                 (and delegation
                      (search ":model-chosen \"test/free\"" delegation)
                      (null (search ":model \"test/free\"" delegation))))
             (closed-loop-assert "chronicle delegation serializes success and escalation as booleans"
                 (and (search ":success t" delegation)
                      (search ":escalated nil" delegation)))
             (closed-loop-assert "chronicle actor booleans serialize as t/nil across record operations"
                 (and (search ":lambdoma-convergent t" harmonic)
                      (search ":rewrite-ready t" harmonic)
                      (search ":accepted t" signalograd)
                      (search ":success t" ouroboros)))))
      (setf (symbol-function 'ipc-call) old-ipc-call)))

  (let ((old-backend (symbol-function 'backend-complete))
        (old-memory-put (symbol-function 'memory-put))
        (old-select-model (symbol-function '%select-model))
        (old-model-outcome (symbol-function 'model-policy-record-outcome))
        (old-delegation (symbol-function 'chronicle-record-delegation))
        (model-outcomes '())
        (delegations '())
        (*pipeline-trace-enabled* nil)
        (*repl-frame* nil)
        (*repl-model-perf* (make-hash-table :test 'equal)))
    (unwind-protect
         (progn
           (setf (symbol-function 'memory-put)
                 (lambda (&rest args) (declare (ignore args)) t))
           (setf (symbol-function '%select-model)
                 (lambda (prompt) (declare (ignore prompt)) "test/free"))
           (setf (symbol-function 'model-policy-record-outcome)
                 (lambda (&rest args) (push args model-outcomes) t))
           (setf (symbol-function 'chronicle-record-delegation)
                 (lambda (&rest args) (push args delegations) t))

           (setf (symbol-function 'backend-complete)
                 (lambda (prompt &optional model)
                   (declare (ignore prompt model))
                   "(respond \"ACCOUNTED-RESPOND\")"))
           (multiple-value-bind (reply outcome)
               (%orchestrate-repl "Complete with respond." :max-rounds 1)
             (closed-loop-assert "respond completion returns the answer"
                 (string= "ACCOUNTED-RESPOND" reply))
             (closed-loop-assert "respond completion exposes recorded outcome metadata"
                 (and (getf outcome :outcome-recorded-p)
                      (getf outcome :success)
                      (string= "test/free" (getf outcome :model)))))

           (setf (symbol-function 'backend-complete)
                 (lambda (prompt &optional model)
                   (declare (ignore prompt model))
                   "ACCOUNTED-NATURAL"))
           (multiple-value-bind (reply outcome)
               (%orchestrate-repl "Complete naturally." :max-rounds 1)
             (closed-loop-assert "natural completion returns the answer"
                 (string= "ACCOUNTED-NATURAL" reply))
             (closed-loop-assert "natural completion uses the same success-accounting path"
                 (and (getf outcome :outcome-recorded-p)
                      (getf outcome :success))))

           (setf (symbol-function 'backend-complete)
                 (lambda (prompt &optional model)
                   (declare (ignore prompt model))
                   nil))
           (multiple-value-bind (reply outcome)
               (%orchestrate-repl "Fail cleanly." :max-rounds 1)
             (closed-loop-assert "unavailable completion returns a graceful answer"
                 (%repl-usable-response-p reply))
             (closed-loop-assert "unavailable completion records failure, not success"
                 (and (getf outcome :outcome-recorded-p)
                      (not (getf outcome :success)))))

           (closed-loop-assert "each REPL completion records model policy exactly once"
               (= 3 (length model-outcomes)))
           (closed-loop-assert "each REPL completion records chronicle delegation exactly once"
               (= 3 (length delegations)))
           (closed-loop-assert "REPL completion success signal reaches both routing sinks"
               (let ((model-successes (mapcar (lambda (args) (getf args :success))
                                              (reverse model-outcomes)))
                     (chronicle-successes (mapcar (lambda (args) (getf args :success))
                                                  (reverse delegations))))
                 (and (equal model-successes '(t t nil))
                      (equal chronicle-successes '(t t nil))))))
      (setf (symbol-function 'backend-complete) old-backend)
      (setf (symbol-function 'memory-put) old-memory-put)
      (setf (symbol-function '%select-model) old-select-model)
      (setf (symbol-function 'model-policy-record-outcome) old-model-outcome)
      (setf (symbol-function 'chronicle-record-delegation) old-delegation)))

  (let ((old-field-ready (symbol-function 'memory-field-port-ready-p))
        (old-field-recall (symbol-function 'memory-field-recall))
        (old-palace-ready (symbol-function 'mempalace-port-ready-p))
        (*memory-store* (make-hash-table :test 'equal))
        (*pipeline-trace-enabled* nil)
        (query "retrieve exact zephyr fact 7741")
        (fact "RETRIEVE-EXACT-ZEPHYR-FACT-7741"))
    (unwind-protect
         (progn
           (setf (gethash "field-transcript" *memory-store*)
                 (make-memory-entry
                  :id "field-transcript" :time 20 :class :daily :depth 0
                  :content
                  (format nil "Q: ~A~%A: STALE-FIELD-ANSWER" query)
                  :tags '(:interaction)))
           (setf (gethash "stored-fact" *memory-store*)
                 (make-memory-entry
                  :id "stored-fact" :time 10 :class :daily :depth 0
                  :content fact :tags '(:user-stored)))
           (setf (symbol-function 'memory-field-port-ready-p) (lambda () t))
           (setf (symbol-function 'memory-field-recall)
                 (lambda (q &key limit)
                   (declare (ignore q limit))
                   '(:activations ((:score 0.99 :entries ("field-transcript"))))))
           (setf (symbol-function 'mempalace-port-ready-p) (lambda () nil))
           (let ((candidates (memory-recall query :limit 2)))
             (closed-loop-assert "memory recall unions field and lexical candidates"
                 (and (find "field-transcript" candidates
                            :key #'memory-entry-id :test #'string=)
                      (find "stored-fact" candidates
                            :key #'memory-entry-id :test #'string=))))
           (closed-loop-assert "exact stored fact survives a weak field hit"
               (string= fact (%prim-recall query))))
      (setf (symbol-function 'memory-field-port-ready-p) old-field-ready)
      (setf (symbol-function 'memory-field-recall) old-field-recall)
      (setf (symbol-function 'mempalace-port-ready-p) old-palace-ready)))

  (let ((old-field-ready (symbol-function 'memory-field-port-ready-p))
        (old-palace-ready (symbol-function 'mempalace-port-ready-p))
        (*memory-store* (make-hash-table :test 'equal))
        (*pipeline-trace-enabled* nil))
    (unwind-protect
         (progn
           (setf (gethash "short-code" *memory-store*)
                 (make-memory-entry
                  :id "short-code" :time 10 :class :daily :depth 0
                  :content "X7" :tags '(:user-stored)))
           (setf (symbol-function 'memory-field-port-ready-p) (lambda () nil))
           (setf (symbol-function 'mempalace-port-ready-p) (lambda () nil))
           (closed-loop-assert "short alphanumeric codes remain recallable"
               (string= "X7" (%prim-recall "X7"))))
      (setf (symbol-function 'memory-field-port-ready-p) old-field-ready)
      (setf (symbol-function 'mempalace-port-ready-p) old-palace-ready)))

  (let ((old-memory-recall (symbol-function 'memory-recall))
        (token "E2E-TUI-EXACT-42"))
    (unwind-protect
         (progn
           (setf (symbol-function 'memory-recall)
                 (lambda (query &key limit)
                   (declare (ignore query limit))
                   (list
                    (make-memory-entry
                     :id "interaction"
                     :time 20
                     :class :interaction
                     :depth 0
                     :content
                     (format nil "Q: retrieve token E2E-TUI-EXACT~%A: ~A" token)
                     :tags '(:repl :interaction))
                    (make-memory-entry
                     :id "stored"
                     :time 10
                     :class :daily
                     :depth 0
                     :content token
                     :tags '(:user-stored))
                    (make-memory-entry
                     :id "older-stored"
                     :time 5
                     :class :daily
                     :depth 0
                     :content "E2E-TUI-EXACT-OLD"
                     :tags '(:user-stored)))))
           (let ((result (%prim-recall "retrieve token E2E-TUI-EXACT")))
             (closed-loop-assert "explicit stored fact outranks newer interaction transcript"
                 (string= token result))
             (closed-loop-assert "newest equally relevant stored fact wins"
                 (string= token result))
             (closed-loop-assert "REPL recall does not leak orchestration context labels"
                 (null (search "MEMORY_RECALL:" result)))))
      (setf (symbol-function 'memory-recall) old-memory-recall)))

  (let* ((authoritative
           (make-memory-entry
            :id "memory-new" :time 20 :class :daily :depth 0
            :content "the probe code is ZQ-NEW" :tags '(:user-stored)))
         (older
           (make-memory-entry
            :id "memory-old" :time 10 :class :daily :depth 0
            :content "the probe code is ZQ-OLD" :tags '(:user-stored)))
         (palace-copy
           (make-memory-entry
            :id "palace:new" :class :palace :depth 0
            :content "the probe code is ZQ-NEW" :tags '("user-stored")))
         (ranked (%rank-recall-entries
                  "probe code"
                  (list authoritative older palace-copy))))
    (closed-loop-assert "cross-source duplicate preserves authoritative memory metadata"
        (eq authoritative (first ranked))))

  (let ((old-memory-recall (symbol-function 'memory-recall))
        (old-ready (symbol-function 'mempalace-port-ready-p))
        (old-search (symbol-function 'palace-search))
        (old-get (symbol-function 'palace-get-drawer)))
    (unwind-protect
         (progn
           (setf (symbol-function 'memory-recall)
                 (lambda (query &key limit)
                   (declare (ignore query limit))
                   nil))
           (setf (symbol-function 'mempalace-port-ready-p) (lambda () t))
           (setf (symbol-function 'palace-search)
                 (lambda (query &key limit)
                   (declare (ignore query limit))
                   '(:count 1 :results ((:id 7 :preview "palace-only")))))
           (setf (symbol-function 'palace-get-drawer)
                 (lambda (id)
                   (declare (ignore id))
                   '(:content "PALACE-ONLY-FACT" :tags ("manual"))))
           (closed-loop-assert "REPL recall preserves palace-only drawer knowledge"
               (string= "PALACE-ONLY-FACT" (%prim-recall "PALACE-ONLY"))))
      (setf (symbol-function 'memory-recall) old-memory-recall)
      (setf (symbol-function 'mempalace-port-ready-p) old-ready)
      (setf (symbol-function 'palace-search) old-search)
      (setf (symbol-function 'palace-get-drawer) old-get)))

  (let ((old-backend (symbol-function 'backend-complete))
        (old-memory-recall (symbol-function 'memory-recall))
        (old-memory-put (symbol-function 'memory-put))
        (old-ready *mempalace-ready*)
        (*pipeline-trace-enabled* nil)
        (*repl-frame* nil)
        (*repl-model-perf* (make-hash-table :test 'equal))
        (token "RECALL-LEAK-ZEPHYR-7741"))
    (unwind-protect
         (progn
           (setf *mempalace-ready* nil)
           (setf (symbol-function 'backend-complete)
                 (lambda (prompt &optional model)
                   (declare (ignore prompt model))
                   "(respond (recall \"RECALL-LEAK-ZEPHYR-7741\"))"))
           (setf (symbol-function 'memory-recall)
                 (lambda (query &key limit)
                   (declare (ignore query limit))
                   (list
                    (make-memory-entry
                     :id "interaction"
                     :time 20
                     :class :interaction
                     :depth 0
                     :content
                     (format nil "Q: What is the recall-leak code?~%A: ~A" token)
                     :tags '(:repl :interaction))
                    (make-memory-entry
                     :id "stored"
                     :time 10
                     :class :daily
                     :depth 0
                     :content token
                     :tags '(:user-stored)))))
           (setf (symbol-function 'memory-put)
                 (lambda (&rest args) (declare (ignore args)) t))
           (let ((reply (%orchestrate-repl
                         "What is the recall-leak code? Answer with only the code."
                         :max-rounds 1)))
             (closed-loop-assert "full REPL recall path returns the exact stored fact"
                 (string= token reply))
             (closed-loop-assert "full REPL recall path hides internal recall envelope"
                 (null (search "MEMORY_RECALL:" reply)))
             (closed-loop-assert "full REPL recall path remains concise"
                 (<= (length reply) 240))))
      (setf (symbol-function 'backend-complete) old-backend)
      (setf (symbol-function 'memory-recall) old-memory-recall)
      (setf (symbol-function 'memory-put) old-memory-put)
      (setf *mempalace-ready* old-ready)))

  (let ((old-backend (symbol-function 'backend-complete))
        (old-memory-put (symbol-function 'memory-put))
        (old-ready *mempalace-ready*)
        (*pipeline-trace-enabled* nil)
        (*repl-frame* nil)
        (*repl-model-perf* (make-hash-table :test 'equal)))
    (unwind-protect
         (progn
           (setf *mempalace-ready* nil)
           (setf (symbol-function 'memory-put)
                 (lambda (&rest args) (declare (ignore args)) t))
           (setf (symbol-function 'backend-complete)
                 (lambda (prompt &optional model)
                   (declare (ignore model))
                   (if (search "repeated form" prompt)
                       "(respond \"REPL-RECOVERED\")"
                       "(field)")))
           (closed-loop-assert "repeated observation is rejected and the REPL recovers"
               (string= "REPL-RECOVERED"
                        (%orchestrate-repl "Complete this request." :max-rounds 3)))
           (setf (symbol-function 'backend-complete)
                 (lambda (prompt &optional model)
                   (declare (ignore prompt model))
                   "(field)"))
           (let ((reply (%orchestrate-repl "Inspect the field." :max-rounds 2)))
             (closed-loop-assert "explicit observation request may return concise field context"
                 (and (stringp reply)
                      (search "(:FIELD " reply)
                      (<= (length reply) 240))))
           (let ((reply (%orchestrate-repl "Complete this request." :max-rounds 2)))
             (closed-loop-assert "observation-only exhaustion never leaks field context"
                 (and (stringp reply)
                      (null (search "(:FIELD " reply))
                      (not (%error-form-p reply))))))
      (setf (symbol-function 'backend-complete) old-backend)
      (setf (symbol-function 'memory-put) old-memory-put)
      (setf *mempalace-ready* old-ready)))

  ;; ── Chaos-risk varies as r evolves ────────────────────────────────
  (format t "~%── chaos-risk varies with r ──~%")

  (let ((rt-near (make-runtime-state-fresh))
        (rt-far  (make-runtime-state-fresh)))
    (setf (runtime-state-harmonic-r rt-near) 3.55) ; near edge
    (setf (runtime-state-harmonic-r rt-far)  3.20) ; far from edge
    (let ((cr-near (let ((*runtime* rt-near)) (getf (%step-logistic rt-near) :chaos-risk)))
          (cr-far  (let ((*runtime* rt-far))  (getf (%step-logistic rt-far)  :chaos-risk))))
      (closed-loop-assert "chaos-risk near edge is high"
          (> cr-near 0.9))
      (closed-loop-assert "chaos-risk far from edge is low"
          (< cr-far 0.1))))

  (format t "~%───────────────────────────────────────────────────────~%")
  (format t "PASS: ~D    FAIL: ~D~%" *test-pass* *test-fail*)
  (values *test-pass* *test-fail*))

(defun make-runtime-state-fresh ()
  "Construct a minimal runtime-state for deterministic test runs."
  (make-runtime-state))
