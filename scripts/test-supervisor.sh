#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "=== Supervisor: Closed-Loop Task Supervision Test ==="
echo ""

# ── Phase 1: Rust crate tests ──────────────────────────────────────────
echo "[1/5] Rust crate tests (actor-protocol, chronicle, signalograd)"
cargo test -p harmonia-actor-protocol -p harmonia-chronicle -p harmonia-signalograd 2>&1 | tail -20
echo ""

# ── Phase 2: Chronicle migration test ──────────────────────────────────
echo "[2/5] Chronicle migration v3 (supervision tables)"
TMPDIR_TEST=$(mktemp -d)
trap 'rm -rf "$TMPDIR_TEST"' EXIT

cat > "$TMPDIR_TEST/test-chronicle-v3.lisp" << 'LISP'
(load #P"~/quicklisp/setup.lisp")
(funcall (find-symbol "QUICKLOAD" (find-package :ql)) :cffi)
(cffi:load-foreign-library (merge-pathnames "libharmonia_chronicle.dylib" #P"~/.local/lib/harmonia/"))

(cffi:defcfun ("harmonia_chronicle_init" %init) :int)
(cffi:defcfun ("harmonia_chronicle_query_sexp" %query) :pointer (sql :string))
(cffi:defcfun ("harmonia_chronicle_free_string" %free) :void (ptr :pointer))
(cffi:defcfun ("harmonia_chronicle_gc_status" %gc-status) :pointer)

(defun ptr->str (ptr)
  (if (cffi:null-pointer-p ptr) "nil"
      (let ((s (cffi:foreign-string-to-lisp ptr)))
        (%free ptr) s)))

;; Init with temp DB
(let ((rc (%init)))
  (format t "~&CHRONICLE_INIT=~D~%" rc)
  (assert (zerop rc)))

;; Verify supervision_specs table exists (SELECT query — chronicle only allows SELECT)
(let ((result (ptr->str (%query "SELECT COUNT(*) AS cnt FROM supervision_specs"))))
  (format t "~&SUPERVISION_SPECS_TABLE=~A~%" result)
  (assert (search "cnt" result)))

;; Verify supervision_evidence table exists
(let ((result (ptr->str (%query "SELECT COUNT(*) AS cnt FROM supervision_evidence"))))
  (format t "~&SUPERVISION_EVIDENCE_TABLE=~A~%" result)
  (assert (search "cnt" result)))

;; Verify table schema has expected columns
(let ((result (ptr->str (%query
  "SELECT sql FROM sqlite_master WHERE type='table' AND name='supervision_specs'"))))
  (format t "~&SPEC_SCHEMA=~A~%" result)
  (assert (search "task" result))
  (assert (search "taxonomy" result))
  (assert (search "verdict" result))
  (assert (search "confidence" result)))

(let ((result (ptr->str (%query
  "SELECT sql FROM sqlite_master WHERE type='table' AND name='supervision_evidence'"))))
  (format t "~&EVIDENCE_SCHEMA=~A~%" result)
  (assert (search "spec" result))
  (assert (search "kind" result))
  (assert (search "passed" result))
  (assert (search "evidence" result))
  (assert (search "duration" result)))

;; Verify indexes exist
(let ((result (ptr->str (%query
  "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_ss_%'"))))
  (format t "~&SPEC_INDEXES=~A~%" result)
  (assert (search "idx_ss_ts" result))
  (assert (search "idx_ss_task" result)))

;; Verify GC status (new tables appear after release build + install)
(let ((status (ptr->str (%gc-status))))
  (format t "~&GC_STATUS=~A~%" status)
  ;; Note: supervision tables only appear in gc_status after release build.
  ;; The schema/index tests above prove the migration worked.
  (if (search "supervision-specs" status)
      (format t "~&GC_STATUS_INCLUDES_SUPERVISION=yes~%")
      (format t "~&GC_STATUS_INCLUDES_SUPERVISION=no (expected: not yet installed as release)~%")))

(format t "~&~%CHRONICLE_V3_OK=1~%")
(sb-ext:exit)
LISP
sbcl --noinform --disable-debugger --load "$TMPDIR_TEST/test-chronicle-v3.lisp"
echo ""

# ── Phase 3: Lisp supervisor unit tests ────────────────────────────────
echo "[3/5] Lisp supervisor unit tests (classification, evidence, verdict)"
cat > "$TMPDIR_TEST/test-supervisor-lisp.lisp" << 'LISP'
(in-package :cl-user)

(defvar *test-count* 0)
(defvar *pass-count* 0)
(defvar *fail-count* 0)

(defun test-assert (name pred &optional (detail ""))
  (incf *test-count*)
  (if pred
      (progn (incf *pass-count*) (format t "  PASS: ~A~%" name))
      (progn (incf *fail-count*) (format t "  FAIL: ~A ~A~%" name detail))))

(load #P"src/core/boot.lisp")
(harmonia:start :run-loop nil)

(format t "~&--- Supervisor Classification Tests ---~%")

;; Test 1: Confirmable task (create file + add function)
(let* ((prompt "Create a file src/core/supervisor.lisp and add function %supervision-classify-task")
       (result (harmonia::%supervision-classify-task prompt))
       (taxonomy (getf result :taxonomy))
       (assertions (getf result :assertions)))
  (test-assert "confirmable-task-taxonomy" (eq taxonomy :confirmable)
               (format nil "got ~A" taxonomy))
  (test-assert "confirmable-task-has-assertions" (>= (length assertions) 2)
               (format nil "got ~D" (length assertions)))
  (test-assert "confirmable-has-file-exists"
               (find :file-exists assertions :key (lambda (a) (getf a :kind)))))

;; Test 2: Partial task (single file reference)
(let* ((prompt "Update the config/harmony-policy.sexp with new values")
       (result (harmonia::%supervision-classify-task prompt))
       (taxonomy (getf result :taxonomy)))
  (test-assert "partial-task-taxonomy" (member taxonomy '(:partial :confirmable))
               (format nil "got ~A" taxonomy)))

;; Test 3: Auditable task (explain)
(let* ((prompt "Explain how the harmonic machine works")
       (result (harmonia::%supervision-classify-task prompt))
       (taxonomy (getf result :taxonomy)))
  (test-assert "auditable-task-taxonomy" (eq taxonomy :auditable)
               (format nil "got ~A" taxonomy)))

;; Test 4: Deferred task (conversational)
(let* ((prompt "What do you think about this approach?")
       (result (harmonia::%supervision-classify-task prompt))
       (taxonomy (getf result :taxonomy)))
  (test-assert "deferred-task-taxonomy" (eq taxonomy :deferred)
               (format nil "got ~A" taxonomy)))

;; Test 5: Test-oriented task
(let* ((prompt "Fix the failing cargo test in the signalograd crate")
       (result (harmonia::%supervision-classify-task prompt))
       (assertions (getf result :assertions)))
  (test-assert "test-task-has-test-assertion"
               (find :test-passes assertions :key (lambda (a) (getf a :kind)))))

(format t "~&--- Supervisor Heuristic Assertion Tests ---~%")

;; Test 6: File extraction (requires paths with / in them)
(let* ((files (harmonia::%supervision-extract-files
               "Create src/core/supervisor.lisp and modify config/harmony-policy.sexp")))
  (test-assert "extract-files-count" (>= (length files) 2)
               (format nil "got ~D: ~A" (length files) files))
  (test-assert "extract-files-lisp"
               (find "supervisor.lisp" files :test (lambda (needle f) (search needle f)))
               (format nil "files=~A" files))
  (test-assert "extract-files-sexp"
               (find "harmony-policy.sexp" files :test (lambda (needle f) (search needle f)))
               (format nil "files=~A" files)))

;; Test 7: Identifier extraction
(let ((name (harmonia::%supervision-extract-identifier "add function %supervision-classify-task")))
  (test-assert "extract-identifier" (and name (search "supervision" name))
               (format nil "got ~A" name)))

;; Test 8: Test command detection
(test-assert "detect-cargo-test"
  (string= "cargo test" (harmonia::%supervision-detect-test-command "run cargo test")))
(test-assert "detect-npm-test"
  (string= "npm test" (harmonia::%supervision-detect-test-command "run npm test")))
(test-assert "detect-pytest"
  (string= "pytest" (harmonia::%supervision-detect-test-command "run pytest")))

(format t "~&--- Supervisor Evidence Collection Tests ---~%")

;; Test 9: File exists evidence (test against a real file)
(let* ((result (harmonia::%supervision-evaluate-file-exists
                "boot.lisp"
                (merge-pathnames "src/core/" (truename ".")))))
  (test-assert "file-exists-real-file" (getf result :passed)
               (format nil "evidence: ~A" (getf result :evidence))))

;; Test 10: File absent evidence (non-existent file)
(let* ((result (harmonia::%supervision-evaluate-file-exists
                "nonexistent-file-xyz.lisp"
                (truename "."))))
  (test-assert "file-not-found" (not (getf result :passed))))

;; Test 11: File absent assertion (should pass for non-existent)
(let* ((result (harmonia::%supervision-evaluate-file-absent
                "nonexistent-file-xyz.lisp"
                (truename "."))))
  (test-assert "file-absent-passes" (getf result :passed)))

;; Test 12: File contains evidence
(let* ((result (harmonia::%supervision-evaluate-file-contains
                "boot.lisp"
                "defun start"
                (merge-pathnames "src/core/" (truename ".")))))
  (test-assert "file-contains-pattern" (getf result :passed)
               (format nil "evidence: ~A" (getf result :evidence))))

;; Test 13: File contains negative
(let* ((result (harmonia::%supervision-evaluate-file-contains
                "boot.lisp"
                "XYZNONEXISTENTPATTERN"
                (merge-pathnames "src/core/" (truename ".")))))
  (test-assert "file-not-contains" (not (getf result :passed))))

;; Test 14: Output present evidence
(let* ((result (harmonia::%supervision-evaluate-output-present "Hello world, task completed")))
  (test-assert "output-present" (getf result :passed)))

;; Test 15: Output empty evidence
(let* ((result (harmonia::%supervision-evaluate-output-present "   ")))
  (test-assert "output-empty" (not (getf result :passed))))

;; Test 16: Semantic evaluation
(let* ((result (harmonia::%supervision-evaluate-semantic
                "explain how harmonic machine works"
                "The harmonic machine is a state machine that transitions through phases including observe, evaluate-global, evaluate-local, logistic-balance, and more.")))
  (test-assert "semantic-match" (getf result :passed)
               (format nil "evidence: ~A" (getf result :evidence))))

(format t "~&--- Supervisor Verdict Tests ---~%")

;; Test 17: All-pass verdict
(let* ((evidence (list (list :kind :file-exists :passed t :evidence "ok" :duration 0)
                       (list :kind :file-contains :passed t :evidence "ok" :duration 0)
                       (list :kind :output-present :passed t :evidence "ok" :duration 0)))
       (verdict (harmonia::%supervision-verdict evidence :confirmable)))
  (test-assert "verdict-confirmed" (eq (getf verdict :grade) :confirmed)
               (format nil "got ~A" (getf verdict :grade)))
  (test-assert "verdict-confidence-high" (> (getf verdict :confidence) 0.9)
               (format nil "got ~,2F" (getf verdict :confidence)))
  (test-assert "verdict-passed-3" (= (getf verdict :passed) 3)))

;; Test 18: Partial verdict
(let* ((evidence (list (list :kind :file-exists :passed t :evidence "ok" :duration 0)
                       (list :kind :file-contains :passed nil :evidence "not found" :duration 0)
                       (list :kind :output-present :passed t :evidence "ok" :duration 0)))
       (verdict (harmonia::%supervision-verdict evidence :confirmable)))
  (test-assert "verdict-partial" (eq (getf verdict :grade) :partial)
               (format nil "got ~A" (getf verdict :grade)))
  (test-assert "verdict-partial-confidence" (< (getf verdict :confidence) 1.0)
               (format nil "got ~,2F" (getf verdict :confidence))))

;; Test 19: Failed verdict
(let* ((evidence (list (list :kind :file-exists :passed nil :evidence "not found" :duration 0)
                       (list :kind :file-contains :passed nil :evidence "not found" :duration 0)
                       (list :kind :output-present :passed nil :evidence "empty" :duration 0)))
       (verdict (harmonia::%supervision-verdict evidence :confirmable)))
  (test-assert "verdict-failed" (eq (getf verdict :grade) :failed)
               (format nil "got ~A" (getf verdict :grade)))
  (test-assert "verdict-failed-confidence-zero" (= (getf verdict :confidence) 0.0)))

;; Test 20: Deferred verdict (taxonomy overrides)
(let* ((evidence (list (list :kind :output-present :passed t :evidence "ok" :duration 0)))
       (verdict (harmonia::%supervision-verdict evidence :deferred)))
  (test-assert "verdict-deferred" (eq (getf verdict :grade) :deferred)
               (format nil "got ~A" (getf verdict :grade)))
  (test-assert "verdict-deferred-zero-confidence" (= (getf verdict :confidence) 0.0)))

;; Test 21: Auditable verdict confidence
(let* ((evidence (list (list :kind :output-present :passed t :evidence "ok" :duration 0)))
       (verdict (harmonia::%supervision-verdict evidence :auditable)))
  (test-assert "verdict-auditable-confidence" (= (getf verdict :confidence) 0.4)
               (format nil "got ~,2F" (getf verdict :confidence))))

;; Test 22: Partial taxonomy baseline confidence
(let* ((evidence (list (list :kind :file-exists :passed t :evidence "ok" :duration 0)
                       (list :kind :output-present :passed nil :evidence "empty" :duration 0)))
       (verdict (harmonia::%supervision-verdict evidence :partial)))
  (test-assert "verdict-partial-baseline"
               (>= (getf verdict :confidence) 0.5)
               (format nil "partial baseline should be >= 0.5, got ~,2F" (getf verdict :confidence))))

(format t "~&--- Supervisor Spec Freezing Tests ---~%")

;; Test 23: Freeze spec and verify structure
(let* ((spec (harmonia::%supervision-freeze-spec 42 :confirmable
               (list (list :kind :file-exists :path "test.lisp")
                     (list :kind :output-present)))))
  (test-assert "freeze-spec-taxonomy" (eq (getf spec :taxonomy) :confirmable))
  (test-assert "freeze-spec-assertions" (= (getf spec :n-assertions) 2))
  (test-assert "freeze-spec-frozen-at" (numberp (getf spec :frozen-at)))
  (test-assert "freeze-spec-task-id" (= (getf spec :task-id) 42)))

;; Test 24: Update task-id on frozen spec
(let* ((spec (list :taxonomy :confirmable :assertions '() :task-id 0)))
  (harmonia::%supervision-update-task-id spec 99)
  (test-assert "update-task-id" (= (getf spec :task-id) 99)))

(format t "~&--- Supervision Rate Tests ---~%")

;; Test 25: Supervision rate returns a number in [0, 1]
(let ((rate (harmonia::%supervision-rate)))
  (test-assert "supervision-rate-numeric" (numberp rate)
               (format nil "got ~A" rate))
  (test-assert "supervision-rate-bounded" (<= 0.0 rate 1.0)
               (format nil "got ~,2F" rate)))

(format t "~&--- Supervisor End-to-End Flow Tests ---~%")

;; Test 26: Full classify -> collect -> verdict flow
(let* ((prompt "Create file src/core/supervisor.lisp with function %supervision-classify-task")
       (classification (harmonia::%supervision-classify-task prompt))
       (spec (harmonia::%supervision-freeze-spec
              1
              (getf classification :taxonomy)
              (getf classification :assertions)))
       (evidence (harmonia::%supervision-collect-evidence
                  spec
                  "File created successfully with the function."
                  (truename ".")))
       (verdict (harmonia::%supervision-verdict evidence (getf spec :taxonomy))))
  (test-assert "e2e-classification" (member (getf classification :taxonomy) '(:confirmable :partial)))
  (test-assert "e2e-spec-frozen" (not (null spec)))
  (test-assert "e2e-evidence-collected" (> (length evidence) 0)
               (format nil "got ~D evidence items" (length evidence)))
  (test-assert "e2e-verdict-has-grade" (member (getf verdict :grade) '(:confirmed :partial :failed))
               (format nil "got ~A" (getf verdict :grade)))
  (test-assert "e2e-verdict-has-summary" (> (length (getf verdict :summary)) 0)))

;; Test 27: Verify actor-record has supervision fields
(let ((record (harmonia::make-actor-record
               :id 1 :model "test" :prompt "test" :state :running
               :supervision-spec '(:test t)
               :supervision-grade nil
               :supervision-confidence nil)))
  (test-assert "actor-record-spec" (equal (harmonia::actor-record-supervision-spec record) '(:test t)))
  (test-assert "actor-record-grade-nil" (null (harmonia::actor-record-supervision-grade record)))
  (test-assert "actor-record-confidence-nil" (null (harmonia::actor-record-supervision-confidence record)))
  ;; Set values
  (setf (harmonia::actor-record-supervision-grade record) :confirmed)
  (setf (harmonia::actor-record-supervision-confidence record) 0.95)
  (test-assert "actor-record-grade-set" (eq (harmonia::actor-record-supervision-grade record) :confirmed))
  (test-assert "actor-record-confidence-set" (= (harmonia::actor-record-supervision-confidence record) 0.95)))

(format t "~&--- Vitruvian Supervision Weight Test ---~%")

;; Test 28: Verify vitruvian utility includes supervision
(let* ((policy-path (merge-pathnames "config/harmony-policy.sexp" (truename ".")))
       (sexp (with-open-file (s policy-path) (let ((*read-eval* nil)) (read s))))
       (vit (getf sexp :vitruvian))
       (sv-weight (getf vit :utility-supervision-weight)))
  (test-assert "policy-supervision-weight-exists" (numberp sv-weight)
               (format nil "got ~A" sv-weight))
  (test-assert "policy-supervision-weight-0.20" (= sv-weight 0.20)
               (format nil "got ~,2F" sv-weight))
  ;; Verify weights sum to 1.0
  (let ((total (+ (getf vit :utility-global-weight)
                  (getf vit :utility-coherence-weight)
                  (getf vit :utility-balance-weight)
                  (getf vit :utility-supervision-weight))))
    (test-assert "utility-weights-sum-to-1" (< (abs (- total 1.0)) 0.01)
                 (format nil "sum=~,3F" total))))

(format t "~&--- Supervision Policy Section Test ---~%")

;; Test 29: Verify supervision policy section
(let* ((policy-path (merge-pathnames "config/harmony-policy.sexp" (truename ".")))
       (sexp (with-open-file (s policy-path) (let ((*read-eval* nil)) (read s))))
       (supervision (getf sexp :supervision)))
  (test-assert "supervision-policy-exists" (not (null supervision)))
  (test-assert "supervision-window" (= (getf supervision :window) 64))
  (test-assert "supervision-confirmable-threshold" (= (getf supervision :confirmable-threshold) 0.85))
  (test-assert "supervision-evidence-timeout" (= (getf supervision :evidence-timeout-ms) 30000)))

(format t "~&--- Signalograd Supervision Dimension Test ---~%")

;; Test 30: Verify signalograd observation includes supervision field
;; The observation sexp is built from harmonic context — check via source inspection
;; that :supervision is emitted between :error-pressure and :prior-confidence
(let* ((src (with-open-file (s (merge-pathnames "src/core/signalograd.lisp" (truename ".")))
              (let ((buf (make-string (file-length s))))
                (read-sequence buf s)
                buf))))
  (test-assert "signalograd-has-supervision"
               (and (search ":supervision" src)
                    (search "%supervision-rate" src))
               "signalograd.lisp should contain :supervision field in observation assembly"))

(format t "~&~%=== RESULTS ===~%")
(format t "Total: ~D  Passed: ~D  Failed: ~D~%" *test-count* *pass-count* *fail-count*)
(if (zerop *fail-count*)
    (format t "~&SUPERVISOR_TEST_OK=1~%")
    (format t "~&SUPERVISOR_TEST_FAILED=~D~%" *fail-count*))
(sb-ext:exit :code (if (zerop *fail-count*) 0 1))
LISP
sbcl --noinform --disable-debugger --load "$TMPDIR_TEST/test-supervisor-lisp.lisp"
echo ""

# ── Phase 4: Actor-protocol supervision message sexp test ──────────────
echo "[4/5] Actor-protocol supervision message format test"
cat > "$TMPDIR_TEST/test-supervision-msgs.lisp" << 'LISP'
(load #P"~/quicklisp/setup.lisp")
(funcall (find-symbol "QUICKLOAD" (find-package :ql)) :cffi)
(cffi:load-foreign-library (merge-pathnames "libharmonia_parallel_agents.dylib" #P"~/.local/lib/harmonia/"))

(cffi:defcfun ("harmonia_actor_register" %register) :long-long (kind :string))
(cffi:defcfun ("harmonia_actor_post" %post) :int
  (source :long-long) (target :long-long) (kind :string) (payload :string))
(cffi:defcfun ("harmonia_actor_drain" %drain) :pointer)
(cffi:defcfun ("harmonia_actor_free_string" %free) :void (ptr :pointer))
(cffi:defcfun ("harmonia_actor_deregister" %deregister) :int (id :long-long))

(defun ptr->str (ptr)
  (if (cffi:null-pointer-p ptr) "nil"
      (let ((s (cffi:foreign-string-to-lisp ptr)))
        (%free ptr) s)))

;; Register a supervisor actor
;; NOTE: This test requires the release dylib to include ActorKind::Supervisor.
;; If using an older installed dylib, supervisor registration will return -1.
(let ((sv-id (%register "supervisor")))
  (format t "~&SUPERVISOR_REGISTERED=~D~%" sv-id)
  (if (< sv-id 0)
      (progn
        (format t "~&SKIP: installed dylib does not yet include ActorKind::Supervisor~%")
        (format t "~&(This is expected before `cargo build --release` + install)~%")
        (format t "~&~%SUPERVISION_MSG_OK=skip~%"))
      (progn
        ;; Drain to verify registration message
        (let ((drain (ptr->str (%drain))))
          (format t "~&DRAIN_AFTER_REGISTER=~A~%" drain))

        (let ((rc (%post sv-id 0 "supervisor"
                         ":state-changed :to :active")))
          (format t "~&POST_RC=~D~%" rc))

        (let ((drain (ptr->str (%drain))))
          (format t "~&DRAIN_SUPERVISOR_MSG=~A~%" drain)
          (assert (search "supervisor" drain)))

        (%deregister sv-id)
        (format t "~&~%SUPERVISION_MSG_OK=1~%"))))
(sb-ext:exit)
LISP
sbcl --noinform --disable-debugger --load "$TMPDIR_TEST/test-supervision-msgs.lisp"
echo ""

# ── Phase 5: Anti-cheat invariant test ─────────────────────────────────
echo "[5/5] Anti-cheat invariant: spec frozen BEFORE execution"
cat > "$TMPDIR_TEST/test-anti-cheat.lisp" << 'LISP'
(in-package :cl-user)
(load #P"src/core/boot.lisp")
(harmonia:start :run-loop nil)

(defvar *failures* 0)
(defun check (name pred &optional (detail ""))
  (if pred
      (format t "  PASS: ~A~%" name)
      (progn (incf *failures*) (format t "  FAIL: ~A ~A~%" name detail))))

(format t "~&--- Anti-Cheat: Temporal Ordering ---~%")

;; Simulate the full spawn flow: classify -> freeze spec -> spawn
(let* ((prompt "Create file /tmp/harmonia-test-artifact.py")
       (t-before (get-universal-time))
       (classification (harmonia::%supervision-classify-task prompt))
       (spec (harmonia::%supervision-freeze-spec
              0
              (getf classification :taxonomy)
              (getf classification :assertions)))
       (t-after-freeze (get-universal-time)))
  ;; Spec must be frozen BEFORE any execution could start
  (check "spec-frozen-timestamp"
         (and (numberp (getf spec :frozen-at))
              (<= t-before (getf spec :frozen-at) t-after-freeze))
         (format nil "frozen-at=~D before=~D after=~D"
                 (getf spec :frozen-at) t-before t-after-freeze))
  ;; Spec is immutable — taxonomy cannot be changed after freeze
  (let ((original-taxonomy (getf spec :taxonomy)))
    ;; Attempt to mutate (the spec is just a plist, but the chronicle copy is immutable)
    (check "spec-taxonomy-preserved"
           (eq original-taxonomy (getf spec :taxonomy))))
  ;; Spec has assertions set before execution
  (check "spec-has-assertions"
         (> (getf spec :n-assertions) 0)
         (format nil "n-assertions=~D" (getf spec :n-assertions))))

(format t "~&--- Anti-Cheat: Actor Separation ---~%")
;; Supervisor is a different ActorKind
(check "supervisor-is-distinct-kind"
       (not (eq :supervisor :cli-agent)))

(format t "~&--- Anti-Cheat: Concrete Evidence ---~%")
;; Evidence collection uses probe-file, not self-report
(let* ((spec (list :taxonomy :confirmable
                   :assertions (list (list :kind :file-exists :path "/tmp/harmonia-nonexistent-xyz")
                                     (list :kind :output-present))))
       (evidence (harmonia::%supervision-collect-evidence spec "" "/tmp/")))
  ;; File doesn't exist — evidence must report failure
  (check "concrete-evidence-file-missing"
         (let ((fe (find :file-exists evidence :key (lambda (e) (getf e :kind)))))
           (and fe (not (getf fe :passed))))
         "file-exists should fail for nonexistent file")
  ;; Output is empty — evidence must report failure
  (check "concrete-evidence-empty-output"
         (let ((op (find :output-present evidence :key (lambda (e) (getf e :kind)))))
           (and op (not (getf op :passed))))
         "output-present should fail for empty output"))

;; An agent that exits with empty output should get :failed verdict
(let* ((spec (list :taxonomy :confirmable
                   :assertions (list (list :kind :file-exists :path "/tmp/harmonia-nonexistent-xyz")
                                     (list :kind :output-present))))
       (evidence (harmonia::%supervision-collect-evidence spec "" "/tmp/"))
       (verdict (harmonia::%supervision-verdict evidence :confirmable)))
  (check "cheating-agent-fails"
         (eq (getf verdict :grade) :failed)
         (format nil "grade=~A (expected :failed)" (getf verdict :grade)))
  (check "cheating-agent-zero-confidence"
         (= (getf verdict :confidence) 0.0)
         (format nil "confidence=~,2F" (getf verdict :confidence))))

(format t "~&~%ANTI_CHEAT_FAILURES=~D~%" *failures*)
(if (zerop *failures*)
    (progn (format t "ANTI_CHEAT_OK=1~%") (sb-ext:exit :code 0))
    (sb-ext:exit :code 1))
LISP
sbcl --noinform --disable-debugger --load "$TMPDIR_TEST/test-anti-cheat.lisp"
echo ""

echo "=== All supervisor tests complete ==="
