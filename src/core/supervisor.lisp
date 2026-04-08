;;; supervisor.lisp — Closed-loop task supervision.
;;; Generates specs BEFORE execution, collects evidence AFTER, feeds verdicts back.
;;; Taxonomy: :confirmable :partial :auditable :deferred

(in-package #:harmonia)

(defun %absolute-path-p (path)
  (let ((s (namestring path)))
    (and (> (length s) 0) (char= (char s 0) #\/))))

(defun %read-file-string (path)
  (with-open-file (in path :direction :input :if-does-not-exist nil)
    (when in
      (let ((buf (make-string (file-length in))))
        (read-sequence buf in)
        buf))))

(defun %run-command (command &key directory)
  "Run a shell command, return (values output exit-code)."
  (let* ((cmd (if (listp command) (format nil "~{~A~^ ~}" command) command))
         (proc (sb-ext:run-program "/bin/sh" (list "-c" cmd)
                                   :directory directory :output :stream :error :output :wait t))
         (out (with-output-to-string (s)
                (loop for line = (read-line (sb-ext:process-output proc) nil nil)
                      while line do (write-line line s))))
         (code (sb-ext:process-exit-code proc)))
    (values out code)))

(defparameter *supervision-verb-keywords*
  '((:create  ("create" "generate" "new file" "add file"))
    (:delete  ("delete" "remove file" "rm "))
    (:add-fn  ("add function" "add fn" "add def" "add method" "add class"))
    (:fix     ("fix test" "test pass" "tests should pass" "test should pass"))
    (:compile ("compile" "cargo build" " build " " make "))
    (:test    ("run test" "cargo test" "npm test" "pytest"))))

(defparameter *supervision-file-extensions*
  '("lisp" "rs" "py" "js" "ts" "go" "rb" "c" "h" "toml" "json" "yaml" "yml" "md" "sh" "sql" "css" "html" "sexp"))

(defun %search-any (keywords text)
  (some (lambda (kw) (search kw text)) keywords))

(defun %supervision-classify-task (prompt)
  "Return (:taxonomy T :assertions LIST) via heuristic pattern matching."
  (let* ((assertions (%supervision-heuristic-assertions prompt))
         (n (length assertions))
         (lower (string-downcase prompt))
         (taxonomy (cond
                     ((>= n 2) :confirmable)
                     ((= n 1)  :partial)
                     ((%search-any '("explain" "draft" "summarize" "describe" "analyze" "review") lower)
                      :auditable)
                     (t :deferred))))
    (when (and (member taxonomy '(:confirmable :partial :auditable))
               (not (find :output-present assertions :key (lambda (a) (getf a :kind)))))
      (push (list :kind :output-present) assertions))
    (list :taxonomy taxonomy :assertions assertions)))

(defun %supervision-heuristic-assertions (prompt)
  (let ((assertions '())
        (files (%supervision-extract-files prompt))
        (lower (string-downcase prompt)))
    (dolist (vp *supervision-verb-keywords*)
      (when (%search-any (second vp) lower)
        (case (first vp)
          (:create (dolist (f files) (push (list :kind :file-exists :path f) assertions)))
          (:delete (dolist (f files) (push (list :kind :file-absent :path f) assertions)))
          (:add-fn (let ((name (%supervision-extract-identifier prompt)))
                     (when (and name files)
                       (dolist (f files)
                         (push (list :kind :file-contains :path f :pattern name) assertions)))))
          (:fix    (push (list :kind :test-passes :command "cargo test") assertions))
          (:compile (push (list :kind :compiles :command "cargo build") assertions))
          (:test   (push (list :kind :test-passes
                               :command (%supervision-detect-test-command prompt)) assertions)))))
    (when (and files (not (find :file-exists assertions :key (lambda (a) (getf a :kind)))))
      (dolist (f files) (push (list :kind :file-modified :path f) assertions)))
    (nreverse assertions)))

(defun %supervision-extract-files (prompt)
  (let ((files '()))
    (dolist (word (%split-whitespace prompt))
      (let ((clean (string-right-trim '(#\, #\. #\; #\: #\) #\]) word)))
        (dolist (ext *supervision-file-extensions*)
          (let ((suffix (concatenate 'string "." ext)))
            (when (and (> (length clean) (length suffix))
                       (string-equal suffix (subseq clean (- (length clean) (length suffix))))
                       (find #\/ clean)
                       (not (member clean files :test #'string-equal)))
              (push clean files))))))
    (nreverse files)))

(defun %split-whitespace (str)
  (let ((tokens '()) (start nil))
    (dotimes (i (length str))
      (if (member (char str i) '(#\Space #\Tab #\Newline #\Return))
          (when start (push (subseq str start i) tokens) (setf start nil))
          (unless start (setf start i))))
    (when start (push (subseq str start) tokens))
    (nreverse tokens)))

(defun %supervision-extract-identifier (prompt)
  (let ((lower (string-downcase prompt)))
    (dolist (kw '("function " "fn " "def " "method " "class "))
      (let ((pos (search kw lower)))
        (when pos
          (let* ((start (+ pos (length kw)))
                 (end (or (position-if
                           (lambda (c) (member c '(#\Space #\Tab #\Newline #\( #\) #\, #\;)))
                           prompt :start start)
                          (length prompt)))
                 (name (subseq prompt start end)))
            (when (> (length name) 0) (return name))))))))

(defun %supervision-detect-test-command (prompt)
  (let ((lower (string-downcase prompt)))
    (cond ((search "cargo test" lower) "cargo test")
          ((search "npm test" lower) "npm test")
          ((search "pytest" lower) "pytest")
          ((search "go test" lower) "go test ./...")
          (t "cargo test"))))

(defun %supervision-freeze-spec (task-id taxonomy assertions)
  "Commit spec to chronicle. Once frozen, only verdict fields are updated later."
  (let ((spec-record (list :task (or task-id 0)
                           :taxonomy (string-downcase (symbol-name taxonomy))
                           :spec (prin1-to-string (list :taxonomy taxonomy :assertions assertions))
                           :assertions (length assertions))))
    (when (and (boundp '*runtime*) *runtime*)
      (push (list :type "supervision-spec" :args spec-record)
            (runtime-state-chronicle-pending *runtime*)))
    (list :taxonomy taxonomy :assertions assertions
          :n-assertions (length assertions)
          :frozen-at (get-universal-time) :task-id (or task-id 0))))

(defun %supervision-update-task-id (spec-sexp actor-id)
  (when (and spec-sexp (listp spec-sexp))
    (setf (getf spec-sexp :task-id) actor-id))
  spec-sexp)

(defun %supervision-collect-evidence (spec output workdir)
  "Evaluate assertions against concrete evidence."
  (let ((evidence '()))
    (dolist (assertion (getf spec :assertions))
      (let* ((kind (getf assertion :kind))
             (result (handler-case
                         (case kind
                           (:file-exists    (%supervision-evaluate-file-exists (getf assertion :path) workdir))
                           (:file-absent    (%supervision-evaluate-file-absent (getf assertion :path) workdir))
                           (:file-contains  (%supervision-evaluate-file-contains
                                             (getf assertion :path) (getf assertion :pattern) workdir))
                           (:file-modified  (%supervision-evaluate-file-modified (getf assertion :path) workdir))
                           (:test-passes   (%supervision-evaluate-test-passes (getf assertion :command) workdir))
                           (:compiles      (%supervision-evaluate-compiles (getf assertion :command) workdir))
                           (:output-present (%supervision-evaluate-output-present output))
                           (:semantic      (%supervision-evaluate-semantic (getf assertion :claim) output))
                           (t (list :passed nil :evidence (format nil "unknown kind: ~A" kind) :duration 0)))
                       (error (e)
                         (list :passed nil :evidence (format nil "error: ~A" e) :duration 0)))))
        (push (list :kind kind
                    :detail (or (getf assertion :path) (getf assertion :command)
                                (getf assertion :claim) (string-downcase (symbol-name kind)))
                    :passed (getf result :passed)
                    :evidence (getf result :evidence)
                    :duration (or (getf result :duration) 0))
              evidence)))
    (nreverse evidence)))

(defun %supervision-evaluate-file-exists (path workdir)
  (let* ((full-path (if (%absolute-path-p path) path (merge-pathnames path workdir)))
         (exists (probe-file full-path)))
    (list :passed (not (null exists))
          :evidence (format nil "~:[file not found~;file exists~]: ~A" exists full-path)
          :duration 0)))

(defun %supervision-evaluate-file-absent (path workdir)
  (let* ((full-path (if (%absolute-path-p path) path (merge-pathnames path workdir)))
         (exists (probe-file full-path)))
    (list :passed (null exists)
          :evidence (format nil "~:[file absent (expected)~;file still exists~]: ~A" exists full-path)
          :duration 0)))

(defun %supervision-evaluate-file-contains (path pattern workdir)
  (let ((full-path (if (%absolute-path-p path) path (merge-pathnames path workdir))))
    (if (not (probe-file full-path))
        (list :passed nil :evidence (format nil "file not found: ~A" full-path) :duration 0)
        (let* ((start (get-internal-real-time))
               (content (handler-case (%read-file-string full-path) (error () nil)))
               (found (and content (search pattern content)))
               (elapsed (round (* 1000 (/ (- (get-internal-real-time) start)
                                           internal-time-units-per-second)))))
          (list :passed (not (null found))
                :evidence (format nil "pattern '~A' ~:[not found~;found~] in ~A"
                                  pattern found (pathname-name full-path))
                :duration elapsed)))))

(defun %supervision-evaluate-file-modified (path workdir)
  (let* ((full-path (if (%absolute-path-p path) path (namestring (merge-pathnames path workdir))))
         (start (get-internal-real-time))
         (diff (handler-case (%run-command (list "git" "diff" "--stat" "--" full-path) :directory workdir) (error () nil)))
         (elapsed (round (* 1000 (/ (- (get-internal-real-time) start) internal-time-units-per-second))))
         (modified (and diff (> (length (string-trim '(#\Space #\Newline) diff)) 0))))
    (list :passed modified
          :evidence (format nil "~:[no git changes~;git diff non-empty~] for ~A"
                            modified (pathname-name (pathname full-path)))
          :duration elapsed)))

(defun %supervision-evaluate-test-passes (command workdir)
  (let ((start (get-internal-real-time)) (output "") (exit-code 1))
    (handler-case
        (multiple-value-bind (out code) (%run-command command :directory workdir)
          (setf output (or out "") exit-code (or code 1)))
      (error () nil))
    (list :passed (zerop exit-code)
          :evidence (format nil "exit-code=~D output-length=~D" exit-code (length output))
          :duration (round (* 1000 (/ (- (get-internal-real-time) start) internal-time-units-per-second))))))

(defun %supervision-evaluate-compiles (command workdir)
  (let ((start (get-internal-real-time)) (exit-code 1))
    (handler-case
        (multiple-value-bind (_out code) (%run-command command :directory workdir)
          (declare (ignore _out)) (setf exit-code (or code 1)))
      (error () nil))
    (list :passed (zerop exit-code)
          :evidence (format nil "compilation exit-code=~D" exit-code)
          :duration (round (* 1000 (/ (- (get-internal-real-time) start) internal-time-units-per-second))))))

(defun %supervision-evaluate-output-present (output)
  (let ((trimmed (string-trim '(#\Space #\Newline #\Tab) (or output ""))))
    (list :passed (> (length trimmed) 0)
          :evidence (format nil "output-length=~D" (length trimmed))
          :duration 0)))

(defun %supervision-evaluate-semantic (claim output)
  (let* ((words (%split-whitespace (string-downcase (or claim ""))))
         (output-lower (string-downcase (or output "")))
         (total (max 1 (count-if (lambda (w) (> (length w) 3)) words)))
         (matches (count-if (lambda (w) (and (> (length w) 3) (search w output-lower))) words))
         (ratio (/ matches total)))
    (list :passed (> ratio 0.3)
          :evidence (format nil "semantic-match ~,1F% (~D/~D keywords)" (* 100.0 ratio) matches total)
          :duration 0)))

(defun %supervision-verdict (evidence taxonomy)
  (let* ((passed-count (count-if (lambda (e) (getf e :passed)) evidence))
         (failed-count (count-if (lambda (e) (and (not (getf e :passed)) (getf e :evidence))) evidence))
         (skipped-count (- (length evidence) passed-count failed-count))
         (total (max 1 (length evidence)))
         (confidence (case taxonomy
                       (:confirmable (/ (float passed-count) total))
                       (:partial (+ 0.5 (* 0.5 (/ (float passed-count) total))))
                       (:auditable (if (> passed-count 0) 0.4 0.0))
                       (t 0.0)))
         (grade (cond ((eq taxonomy :deferred) :deferred)
                      ((= passed-count total) :confirmed)
                      ((> passed-count failed-count) :partial)
                      (t :failed))))
    (list :grade grade :passed passed-count :failed failed-count
          :skipped skipped-count :confidence (float confidence)
          :summary (format nil "~A: ~D/~D passed~@[, ~D failed~]~@[, ~D skipped~]"
                           (string-downcase (symbol-name grade)) passed-count total
                           (when (> failed-count 0) failed-count)
                           (when (> skipped-count 0) skipped-count)))))

(defun %supervision-rate ()
  "Rolling pass rate across recent supervised tasks."
  (let ((window (harmony-policy-number "supervision/window" 64)))
    (handler-case
        (let* ((results (chronicle-query
                         (format nil "SELECT verdict, confidence FROM supervision_specs
                          WHERE verdict IS NOT NULL ORDER BY ts DESC LIMIT ~D" (round window))))
               (total (length results))
               (passed (count-if (lambda (v)
                                   (member (getf v :verdict) '("confirmed" "partial")
                                           :test #'string-equal))
                                 results)))
          (if (> total 0) (%safe-div (float passed) (float total)) 0.5))
      (error () 0.5))))

(defun %supervision-record-learning (record verdict evidence)
  "Store supervision outcome as memory for future learning."
  (handler-case

      (let* ((grade (getf verdict :grade)

    (error () nil))
           (failures (remove-if (lambda (e) (getf e :passed)) evidence))
           (successes (remove-if-not (lambda (e) (getf e :passed)) evidence))
           (summary (format nil "Task: ~A~%Grade: ~A~%Confidence: ~,2F~%Passed: ~D, Failed: ~D~%~
                                ~@[Mistakes: ~{~A~^, ~}~]~%~@[Succeeded: ~{~A~^, ~}~]"
                            (%clip-prompt (actor-record-prompt record) 200)
                            (string-downcase (symbol-name grade))
                            (getf verdict :confidence)
                            (getf verdict :passed) (getf verdict :failed)
                            (mapcar (lambda (f) (getf f :detail)) failures)
                            (mapcar (lambda (s) (getf s :detail)) successes)))
           (tags (append '(:supervision :learning)
                         (when (member grade '(:failed :partial)) '(:mistakes))
                         (when (eq grade :confirmed) '(:success-pattern)))))
      (memory-put :supervision summary :tags tags)
      (when (and (boundp '*runtime*) *runtime*)
        (push (list :type "memory" :args (list "supervision-learning"
                                               :entries-created 1 :detail summary))
              (runtime-state-chronicle-pending *runtime*))))))

(defun %clip-prompt (prompt max-len)
  (if (> (length prompt) max-len) (subseq prompt 0 max-len) prompt))

(defun %tick-supervision (runtime)
  "Scan completed actors with specs but no verdict; collect evidence and compute verdicts."
  (%supervised-action "supervision"
    (lambda ()
      (maphash
       (lambda (actor-id record)
         (declare (ignore actor-id))
         (when (and (eq (actor-record-state record) :completed)
                    (actor-record-supervision-spec record)
                    (null (actor-record-supervision-grade record)))
           (let* ((spec (actor-record-supervision-spec record))
                  (output (or (actor-record-result record) ""))
                  (workdir (or (handler-case (config-get-for "conductor" "workdir") (error () nil))
                               (namestring (user-homedir-pathname))))
                  (evidence (%supervision-collect-evidence spec output workdir))
                  (verdict (%supervision-verdict evidence (getf spec :taxonomy))))
             (setf (actor-record-supervision-grade record) (getf verdict :grade)
                   (actor-record-supervision-confidence record) (getf verdict :confidence))
             (when (and (boundp '*runtime*) *runtime*)
               (push (list :type "supervision-verdict"
                           :args (list :task-id (actor-record-id record)
                                       :verdict verdict :evidence evidence))
                     (runtime-state-chronicle-pending *runtime*)))
             (%supervision-record-learning record verdict evidence))))
       (runtime-state-actor-registry runtime))
      t)))

(defun %chronicle-flush-supervision-spec (args)
  (handler-case
      (chronicle-query
       (format nil "INSERT INTO supervision_specs (task, taxonomy, spec, assertions)
        VALUES (~D, '~A', '~A', ~D)"
        (or (getf args :task) 0)
        (or (getf args :taxonomy) "deferred")
        (%sql-escape (or (getf args :spec) ""))
        (or (getf args :assertions) 0)))
    (error (e) (%log :warn "supervision" "flush-spec failed: ~A" e) nil)))

(defun %chronicle-flush-supervision-verdict (args)
  (handler-case

      (let* ((task-id (getf args :task-id)

    (error () nil))
           (verdict (getf args :verdict))
           (grade (string-downcase (symbol-name (getf verdict :grade)))))
      (chronicle-query
       (format nil "UPDATE supervision_specs SET verdict='~A', confidence=~,4F,
        passed=~D, failed=~D, skipped=~D WHERE task=~D AND verdict IS NULL"
        grade (or (getf verdict :confidence) 0.0)
        (or (getf verdict :passed) 0) (or (getf verdict :failed) 0)
        (or (getf verdict :skipped) 0) (or task-id 0)))
      (dolist (e (getf args :evidence))
        (chronicle-query
         (format nil "INSERT INTO supervision_evidence (spec, kind, detail, passed, evidence, duration)
          SELECT id, '~A', '~A', ~D, '~A', ~D
          FROM supervision_specs WHERE task=~D ORDER BY id DESC LIMIT 1"
          (string-downcase (symbol-name (getf e :kind)))
          (%sql-escape (or (getf e :detail) ""))
          (if (getf e :passed) 1 0)
          (%sql-escape (or (getf e :evidence) ""))
          (or (getf e :duration) 0)
          (or task-id 0)))))))

(defun %sql-escape (str)
  (let ((s (or str ""))
        (result (make-string-output-stream)))
    (dotimes (i (length s))
      (let ((ch (char s i)))
        (write-char ch result)
        (when (char= ch #\') (write-char #\' result))))
    (get-output-stream-string result)))
