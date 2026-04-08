;;; presentation.lisp — Presentation analysis, sanitization, telemetry, feedback, guidance.

(in-package :harmonia)

(defparameter *presentation-feedback-max* 32)

(defparameter *presentation-correction-markers*
  '("don't" "do not" "avoid" "stop" "too " "less " "more " "simpler" "keep it simple"
    "plain" "clean" "ugly" "chaotic" "messy" "noisy" "mirror" "looks bad"))

(defparameter *presentation-positive-markers*
  '("beautiful" "clear" "clean" "good" "better" "nice"))

(defun %presentation-count-substring (haystack needle)
  (let ((s (or haystack "")) (count 0) (start 0) (nlen (length needle)))
    (when (zerop nlen) (return-from %presentation-count-substring 0))
    (loop for pos = (search needle s :start2 start :test #'char=) while pos
          do (incf count) (setf start (+ pos nlen))
          finally (return count))))

(defun %presentation-line-count (text)
  (if (or (null text) (zerop (length text))) 0
      (1+ (%presentation-count-substring text (string #\Newline)))))

(defun %presentation-word-count (text)
  (let ((count 0) (in-word nil))
    (loop for ch across (or text "") do
      (if (or (alphanumericp ch) (char= ch #\_))
          (unless in-word (setf in-word t) (incf count))
          (setf in-word nil)))
    count))

(defun %presentation-non-ascii-char-p (ch)
  (> (char-code ch) 127))

(defun %presentation-decorative-char-p (ch)
  (let ((code (char-code ch)))
    (or (and (>= code #x2500) (<= code #x257F))
        (and (>= code #x2580) (<= code #x259F))
        (and (>= code #x25A0) (<= code #x25FF))
        (and (>= code #x2800) (<= code #x28FF))
        (and (>= code #x2190) (<= code #x21FF))
        (find ch "◆◇■□▲△▼▽○●◦•▪▫★☆✦✧✶✳✴✱✴" :test #'char=))))

(defun %presentation-control-char-p (ch)
  (let ((code (char-code ch)))
    (and (< code 32) (not (member ch '(#\Newline #\Tab #\Return) :test #'char=)))))

(defun %presentation-csi-final-byte-p (ch)
  (let ((code (char-code ch)))
    (and (>= code 64) (<= code 126))))

(defmacro %with-char-accumulator ((input-string) &body body)
  "Process INPUT-STRING char-by-char. BODY receives S, LEN, I and flet EMIT. Must advance I."
  (let ((out (gensym "OUT")) (s-var (gensym "S")) (len-var (gensym "LEN")))
    `(let* ((,s-var (or ,input-string "")) (,len-var (length ,s-var)))
       (with-output-to-string (,out)
         (flet ((emit (ch) (write-char ch ,out)))
           (loop with s = ,s-var with len = ,len-var with i = 0 while (< i len) do
             (macrolet ((advance (&optional (n 1)) `(incf i ,n))
                        (peek (&optional (offset 0)) `(char s (+ i ,offset)))
                        (has-ahead (n) `(< (+ i ,n) len)))
               ,@body)))))))

(defun %presentation-skip-csi (s i len)
  (loop while (and (< i len) (not (%presentation-csi-final-byte-p (char s i)))) do (incf i))
  (if (< i len) (1+ i) i))

(defun %presentation-skip-osc (s i len)
  (loop while (< i len) do
    (let ((osc (char s i)))
      (cond ((char= osc #\Bell) (return (1+ i)))
            ((and (char= osc #\Esc) (< (1+ i) len) (char= (char s (1+ i)) #\\))
             (return (+ i 2)))
            (t (incf i)))))
  i)

(defun %presentation-strip-ansi-and-controls (text)
  (%with-char-accumulator (text)
    (let ((ch (peek)))
      (cond
        ((char= ch #\Esc)
         (advance)
         (when (< i len)
           (case (peek)
             (#\[ (setf i (%presentation-skip-csi s (1+ i) len)))
             (#\] (setf i (%presentation-skip-osc s (1+ i) len)))
             (otherwise (advance)))))
        ((%presentation-control-char-p ch) (advance))
        (t (emit ch) (advance))))))

(defun %presentation-normalize-literal-escapes (text)
  "Convert literal \\n/\\r/\\t to real whitespace to prevent 'nn' artifacts."
  (let* ((s (or text ""))
         (literal-n (%presentation-count-substring s "\\n"))
         (literal-r (%presentation-count-substring s "\\r")))
    (if (and (plusp (+ literal-n literal-r))
             (not (search "```" s :test #'char=))
             (not (search "\\x" s :test #'char=)))
        (%with-char-accumulator (text)
          (let ((ch (peek)))
            (if (and (char= ch #\\) (has-ahead 1))
                (case (peek 1)
                  (#\n (emit #\Newline) (advance 2))
                  (#\r (advance 2))
                  (#\t (emit #\Space) (advance 2))
                  (otherwise (emit ch) (advance)))
                (progn (emit ch) (advance)))))
        s)))

(defun %presentation-sanitize-line (line)
  (string-right-trim '(#\Space #\Tab)
    (with-output-to-string (out)
      (loop for ch across (or line "") do
        (cond ((%presentation-control-char-p ch))
              ((member ch '(#\Tab #\Return) :test #'char=) (write-char #\Space out))
              ((member ch '(#\` #\~) :test #'char=) (write-char ch out))
              ((%presentation-decorative-char-p ch) (write-char #\Space out))
              ((and (%presentation-non-ascii-char-p ch) (not (alphanumericp ch)))
               (write-char #\Space out))
              (t (write-char ch out)))))))

(defun %presentation-noisy-line-p (line)
  (let ((chars 0) (decor 0) (alpha 0))
    (loop for ch across (or line "") do
      (unless (member ch '(#\Space #\Tab) :test #'char=)
        (incf chars)
        (when (%presentation-decorative-char-p ch) (incf decor))
        (when (alphanumericp ch) (incf alpha))))
    (and (> chars 0) (zerop alpha) (> (/ decor (float chars)) 0.6))))

(defun %presentation-sanitize-visible-text (text)
  (let ((lines '()))
    (with-input-from-string (in (%presentation-normalize-literal-escapes
                                 (%presentation-strip-ansi-and-controls text)))
      (loop for line = (read-line in nil nil) while line
            unless (%presentation-noisy-line-p line)
              do (push (%presentation-sanitize-line line) lines)))
    (let ((collapsed (string-trim '(#\Space #\Tab #\Newline)
                       (with-output-to-string (out)
                         (loop for line in (nreverse lines) for idx from 0
                               do (when (> idx 0) (terpri out)) (write-string line out))))))
      (if (zerop (length collapsed)) "" collapsed))))

(defun %presentation-symbolic-density (text)
  (let ((symbols 0) (total 0))
    (loop for ch across (or text "") do
      (unless (member ch '(#\Space #\Tab #\Newline #\Return) :test #'char=)
        (incf total)
        (when (or (find ch "!@#$%^&*()[]{}<>|/:;=+_~" :test #'char=)
                  (%presentation-decorative-char-p ch))
          (incf symbols))))
    (if (> total 0) (/ symbols (float total)) 0.0)))

(defun %presentation-markdown-density (text)
  (let ((lines (max 1 (%presentation-line-count text))) (marked 0))
    (with-input-from-string (in (or text ""))
      (loop for line = (read-line in nil nil) while line do
        (let ((trimmed (string-left-trim '(#\Space #\Tab) line)))
          (when (or (search "```" trimmed :test #'char=)
                    (and (> (length trimmed) 1)
                         (member (char trimmed 0) '(#\# #\- #\* #\>) :test #'char=)))
            (incf marked)))))
    (/ marked (float lines))))

(defun %presentation-self-reference-score (text)
  (let* ((s (string-downcase (or text "")))
         (words (max 1 (%presentation-word-count s)))
         (hits (+ (%presentation-count-substring s " i ")
                  (%presentation-count-substring s " i'")
                  (%presentation-count-substring s " i am")
                  (%presentation-count-substring s " my ")
                  (%presentation-count-substring s " myself"))))
    (min 1.0 (/ hits (float words)))))

(defun %presentation-decor-density (text)
  (let ((decor 0) (total 0))
    (loop for ch across (or text "") do
      (unless (member ch '(#\Space #\Tab #\Newline #\Return) :test #'char=)
        (incf total)
        (when (%presentation-decorative-char-p ch) (incf decor))))
    (if (> total 0) (/ decor (float total)) 0.0)))

(defun %presentation-user-affinity (&optional (runtime *runtime*))
  (let ((events (and runtime (runtime-state-presentation-feedback runtime))))
    (if (null events) 0.5
        (let ((sum 0.0) (weight-sum 0.0))
          (dolist (event events)
            (let ((w (float (or (getf event :weight) 0.0)))
                  (a (float (or (getf event :affinity) 0.5))))
              (incf sum (* a w)) (incf weight-sum w)))
          (if (> weight-sum 0.0) (/ sum weight-sum) 0.5)))))

(defparameter *presentation-feedback-min-events* 3)

(defun %presentation-active-feedback-tags (&optional (runtime *runtime*))
  (let ((table (make-hash-table :test 'eq))
        (counts (make-hash-table :test 'eq)))
    (when runtime
      (dolist (event (runtime-state-presentation-feedback runtime))
        (when (< (or (getf event :affinity) 0.5) 0.45)
          (dolist (tag (getf event :tags))
            (incf (gethash tag table 0.0) (float (or (getf event :weight) 0.0)))
            (incf (gethash tag counts 0))))))
    (let ((pairs '()))
      (maphash (lambda (tag weight)
                 (when (and (> weight 0.6)
                            (>= (gethash tag counts 0) *presentation-feedback-min-events*))
                   (push (cons tag weight) pairs)))
               table)
      (mapcar #'car (sort pairs #'> :key #'cdr)))))

(defun %telemetry-text-metrics (visible-text)
  (list :verbosity (%presentation-word-count visible-text)
        :visible-length (length visible-text)))

(defun %telemetry-quality-metrics (raw-text)
  (let* ((raw-len (max 1 (length raw-text)))
         (ansi-count (%presentation-count-substring raw-text (string #\Esc)))
         (control-count (count-if #'%presentation-control-char-p raw-text))
         (literal-escape-count (+ (%presentation-count-substring raw-text "\\n")
                                  (%presentation-count-substring raw-text "\\r")
                                  (%presentation-count-substring raw-text "\\t")))
         (decor-density (%presentation-decor-density raw-text)))
    (list :cleanliness (max 0.0 (- 1.0 (min 1.0
                         (+ (* 0.16 (/ ansi-count (float raw-len)))
                            (* 0.12 (/ control-count (float raw-len)))
                            (* 0.10 (/ literal-escape-count (float raw-len)))
                            (* 0.55 decor-density)))))
          :ansi-count ansi-count :control-count control-count
          :literal-escape-count literal-escape-count
          :decor-density decor-density :raw-length (length raw-text))))

(defun %telemetry-feedback-metrics (visible-text &optional (runtime *runtime*))
  (list :self-reference (%presentation-self-reference-score visible-text)
        :markdown-density (%presentation-markdown-density visible-text)
        :symbolic-density (%presentation-symbolic-density visible-text)
        :user-affinity (%presentation-user-affinity runtime)
        :feedback-tags (%presentation-active-feedback-tags runtime)))

(defun %presentation-response-telemetry (raw visible &optional (runtime *runtime*))
  (append (%telemetry-quality-metrics (or raw ""))
          (%telemetry-text-metrics (or visible ""))
          (%telemetry-feedback-metrics (or visible "") runtime)))

(defun %presentation-next-response-id (&optional (runtime *runtime*))
  (let ((rt (or runtime *runtime*)))
    (if rt
        (progn (incf (runtime-state-response-seq rt))
               (format nil "resp-~D" (runtime-state-response-seq rt)))
        (format nil "resp-~D" (get-universal-time)))))

(defun %presentation-clip-note (text &optional (limit 240))
  (let ((trimmed (string-trim '(#\Space #\Tab #\Newline) (or text ""))))
    (if (<= (length trimmed) limit) trimmed (subseq trimmed 0 limit))))

(defun %presentation-contains-any-p (text needles)
  (let ((s (string-downcase (or text ""))))
    (some (lambda (needle) (search needle s :test #'char=)) needles)))

(defun %presentation-feedback-tags-from-text (text)
  (let ((s (string-downcase (or text ""))) (tags '()))
    (when (%presentation-contains-any-p s '("ansi" "escape" "\\x1b" "control sequence" "control byte"))
      (push :ansi tags) (push :control tags))
    (when (%presentation-contains-any-p s '("box" "glyph" "character" "characters"
                                            "decor" "decorative" "chaotic" "ugly"
                                            "messy" "noisy" "terminal"))
      (push :decor-density tags))
    (when (%presentation-contains-any-p s '("simple" "simpler" "plain" "plain text" "clean"))
      (push :simplicity tags))
    (when (%presentation-contains-any-p s '("verbose" "too long" "shorter" "brief" "concise"))
      (push :verbosity tags))
    (when (%presentation-contains-any-p s '("status" "telemetry" "constitution" "schema" "yaml" "json"))
      (push :telemetry tags))
    (when (%presentation-contains-any-p s '("self reference" "talk about yourself" "i am" "about you"))
      (push :self-reference tags))
    (remove-duplicates (nreverse tags) :test #'eq)))

(defun %presentation-feedback-analysis (text &key (source :implicit) explicit-p)
  (let* ((s (string-downcase (or text "")))
         (corrective-p (or explicit-p (%presentation-contains-any-p s *presentation-correction-markers*)))
         (positive-p (and (not corrective-p) (%presentation-contains-any-p s *presentation-positive-markers*)))
         (tags (%presentation-feedback-tags-from-text s)))
    (when (or (and corrective-p tags) positive-p)
      (list :source source :tags tags
            :weight (if explicit-p 1.0 (if corrective-p 0.8 0.45))
            :affinity (if corrective-p 0.2 (if positive-p 0.8 0.5))
            :note (%presentation-clip-note text)
            :positive-p positive-p))))

(defun %presentation-record-feedback-event (event &optional (runtime *runtime*))
  (when (and event runtime)
    (let ((record (append (list :time (get-universal-time)
                                :response-id (or (getf event :response-id)
                                                 (getf (first (runtime-state-responses runtime)) :response-id)))
                          event)))
      (push record (runtime-state-presentation-feedback runtime))
      (when (> (length (runtime-state-presentation-feedback runtime)) *presentation-feedback-max*)
        (setf (runtime-state-presentation-feedback runtime)
              (subseq (runtime-state-presentation-feedback runtime) 0 *presentation-feedback-max*)))
      (handler-case

          (memory-put :feedback record :tags (append '(:feedback :presentation) (getf record :tags)

        (error () nil))))
      (push (list :type "memory" :args (list "human-feedback" :entries-created 1
                                             :detail (prin1-to-string record)))
            (runtime-state-chronicle-pending runtime))
      (runtime-log runtime :human-feedback record)
      record)))

(defun %presentation-maybe-record-feedback (text &key (source :implicit) explicit-p
                                                 (runtime *runtime*))
  (let ((event (%presentation-feedback-analysis text :source source :explicit-p explicit-p)))
    (when event (%presentation-record-feedback-event event runtime))))

(defun %presentation-input-artifact-note (text)
  (let ((s (or text "")))
    (when (or (search (string #\Esc) s :test #'char=)
              (search "\\n" s :test #'char=) (search "\\t" s :test #'char=)
              (search "\\r" s :test #'char=)
              (find-if #'%presentation-decorative-char-p s))
      "Input may contain copied terminal artifacts or escaped formatting. Interpret semantically and do not mirror the contamination unless the user explicitly asks to inspect it.")))

(defun %guidance-presentation-delta (presentation key)
  (or (and presentation (getf presentation key)) 0.0))

(defun %guidance-default-lines (defaults)
  (if (listp defaults) defaults
      (list "Keep visible replies clear and readable. Your harmonic voice and personality are welcome -- avoid only raw status dumps and telemetry noise."
            "Never emit ANSI escapes, raw control bytes, decorative terminal frames, or copied UI glyphs in visible replies."
            "Keep raw runtime diagnostics and telemetry data internal. Your identity, principles, and harmonic worldview are part of who you are -- express them naturally.")))

(defun %guidance-conditional-lines (cond-prompts tags presentation)
  (let ((decor (%guidance-presentation-delta presentation :decor-density-delta))
        (symbolic (%guidance-presentation-delta presentation :symbolic-density-delta))
        (self-ref (%guidance-presentation-delta presentation :self-reference-delta))
        (verbosity (%guidance-presentation-delta presentation :verbosity-delta))
        (markdown (%guidance-presentation-delta presentation :markdown-density-delta))
        (rules (list (list (lambda (d s v m sr tags)
                       (declare (ignore s v m sr))
                       (or (< d -0.03) (member :decor-density tags)))
                     :decor "Use plain text and light markdown only. No banners, box drawing, status blocks, or ceremonial framing.")
                    (list (lambda (d s v m sr tags)
                       (declare (ignore d v m sr))
                       (or (< s -0.03) (member :simplicity tags)))
                     :symbolic "Prefer straightforward wording over symbolic compression, schemas, or ritual phrasing.")
                    (list (lambda (d s v m sr tags)
                       (declare (ignore d s v m))
                       (or (< sr -0.03) (member :self-reference tags) (member :telemetry tags)))
                     :self-reference "Avoid talking about yourself unless the user is directly asking about identity or internals.")
                    (list (lambda (d s v m sr tags)
                       (declare (ignore d s m sr))
                       (or (< v -0.03) (member :verbosity tags)))
                     :verbosity "Default to concise answers unless the user asks for depth.")
                    (list (lambda (d s v m sr tags)
                       (declare (ignore d s v sr tags))
                       (> m 0.08))
                     :markdown "If structure helps, use light markdown with short headings or flat bullets."))))
    (mapcan (lambda (rule)
              (destructuring-bind (pred key fallback) rule
                (when (funcall pred decor symbolic verbosity markdown self-ref tags)
                  (list (or (getf cond-prompts key) fallback)))))
            rules)))

(defun %presentation-guidance-lines (&optional (runtime *runtime*))
  (let* ((projection (and (fboundp 'signalograd-current-projection)
                          (signalograd-current-projection runtime)))
         (presentation (and (listp projection) (getf projection :presentation)))
         (tags (%presentation-active-feedback-tags runtime))
         (defaults (load-prompt :evolution :presentation-guidance :defaults nil))
         (cond-prompts (load-prompt :evolution :presentation-guidance :conditional nil)))
    (remove-duplicates
     (append (%guidance-default-lines defaults)
             (%guidance-conditional-lines cond-prompts tags presentation))
     :test #'string=)))

(defun %presentation-context-block (user-prompt &optional (runtime *runtime*))
  (let ((lines (%presentation-guidance-lines runtime))
        (artifact-note (%presentation-input-artifact-note user-prompt)))
    (with-output-to-string (out)
      (when (or artifact-note lines)
        (terpri out) (terpri out)
        (write-string (load-prompt :genesis :visible-reply-policy-header nil
                                   "VISIBLE_REPLY_POLICY:") out)
        (terpri out)
        (dolist (line lines) (write-string "- " out) (write-string line out) (terpri out))
        (when artifact-note
          (write-string "- " out) (write-string artifact-note out) (terpri out))))))

(defun %presentation-record-response (prompt raw-response
                                      &key visible-response origin model score harmony
                                           memory-id telemetry-extra (runtime *runtime*))
  (let* ((visible (or visible-response (%presentation-sanitize-visible-text raw-response)))
         (response-id (%presentation-next-response-id runtime))
         (telemetry (append (%presentation-response-telemetry raw-response visible runtime)
                            telemetry-extra))
         (record (append (list :response-id response-id :origin origin :prompt prompt
                               :response visible :raw-response (or raw-response "") :telemetry telemetry)
                         (when model (list :model model))
                         (when score (list :score score))
                         (when harmony (list :harmony harmony))
                         (when memory-id (list :memory-id memory-id)))))
    (when runtime
      (setf (runtime-state-last-response-telemetry runtime) telemetry)
      (push record (runtime-state-responses runtime))
      (push (list :type "memory" :args (list "response-telemetry" :entries-created 1
                                             :detail (prin1-to-string
                                                      (list :response-id response-id
                                                            :origin origin :telemetry telemetry))))
            (runtime-state-chronicle-pending runtime))
      (runtime-log runtime :response-telemetry
                   (list :response-id response-id :origin origin
                         :cleanliness (getf telemetry :cleanliness)
                         :user-affinity (getf telemetry :user-affinity))))
    (values visible response-id telemetry)))
