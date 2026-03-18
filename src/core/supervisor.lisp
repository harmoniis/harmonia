;;; supervisor.lisp — Closed-loop task supervision.
;;;
;;; Generates supervision specs BEFORE task execution, collects evidence
;;; AFTER completion, and feeds verdicts back into the harmonic system.
;;; The spec is frozen in chronicle — immutable once committed.
;;;
;;; Taxonomy tiers:
;;;   :confirmable — produces concrete artifacts (files, tests, git ops)
;;;   :partial     — some aspects checkable, others semantic
;;;   :auditable   — creative/analytical (explain, draft, summarize)
;;;   :deferred    — conversational, opinion-based (no evidence possible)

(in-package #:harmonia)

;;; ─── Heuristic patterns ──────────────────────────────────────────────
;;;
;;; Each pattern matches prompt text and produces typed assertions.
;;; Pure Lisp — no Rust dependency.

;; Verb keyword lists for heuristic matching (pure CL — no cl-ppcre dependency)
(defparameter *supervision-verb-keywords*
  '((:create  ("create" "generate" "new file" "add file"))
    (:delete  ("delete" "remove file" "rm "))
    (:add-fn  ("add function" "add fn" "add def" "add method" "add class"))
    (:fix     ("fix test" "test pass" "tests should pass" "test should pass"))
    (:compile ("compile" "cargo build" " build " " make "))
    (:test    ("run test" "cargo test" "npm test" "pytest")))
  "Verb keyword lists for heuristic assertion extraction.")

(defparameter *supervision-file-extensions*
  '("lisp" "rs" "py" "js" "ts" "go" "rb" "c" "h" "toml" "json" "yaml" "yml" "md" "sh" "sql" "css" "html" "sexp")
  "File extensions to recognize in prompts.")

;;; ─── String matching helpers (pure CL) ────────────────────────────────

(defun %search-any (keywords text)
  "Return T if any of KEYWORDS is found in TEXT (case-insensitive via pre-lowered text)."
  (some (lambda (kw) (search kw text)) keywords))

(defun %supervision-match-verb (verb-entry lower-prompt)
  "Check if any keyword in verb-entry matches the lowered prompt."
  (let ((keywords (second verb-entry)))
    (%search-any keywords lower-prompt)))

;;; ─── Classification ──────────────────────────────────────────────────

(defun %supervision-classify-task (prompt)
  "Analyze task prompt and return (:taxonomy T :assertions LIST).
   Heuristic layer: pattern-match for file refs, test commands, git ops.
   Falls back to :auditable if some structure found, :deferred otherwise."
  (let* ((assertions (%supervision-heuristic-assertions prompt))
         (n (length assertions))
         (lower (string-downcase prompt))
         (taxonomy (cond
                     ((>= n 2) :confirmable)
                     ((= n 1)  :partial)
                     ;; Check for any action-oriented language
                     ((%search-any '("explain" "draft" "summarize" "describe" "analyze" "review") lower)
                      :auditable)
                     (t :deferred))))
    ;; For confirmable/partial tasks with few assertions, always include output-present
    (when (and (member taxonomy '(:confirmable :partial :auditable))
               (not (find :output-present assertions :key (lambda (a) (getf a :kind)))))
      (push (list :kind :output-present) assertions))
    (list :taxonomy taxonomy :assertions assertions)))

(defun %supervision-heuristic-assertions (prompt)
  "Extract concrete assertions from prompt text.
   Scans for: file paths, create/add/fix/delete verbs, test commands,
   compilation commands. Returns list of assertion plists."
  (let ((assertions '())
        (files (%supervision-extract-files prompt))
        (lower (string-downcase prompt)))
    ;; Check verb patterns
    (dolist (vp *supervision-verb-keywords*)
      (let ((verb (first vp)))
        (when (%supervision-match-verb vp lower)
          (case verb
            (:create
             (dolist (f files)
               (push (list :kind :file-exists :path f) assertions)))
            (:delete
             (dolist (f files)
               (push (list :kind :file-absent :path f) assertions)))
            (:add-fn
             ;; Try to extract function/method name
             (let ((name (%supervision-extract-identifier prompt)))
               (when (and name files)
                 (dolist (f files)
                   (push (list :kind :file-contains :path f :pattern name) assertions)))))
            (:fix
             (push (list :kind :test-passes :command "cargo test") assertions))
            (:compile
             (push (list :kind :compiles :command "cargo build") assertions))
            (:test
             (push (list :kind :test-passes
                         :command (%supervision-detect-test-command prompt))
                   assertions))))))
    ;; If files referenced, assert they are modified (git diff)
    (when (and files (not (find :file-exists assertions :key (lambda (a) (getf a :kind)))))
      (dolist (f files)
        (push (list :kind :file-modified :path f) assertions)))
    (nreverse assertions)))

(defun %supervision-extract-files (prompt)
  "Extract file paths from prompt text. Returns list of path strings.
   Splits on whitespace and looks for tokens ending in known extensions."
  (let ((files '())
        (words (%split-whitespace prompt)))
    (dolist (word words)
      ;; Strip trailing punctuation
      (let ((clean (string-right-trim '(#\, #\. #\; #\: #\) #\]) word)))
        (dolist (ext *supervision-file-extensions*)
          (let ((suffix (concatenate 'string "." ext)))
            (when (and (> (length clean) (length suffix))
                       (string-equal suffix (subseq clean (- (length clean) (length suffix))))
                       (find #\/ clean)  ;; require at least one / to avoid matching bare words
                       (not (member clean files :test #'string-equal)))
              (push clean files))))))
    (nreverse files)))

(defun %split-whitespace (str)
  "Split STR on whitespace. Returns list of non-empty tokens."
  (let ((tokens '())
        (start nil))
    (dotimes (i (length str))
      (let ((ch (char str i)))
        (if (member ch '(#\Space #\Tab #\Newline #\Return))
            (when start
              (push (subseq str start i) tokens)
              (setf start nil))
            (unless start
              (setf start i)))))
    (when start
      (push (subseq str start) tokens))
    (nreverse tokens)))

(defun %supervision-extract-identifier (prompt)
  "Try to extract a function/method/class name from prompt.
   Looks for patterns like 'add function foo' or 'def bar'."
  (let* ((lower (string-downcase prompt))
         (keywords '("function " "fn " "def " "method " "class ")))
    (dolist (kw keywords)
      (let ((pos (search kw lower)))
        (when pos
          (let* ((start (+ pos (length kw)))
                 (end (or (position-if
                           (lambda (c) (member c '(#\Space #\Tab #\Newline #\( #\) #\, #\;)))
                           prompt :start start)
                          (length prompt)))
                 (name (subseq prompt start end)))
            (when (> (length name) 0)
              (return name))))))))

(defun %supervision-detect-test-command (prompt)
  "Detect test runner from prompt context."
  (let ((lower (string-downcase prompt)))
    (cond
      ((search "cargo test" lower) "cargo test")
      ((search "npm test" lower) "npm test")
      ((search "pytest" lower) "pytest")
      ((search "go test" lower) "go test ./...")
      (t "cargo test"))))

;;; ─── Spec freezing (chronicle) ───────────────────────────────────────

(defun %supervision-freeze-spec (task-id taxonomy assertions)
  "Commit spec to chronicle via SQL. Returns spec-id plist.
   Once frozen, the spec is immutable — only verdict fields are updated later."
  (let* ((spec-sexp (prin1-to-string (list :taxonomy taxonomy :assertions assertions)))
         (n-assertions (length assertions))
         (taxonomy-str (string-downcase (symbol-name taxonomy))))
    ;; Insert via chronicle-query (SELECT-only) won't work; batch through chronicle-pending
    (let ((spec-record (list :task (or task-id 0)
                             :taxonomy taxonomy-str
                             :spec spec-sexp
                             :assertions n-assertions)))
      ;; Queue the insert for chronicle flush
      (when (and (boundp '*runtime*) *runtime*)
        (push (list :type "supervision-spec" :args spec-record)
              (runtime-state-chronicle-pending *runtime*)))
      ;; Return the spec as a frozen s-expression (used by actor-record)
      (list :taxonomy taxonomy
            :assertions assertions
            :n-assertions n-assertions
            :frozen-at (get-universal-time)
            :task-id (or task-id 0)))))

(defun %supervision-update-task-id (spec-sexp actor-id)
  "Update the task-id on a spec after the actor is spawned.
   Modifies the in-memory plist only — chronicle update is batched."
  (when (and spec-sexp (listp spec-sexp))
    (setf (getf spec-sexp :task-id) actor-id))
  spec-sexp)

;;; ─── Evidence collection ─────────────────────────────────────────────

(defun %supervision-collect-evidence (spec output workdir)
  "Evaluate assertions against concrete evidence.
   Uses probe-file for file existence, uiop:run-program for git/test,
   string matching for output checks. Returns evidence list."
  (let* ((assertions (getf spec :assertions))
         (evidence '())
         (timeout-ms (harmony-policy-number "supervision/evidence-timeout-ms" 30000)))
    (declare (ignore timeout-ms))
    (dolist (assertion assertions)
      (let* ((kind (getf assertion :kind))
             (result (handler-case
                         (case kind
                           (:file-exists
                            (%supervision-evaluate-file-exists
                             (getf assertion :path) workdir))
                           (:file-absent
                            (%supervision-evaluate-file-absent
                             (getf assertion :path) workdir))
                           (:file-contains
                            (%supervision-evaluate-file-contains
                             (getf assertion :path)
                             (getf assertion :pattern)
                             workdir))
                           (:file-modified
                            (%supervision-evaluate-file-modified
                             (getf assertion :path) workdir))
                           (:test-passes
                            (%supervision-evaluate-test-passes
                             (getf assertion :command) workdir))
                           (:compiles
                            (%supervision-evaluate-compiles
                             (getf assertion :command) workdir))
                           (:output-present
                            (%supervision-evaluate-output-present output))
                           (:semantic
                            (%supervision-evaluate-semantic
                             (getf assertion :claim) output))
                           (t (list :passed nil
                                    :evidence (format nil "unknown assertion kind: ~A" kind)
                                    :duration 0)))
                       (error (e)
                         (list :passed nil
                               :evidence (format nil "error: ~A" (princ-to-string e))
                               :duration 0)))))
        (push (list :kind kind
                    :detail (or (getf assertion :path)
                                (getf assertion :command)
                                (getf assertion :claim)
                                (string-downcase (symbol-name kind)))
                    :passed (getf result :passed)
                    :evidence (getf result :evidence)
                    :duration (or (getf result :duration) 0))
              evidence)))
    (nreverse evidence)))

;;; ─── Individual evidence evaluators ──────────────────────────────────

(defun %supervision-evaluate-file-exists (path workdir)
  "Check if file exists. Returns (:passed T/NIL :evidence description)."
  (let* ((full-path (if (uiop:absolute-pathname-p path)
                        path
                        (merge-pathnames path workdir)))
         (exists (probe-file full-path)))
    (list :passed (not (null exists))
          :evidence (if exists
                        (format nil "file exists: ~A" full-path)
                        (format nil "file not found: ~A" full-path))
          :duration 0)))

(defun %supervision-evaluate-file-absent (path workdir)
  "Check if file does NOT exist."
  (let* ((full-path (if (uiop:absolute-pathname-p path)
                        path
                        (merge-pathnames path workdir)))
         (exists (probe-file full-path)))
    (list :passed (null exists)
          :evidence (if exists
                        (format nil "file still exists: ~A" full-path)
                        (format nil "file absent (expected): ~A" full-path))
          :duration 0)))

(defun %supervision-evaluate-file-contains (path pattern workdir)
  "Grep file for pattern. Returns (:passed T/NIL :evidence matched-line)."
  (let* ((full-path (if (uiop:absolute-pathname-p path)
                        path
                        (merge-pathnames path workdir))))
    (if (not (probe-file full-path))
        (list :passed nil :evidence (format nil "file not found: ~A" full-path) :duration 0)
        (let* ((start (get-internal-real-time))
               (content (ignore-errors
                          (uiop:read-file-string full-path)))
               (found (and content (search pattern content)))
               (elapsed (round (* 1000 (/ (- (get-internal-real-time) start)
                                           internal-time-units-per-second)))))
          (list :passed (not (null found))
                :evidence (if found
                              (format nil "pattern '~A' found in ~A" pattern (pathname-name full-path))
                              (format nil "pattern '~A' not found in ~A" pattern (pathname-name full-path)))
                :duration elapsed)))))

(defun %supervision-evaluate-file-modified (path workdir)
  "Check git diff for file. Returns (:passed T/NIL :evidence diff-summary)."
  (let* ((full-path (if (uiop:absolute-pathname-p path)
                        path
                        (namestring (merge-pathnames path workdir))))
         (start (get-internal-real-time))
         (diff (ignore-errors
                 (uiop:run-program
                  (list "git" "diff" "--stat" "--" full-path)
                  :directory workdir
                  :output :string
                  :error-output nil
                  :ignore-error-status t)))
         (elapsed (round (* 1000 (/ (- (get-internal-real-time) start)
                                     internal-time-units-per-second))))
         (modified (and diff (> (length (string-trim '(#\Space #\Newline) diff)) 0))))
    (list :passed modified
          :evidence (if modified
                        (format nil "git diff non-empty for ~A" (pathname-name (pathname full-path)))
                        (format nil "no git changes for ~A" (pathname-name (pathname full-path))))
          :duration elapsed)))

(defun %supervision-evaluate-test-passes (command workdir)
  "Run test command. Returns (:passed T/NIL :evidence exit-code + output)."
  (let* ((timeout-ms (harmony-policy-number "supervision/test-timeout-ms" 60000))
         (start (get-internal-real-time))
         (output "")
         (exit-code 1))
    (ignore-errors
      (multiple-value-bind (out err code)
          (uiop:run-program
           command
           :directory workdir
           :output :string
           :error-output :string
           :ignore-error-status t)
        (declare (ignore err))
        (setf output (or out ""))
        (setf exit-code (or code 1))))
    (let ((elapsed (round (* 1000 (/ (- (get-internal-real-time) start)
                                      internal-time-units-per-second)))))
      (declare (ignore timeout-ms))
      (list :passed (zerop exit-code)
            :evidence (format nil "exit-code=~D output-length=~D" exit-code (length output))
            :duration elapsed))))

(defun %supervision-evaluate-compiles (command workdir)
  "Run compilation command. Returns (:passed T/NIL :evidence exit-code)."
  (let* ((start (get-internal-real-time))
         (exit-code 1))
    (ignore-errors
      (multiple-value-bind (out err code)
          (uiop:run-program
           command
           :directory workdir
           :output :string
           :error-output :string
           :ignore-error-status t)
        (declare (ignore out err))
        (setf exit-code (or code 1))))
    (let ((elapsed (round (* 1000 (/ (- (get-internal-real-time) start)
                                      internal-time-units-per-second)))))
      (list :passed (zerop exit-code)
            :evidence (format nil "compilation exit-code=~D" exit-code)
            :duration elapsed))))

(defun %supervision-evaluate-output-present (output)
  "Check non-empty output. Returns (:passed T/NIL :evidence length)."
  (let ((trimmed (string-trim '(#\Space #\Newline #\Tab) (or output ""))))
    (list :passed (> (length trimmed) 0)
          :evidence (format nil "output-length=~D" (length trimmed))
          :duration 0)))

(defun %supervision-evaluate-semantic (claim output)
  "Semantic assertion — check if output addresses the claim.
   Simple heuristic: check if key words from claim appear in output."
  (let* ((words (%split-whitespace (string-downcase (or claim ""))))
         (output-lower (string-downcase (or output "")))
         (matches (count-if (lambda (w)
                              (and (> (length w) 3) (search w output-lower)))
                            words))
         (total (max 1 (count-if (lambda (w) (> (length w) 3)) words)))
         (ratio (/ matches total)))
    (list :passed (> ratio 0.3)
          :evidence (format nil "semantic-match ~,1F% (~D/~D keywords)"
                            (* 100.0 ratio) matches total)
          :duration 0)))

;;; ─── Verdict aggregation ─────────────────────────────────────────────

(defun %supervision-verdict (evidence taxonomy)
  "Aggregate evidence into a verdict.
   Returns (:grade G :passed N :failed N :skipped N :confidence C :summary S)."
  (let* ((passed-count (count-if (lambda (e) (getf e :passed)) evidence))
         (failed-count (count-if (lambda (e) (and (not (getf e :passed))
                                                   (getf e :evidence)))
                                 evidence))
         (skipped-count (- (length evidence) passed-count failed-count))
         (total (max 1 (length evidence)))
         (confidence
           (case taxonomy
             (:confirmable
              ;; Direct ratio
              (/ (float passed-count) total))
             (:partial
              ;; Baseline 0.5 + scaled ratio
              (+ 0.5 (* 0.5 (/ (float passed-count) total))))
             (:auditable
              ;; 0.4 if any output present, 0.0 if empty
              (if (> passed-count 0) 0.4 0.0))
             (:deferred 0.0)
             (t 0.0)))
         (grade
           (cond
             ((eq taxonomy :deferred) :deferred)
             ((= passed-count total) :confirmed)
             ((> passed-count failed-count) :partial)
             (t :failed)))
         (summary
           (format nil "~A: ~D/~D passed~@[, ~D failed~]~@[, ~D skipped~]"
                   (string-downcase (symbol-name grade))
                   passed-count total
                   (when (> failed-count 0) failed-count)
                   (when (> skipped-count 0) skipped-count))))
    (list :grade grade
          :passed passed-count
          :failed failed-count
          :skipped skipped-count
          :confidence (float confidence)
          :summary summary)))

;;; ─── Supervision rate (for harmonic machine) ─────────────────────────

(defun %supervision-rate ()
  "Rolling pass rate across recent supervised tasks.
   Queries chronicle for recent supervision verdicts."
  (let ((window (harmony-policy-number "supervision/window" 64)))
    (handler-case
        (let* ((results (chronicle-query
                         (format nil
                          "SELECT verdict, confidence FROM supervision_specs
                           WHERE verdict IS NOT NULL
                           ORDER BY ts DESC LIMIT ~D" (round window))))
               (total (length results))
               (passed (count-if
                        (lambda (v)
                          (let ((verdict (getf v :verdict)))
                            (or (string-equal verdict "confirmed")
                                (string-equal verdict "partial"))))
                        results)))
          (if (> total 0)
              (%safe-div (float passed) (float total))
              0.5))  ;; neutral default when no data
      (error () 0.5))))

;;; ─── Supervision memory: learning from mistakes ──────────────────────

(defun %supervision-record-learning (record verdict evidence)
  "Store supervision outcome as memory for future learning.
   On failure/partial: record what went wrong for future delegation context."
  (ignore-errors
    (let* ((task-prompt (actor-record-prompt record))
           (grade (getf verdict :grade))
           (failures (remove-if (lambda (e) (getf e :passed)) evidence))
           (successes (remove-if-not (lambda (e) (getf e :passed)) evidence))
           (summary (format nil "Task: ~A~%Grade: ~A~%Confidence: ~,2F~%~
                                Passed: ~D, Failed: ~D~%~
                                ~@[Mistakes: ~{~A~^, ~}~]~%~
                                ~@[Succeeded: ~{~A~^, ~}~]"
                            (%clip-prompt task-prompt 200)
                            (string-downcase (symbol-name grade))
                            (getf verdict :confidence)
                            (getf verdict :passed) (getf verdict :failed)
                            (mapcar (lambda (f) (getf f :detail)) failures)
                            (mapcar (lambda (s) (getf s :detail)) successes)))
           (tags (append '(:supervision :learning)
                         (when (member grade '(:failed :partial))
                           '(:mistakes))
                         (when (eq grade :confirmed)
                           '(:success-pattern)))))
      ;; Store as memory for semantic recall
      (memory-put :supervision summary :tags tags)
      ;; Also batch to chronicle for durable recording
      (when (and (boundp '*runtime*) *runtime*)
        (push (list :type "memory"
                    :args (list "supervision-learning"
                                :entries-created 1
                                :detail summary))
              (runtime-state-chronicle-pending *runtime*))))))

(defun %clip-prompt (prompt max-len)
  "Clip prompt to max-len characters."
  (if (> (length prompt) max-len)
      (subseq prompt 0 max-len)
      prompt))

;;; ─── Tick integration ────────────────────────────────────────────────

(defun %tick-supervision (runtime)
  "Scan actor registry for completed actors with supervision-spec but no verdict.
   For each: collect evidence, compute verdict, store in chronicle, update actor."
  (%supervised-action "supervision"
    (lambda ()
      (let ((registry (runtime-state-actor-registry runtime)))
        (maphash
         (lambda (actor-id record)
           (declare (ignore actor-id))
           (when (and (eq (actor-record-state record) :completed)
                      (actor-record-supervision-spec record)
                      (null (actor-record-supervision-grade record)))
             (let* ((spec (actor-record-supervision-spec record))
                    (output (or (actor-record-result record) ""))
                    (workdir (or (ignore-errors (config-get-for "conductor" "workdir"))
                                 (namestring (user-homedir-pathname))))
                    (evidence (%supervision-collect-evidence spec output workdir))
                    (taxonomy (getf spec :taxonomy))
                    (verdict (%supervision-verdict evidence taxonomy)))
               ;; Update actor record
               (setf (actor-record-supervision-grade record) (getf verdict :grade))
               (setf (actor-record-supervision-confidence record) (getf verdict :confidence))
               ;; Queue chronicle update (verdict + evidence)
               (when (and (boundp '*runtime*) *runtime*)
                 (push (list :type "supervision-verdict"
                             :args (list :task-id (actor-record-id record)
                                         :verdict verdict
                                         :evidence evidence))
                       (runtime-state-chronicle-pending *runtime*)))
               ;; Record learning from outcome
               (%supervision-record-learning record verdict evidence))))
         registry))
      t)))

;;; ─── Chronicle flush handlers ────────────────────────────────────────
;;;
;;; These are called from %tick-chronicle-flush in loop.lisp when
;;; the pending batch contains supervision records.

(defun %chronicle-flush-supervision-spec (args)
  "Insert a supervision spec into chronicle via SQL."
  (ignore-errors
    (let ((task (getf args :task))
          (taxonomy (getf args :taxonomy))
          (spec (getf args :spec))
          (n-assertions (getf args :assertions)))
      (chronicle-query
       (format nil
        "INSERT INTO supervision_specs (task, taxonomy, spec, assertions)
         VALUES (~D, '~A', '~A', ~D)"
        (or task 0)
        (or taxonomy "deferred")
        (%sql-escape (or spec ""))
        (or n-assertions 0))))))

(defun %chronicle-flush-supervision-verdict (args)
  "Update supervision spec with verdict and insert evidence rows."
  (ignore-errors
    (let* ((task-id (getf args :task-id))
           (verdict (getf args :verdict))
           (evidence (getf args :evidence))
           (grade (string-downcase (symbol-name (getf verdict :grade))))
           (confidence (getf verdict :confidence))
           (passed (getf verdict :passed))
           (failed (getf verdict :failed))
           (skipped (getf verdict :skipped)))
      ;; Update spec verdict
      (chronicle-query
       (format nil
        "UPDATE supervision_specs SET verdict='~A', confidence=~,4F,
         passed=~D, failed=~D, skipped=~D
         WHERE task=~D AND verdict IS NULL"
        grade (or confidence 0.0)
        (or passed 0) (or failed 0) (or skipped 0)
        (or task-id 0)))
      ;; Insert evidence rows
      (dolist (e evidence)
        (chronicle-query
         (format nil
          "INSERT INTO supervision_evidence (spec, kind, detail, passed, evidence, duration)
           SELECT id, '~A', '~A', ~D, '~A', ~D
           FROM supervision_specs WHERE task=~D ORDER BY id DESC LIMIT 1"
          (string-downcase (symbol-name (getf e :kind)))
          (%sql-escape (or (getf e :detail) ""))
          (if (getf e :passed) 1 0)
          (%sql-escape (or (getf e :evidence) ""))
          (or (getf e :duration) 0)
          (or task-id 0)))))))

(defun %sql-escape (str)
  "Escape single quotes for SQL string literals."
  (let* ((s (or str ""))
         (result (make-string-output-stream)))
    (dotimes (i (length s))
      (let ((ch (char s i)))
        (write-char ch result)
        (when (char= ch #\')
          (write-char #\' result))))
    (get-output-stream-string result)))
