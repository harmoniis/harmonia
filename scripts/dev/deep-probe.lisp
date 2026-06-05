;;;; deep-probe.lisp — exhaustive live instrumentation of every signal/chain.
;;;; Run headless against a LIVE runtime:
;;;;   sbcl --load src/core/boot.lisp --eval '(harmonia:start :run-loop nil)' \
;;;;        --load scripts/dev/deep-probe.lisp --eval '(harmonia::deep-probe-run)'
;;;; Prints [PROBE]/[PASS]/[FAIL] lines + a final tally. Read-only except where noted.

(in-package :harmonia)

(defparameter *probe-pass* 0)
(defparameter *probe-fail* 0)

(defun %pp (label thunk)
  "Run THUNK, print its (clipped) result. Never aborts the run."
  (handler-case
      (let* ((r (funcall thunk)) (s (princ-to-string r)))
        (format t "~&[PROBE] ~A => ~A~%" label
                (if (> (length s) 320) (concatenate 'string (subseq s 0 320) " …") s))
        r)
    (error (c) (format t "~&[PROBE] ~A => ERROR: ~A~%" label c) nil)))

(defun %assert (label ok)
  (if ok (progn (incf *probe-pass*) (format t "  [PASS] ~A~%" label))
         (progn (incf *probe-fail*) (format t "  [FAIL] ~A~%" label))))

(defun %reply-raw (reply)
  "Parse an IPC reply sexp-string into a list (or pass through a list)."
  (cond ((stringp reply) (ignore-errors (let ((*read-eval* nil)) (read-from-string reply))))
        ((listp reply) reply)
        (t nil)))

(defun %reply-plist (reply)
  "Reply shape is (:ok KEY VAL …) or (:ok :result \"<sexp-string>\"). Drop the
   leading status token and, when the body is (:result \"…\"), parse that nested
   sexp string — signalograd status/snapshot wrap their payload that way."
  (let* ((raw (%reply-raw reply))
         (body (if (and (consp raw) (member (car raw) '(:ok :error))) (cdr raw) raw)))
    (if (and (consp body) (eq (car body) :result) (stringp (cadr body)))
        (%reply-raw (cadr body))
        body)))

(defun %num (reply key)
  (ignore-errors
    (let ((pl (%reply-plist reply)))
      (and (listp pl) (let ((v (getf pl key))) (and (realp v) v))))))

(defun %reply-value (reply key)
  (ignore-errors
    (let ((pl (%reply-plist reply)))
      (and (listp pl) (getf pl key)))))

(defun %in (x lo hi) (and (realp x) (<= lo x hi)))

(defun deep-probe-run ()
  (setf *probe-pass* 0 *probe-fail* 0)
  (format t "~%════════ HARMONIA DEEP PROBE ════════~%")

  ;; ─────────────────────────── EVAL / DECISION HARNESS ───────────────────────────
  (format t "~%── EVAL / DECISION HARNESS ──~%")
  (dolist (p '("implement a REST endpoint and write tests"
               "summarize this conversation and store it"
               "what is the current state of quantum computing in 2026?"
               "ocr this image and extract the text"
               "what do you think about my idea?"))
    (%pp (format nil "task-kind [~A…]" (subseq p 0 (min 28 (length p))))
         (lambda () (%task-kind p))))
  (let* ((w (%pp "task-weights :software-dev" (lambda () (%task-weights :software-dev))))
         (sum (and (listp w) (loop for (k v) on w by #'cddr when (realp v) sum v))))
    (%assert "task-weights sum ≈ 1.0" (and sum (%in sum 0.95 1.05))))
  (dolist (tier '(:auto :eco :premium :free))
    (let ((pool (%pp (format nil "tier-pool ~A" tier) (lambda () (%tier-model-pool tier)))))
      (%assert (format nil "tier ~A non-empty" tier) (and (listp pool) (plusp (length pool))))))
  (%pp "choose-model(software prompt)" (lambda () (choose-model "implement a REST endpoint")))
  (%pp "choose-model(casual prompt)"   (lambda () (choose-model "hello how are you")))
  ;; P4: escalation never retries the failed model and reaches the premium tier.
  (let* ((failed (choose-model "fix a bug"))
         (chain (%pp "escalation-chain (after failed default)"
                     (lambda () (model-escalation-chain "fix a bug" failed))))
         (premium (%tier-model-pool :premium)))
    (%assert "escalation-chain returns a non-empty list" (and (listp chain) chain))
    (%assert "escalation-chain never retries the failed model"
             (not (member failed chain :test #'string=)))
    (%assert "escalation-chain reaches the premium tier"
             (or (null premium) (intersection chain premium :test #'string=))))

  ;; ─────────────────────────── L1 MEMORY-FIELD DYNAMICS ───────────────────────────
  (format t "~%── L1 MEMORY-FIELD (attractor / heat-kernel / topology) ──~%")
  (%pp "field status" (lambda () (ipc-call "(:component \"memory-field\" :op \"status\")")))
  (let ((b (%pp "basin-status" (lambda () (ipc-call "(:component \"memory-field\" :op \"basin-status\")")))))
    (let ((e (%num b :coercive-energy)) (th (%num b :threshold)))
      (%assert "basin coercive-energy in [0,5]" (%in e 0.0 5.0))
      (%assert "basin threshold in [0.30,0.60]" (%in th 0.30 0.60))))
  ;; Step the attractor 25× and assert bounded, non-NaN evolution.
  (let ((bounded t) (moved nil) (prevx nil))
    (dotimes (i 25)
      (let* ((r (ipc-call "(:component \"memory-field\" :op \"step-attractors\" :signal 0.75 :noise 0.2)"))
             (pl (%reply-plist r))
             (th (and (listp pl) (getf pl :thomas)))
             (x (and (listp th) (getf th :x))) (y (and (listp th) (getf th :y)))
             (z (and (listp th) (getf th :z))) (bb (and (listp th) (getf th :b))))
        (when (and (realp x) (realp y) (realp z))
          (unless (and (%in x -3.5 3.5) (%in y -3.5 3.5) (%in z -3.5 3.5)) (setf bounded nil))
          (when (and prevx (> (abs (- x prevx)) 1e-9)) (setf moved t))
          (setf prevx x))
        (when (realp bb) (unless (%in bb 0.17 0.25) (setf bounded nil)))))
    (%assert "attractor stays bounded over 25 RK4 steps (|coord|≤3.5, b∈[0.17,0.25])" bounded)
    (%assert "attractor actually evolves (not frozen)" moved))
  (let* ((eig (%pp "eigenmode-status" (lambda () (ipc-call "(:component \"memory-field\" :op \"eigenmode-status\")"))))
         (coh (%num eig :coherence))
         (eigenvalues (%reply-value eig :eigenvalues)))
    (%assert "eigenmode coherence in [0,1]" (%in coh 0.0 1.0))
    (%assert "field graph has a positive nontrivial eigenvalue"
             (and (listp eigenvalues)
                  eigenvalues
                  (every (lambda (v) (and (realp v) (> v 1.0e-6)))
                         eigenvalues))))
  (let* ((rec (%pp "field-recall 'memory'" (lambda () (ipc-call "(:component \"memory-field\" :op \"field-recall\" :query-concepts (\"memory\" \"agent\") :limit 5)"))))
         (activations (%reply-value rec :activations))
         (first-activation (and (listp activations) (first activations)))
         (memory-activation
           (and (listp activations)
                (find "memory" activations :key (lambda (a) (getf a :concept))
                                           :test #'string=))))
    (%assert "field-recall returned :ok" (eq (car (%reply-raw rec)) :ok))
    (%assert "field-recall prioritizes a query boundary"
             (member (getf first-activation :concept) '("memory" "agent")
                     :test #'string=))
    (%assert "field-recall returns canonical entry ids"
             (and (listp activations)
                  (some (lambda (activation)
                          (some (lambda (entry-id)
                                  (and (stringp entry-id)
                                       (null (find #\\ entry-id))
                                       (gethash entry-id *memory-store*)))
                                (getf activation :entries)))
                        activations)))
    (%assert "field-recall preserves the memory concept domain"
             (and memory-activation
                  (eq (getf memory-activation :domain) :cognitive)))
    (let ((post-recall-eig
            (%pp "eigenmode-status after field-recall"
                 (lambda () (ipc-call "(:component \"memory-field\" :op \"eigenmode-status\")")))))
      (%assert "field-recall produces nonzero eigenmode coherence"
               (let ((coh (%num post-recall-eig :coherence)))
                 (and (realp coh) (> coh 0.0))))))
  (%pp "dream" (lambda () (ipc-call "(:component \"memory-field\" :op \"dream\")")))
  (let ((ck (%pp "checkpoint (sexp len)" (lambda () (let ((r (ipc-call "(:component \"memory-field\" :op \"checkpoint\")"))) (length (princ-to-string r)))))))
    (%assert "checkpoint produced non-trivial state" (and (integerp ck) (> ck 50))))

  ;; ─────────────────────────── SIGNALOGRAD KERNEL ───────────────────────────
  (format t "~%── SIGNALOGRAD (projection / learning) ──~%")
  (let ((st (%pp "sg status" (lambda () (ipc-call "(:component \"signalograd\" :op \"status\")")))))
    (let ((c (%num st :confidence)) (s (%num st :stability)) (n (%num st :novelty)))
      (%assert "sg confidence in [0,1]" (%in c 0.0 1.0))
      (%assert "sg stability in [0,1]" (%in s 0.0 1.0))
      (%assert "sg novelty in [0,1]" (%in n 0.0 1.0))))
  (%pp "sg snapshot" (lambda () (ipc-call "(:component \"signalograd\" :op \"snapshot\")")))
  (let ((proj (%pp "sg current-projection (lisp)" (lambda () (signalograd-current-projection *runtime*)))))
    (when (listp proj)
      (let* ((rt (getf proj :routing)) (pw (and (listp rt) (getf rt :price-weight-delta))))
        (%assert "routing price-weight-delta in [-0.07,0.07]" (or (null pw) (%in pw -0.0701 0.0701))))))
  ;; live observe op accepts an observation (cycle is advanced by the reflection
  ;; loop in run-loop mode; here we assert the kernel accepts the observation).
  (let ((obs (%pp "sg observe op"
               (lambda () (ipc-call "(:component \"signalograd\" :op \"observe\" :observation (:signalograd-observe :cycle 1 :signal 0.8 :noise 0.1 :chaos-risk 0.3 :reward 0.7 :stability 0.7 :novelty 0.4))")))))
    (%assert "signalograd observe accepted (:ok)" (eq (car (%reply-raw obs)) :ok)))

  ;; ─────────────────────────── L3 PALACE ───────────────────────────
  (format t "~%── L3 MEMPALACE ──~%")
  (%pp "palace graph-stats" (lambda () (palace-graph-stats)))
  (let* ((token (format nil "p0-palace-~36R" (get-universal-time)))
         (prompt (format nil "Remember the ~A cross-domain palace link." token))
         (response (format nil "The ~A link joins music harmony melody, lisp rust code, and memory dream concepts." token)))
    (%pp "palace P0 record cross-domain memory"
         (lambda () (memory-record-orchestration prompt response "deep-probe" 1.0 0)))
    (let ((stats (%pp "palace P0 graph-stats after filing" (lambda () (palace-graph-stats)))))
      (%assert "palace graph has wings after memory filing"
               (let ((n (%num stats :wings))) (and (integerp n) (> n 0))))
      (%assert "palace graph has concepts after memory filing"
               (let ((n (%num stats :concepts))) (and (integerp n) (> n 0))))
      (%assert "palace graph has edges after memory filing"
               (let ((n (%num stats :edges))) (and (integerp n) (> n 0))))
      (%assert "palace graph has tunnel nodes after cross-domain filing"
               (let ((n (%num stats :tunnels))) (and (integerp n) (> n 0)))))
    (let ((l0 (%pp "palace P0 context :l0 after filing" (lambda () (palace-context :l0)))))
      (%assert "palace context :l0 returns wings"
               (let ((w (getf (%reply-plist l0) :wings))) (and (listp w) w)))))
  (%pp "palace context :l0 (lisp)" (lambda () (palace-context :l0)))
  (%pp "palace context :l1 (lisp)" (lambda () (palace-context :l1)))
  ;; raw IPC to distinguish broken op vs broken lisp wrapper
  (%pp "palace context-l0 (raw ipc)" (lambda () (ipc-call "(:component \"mempalace\" :op \"context-l0\")")))
  (%pp "palace context-l1 (raw ipc)" (lambda () (ipc-call "(:component \"mempalace\" :op \"context-l1\")")))
  (let ((s (%pp "palace-search 'memory'" (lambda () (palace-search "memory" :limit 3)))))
    (%assert "palace-search returned results" (let ((c (getf (%reply-plist s) :count))) (and (integerp c) (>= c 0)))))
  ;; 1A: palace is reconciled to the chronicle record at boot. Re-running the
  ;; reconciliation must file 0 — it is keyed on entry-id and never duplicates.
  (%pp "palace entry-id-keyed drawer count" (lambda () (hash-table-count (palace-entry-ids))))
  (%assert "boot palace reconciliation is idempotent (re-run files 0)"
           (eql (%palace-reconcile-from-memory) 0))

  ;; ─────────────────────────── L0 GENESIS / L4 DATAMINING ───────────────────────────
  (format t "~%── L0 GENESIS / L4 DATAMINING ──~%")
  (let ((before (length (gethash :soul *memory-by-class*))))
    (%pp "L0 re-seed genesis from DNA" (lambda () (memory-seed-soul-from-dna)))
    (%assert "L0 genesis seeding is idempotent (soul count stable on re-seed)"
             (= before (length (gethash :soul *memory-by-class*)))))
  (%assert "L4 terraphon datamining port reachable"
           (and (fboundp 'terraphon-port-ready-p) (terraphon-port-ready-p)))

  ;; ─────────────────────────── L2 CHRONICLE ───────────────────────────
  (format t "~%── L2 CHRONICLE ──~%")
  (dolist (tbl '("memory_entries" "memory_events" "harmonic_snapshots" "signalograd_events" "delegation_log"))
    (%pp (format nil "count ~A" tbl)
         (lambda () (chronicle-query (format nil "SELECT count(*) FROM ~A" tbl)))))
  (%pp "harmony-summary" (lambda () (chronicle-harmony-summary)))
  (%pp "delegation-report" (lambda () (chronicle-delegation-report)))

  ;; ─── 1B: lossless field warm-start (graph-snapshot capture → restore) ───
  ;; The entry-derived rebuild cannot reproduce runtime-learned edges; chronicle
  ;; captures the full graph, and %merge-graph-snapshot-into-field restores it on
  ;; boot. Prove the round-trip restores a dropped edge, exactly once.
  (format t "~%── 1B FIELD SNAPSHOT WARM-START ──~%")
  (%pp "record graph snapshot" (lambda () (chronicle-record-graph-snapshot)))
  (let ((snap (%pp "chronicle latest-graph" (lambda () (chronicle-latest-graph-snapshot)))))
    (%assert "latest-graph returns a concept map with edges"
             (and (listp snap) (consp (getf snap :concept-edges))))
    (let* ((sample (first (getf snap :concept-edges)))
           (a (getf sample :a)) (b (getf sample :b))
           (k (and (stringp a) (stringp b) (%edge-key a b))))
      (when k
        (with-memory-lock () (remhash k *memory-concept-edges*))
        (let ((restored (%pp "merge snapshot after dropping one edge"
                             (lambda () (%merge-graph-snapshot-into-field)))))
          (%assert "snapshot merge restores the dropped learned edge"
                   (and (integerp restored) (>= restored 1)
                        (gethash k *memory-concept-edges*))))
        (%assert "snapshot merge is idempotent (re-merge restores 0)"
                 (eql (%merge-graph-snapshot-into-field) 0)))))

  ;; ─────────────────────────── CHAINS ───────────────────────────
  (format t "~%── CHAINS (IPC round-trip + REPL) ──~%")
  (let ((h (%pp "ipc round-trip mempalace health" (lambda () (ipc-call "(:component \"mempalace\" :op \"health\")")))))
    (%assert "ipc round-trip ok" (and h (eq (car (%reply-raw h)) :ok))))
  (let ((r (%pp "%orchestrate-repl chain" (lambda () (%orchestrate-repl "Compute 6 times 7 using the REPL and respond with the number.")))))
    (%assert "chain returned non-error answer" (and (stringp r) (plusp (length r)) (not (%error-form-p r)))))
  (let ((report (%pp "delegation-report after REPL completion"
                     (lambda () (chronicle-delegation-report)))))
    (%assert "successful REPL completion feeds identified model success into delegation report"
             (and (listp report)
                  (some (lambda (row)
                          (let ((model (and (listp row) (getf row :model-chosen)))
                                (success-pct (and (listp row) (getf row :success-pct))))
                            (and (stringp model)
                                 (plusp (length model))
                                 (realp success-pct)
                                 (> success-pct 0.0))))
                        report))))

  (format t "~%════════ DEEP PROBE TALLY: ~A pass / ~A fail ════════~%" *probe-pass* *probe-fail*)
  (sb-ext:exit :code (if (zerop *probe-fail*) 0 1)))
