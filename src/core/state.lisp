;;; state.lisp — Runtime state container.

(in-package :harmonia)

;;; --- Security kernel: typed baseband channel envelope ---

(defstruct harmonia-channel
  kind
  address
  label)

(defstruct harmonia-peer
  id
  origin-fp
  agent-fp
  device-id
  platform
  device-model
  app-version
  a2ui-version)

(defstruct harmonia-origin
  node-id
  node-label
  node-role
  channel-class
  node-key-id
  transport-security
  remote-p)

(defstruct harmonia-session
  id
  label)

(defstruct harmonia-body
  format
  text
  raw)

(defstruct harmonia-security
  label
  source
  fingerprint-valid-p)

(defstruct harmonia-audit
  timestamp-ms
  dissonance)

(defstruct harmonia-transport
  kind
  raw-address
  raw-metadata)

(defstruct harmonia-signal
  id
  version
  kind
  type-name
  channel
  peer
  conversation-id
  origin
  session
  body
  capabilities
  security
  audit
  attachments
  transport
  taint)

(defun harmonia-signal-channel-kind (signal)
  (and signal (harmonia-channel-kind (harmonia-signal-channel signal))))

(defun harmonia-signal-channel-address (signal)
  (and signal (harmonia-channel-address (harmonia-signal-channel signal))))

(defun harmonia-signal-channel-label (signal)
  (and signal (harmonia-channel-label (harmonia-signal-channel signal))))

(defun harmonia-signal-frontend (signal)
  (harmonia-signal-channel-kind signal))

(defun harmonia-signal-sub-channel (signal)
  (harmonia-signal-channel-address signal))

(defun harmonia-signal-payload (signal)
  (and signal (harmonia-body-text (harmonia-signal-body signal))))

(defun harmonia-signal-security-label (signal)
  (and signal (harmonia-security-label (harmonia-signal-security signal))))

(defun harmonia-signal-dissonance (signal)
  (and signal (harmonia-audit-dissonance (harmonia-signal-audit signal))))

(defun harmonia-signal-timestamp-ms (signal)
  (and signal (harmonia-audit-timestamp-ms (harmonia-signal-audit signal))))

(defun harmonia-signal-origin-fp (signal)
  (and signal (harmonia-peer-origin-fp (harmonia-signal-peer signal))))

(defun harmonia-signal-agent-fp (signal)
  (and signal (harmonia-peer-agent-fp (harmonia-signal-peer signal))))

(defun harmonia-signal-device-id (signal)
  (and signal (harmonia-peer-device-id (harmonia-signal-peer signal))))

(defun harmonia-signal-origin-node-id (signal)
  (and signal (harmonia-origin-node-id (harmonia-signal-origin signal))))

(defun harmonia-signal-origin-node-label (signal)
  (and signal (harmonia-origin-node-label (harmonia-signal-origin signal))))

(defun harmonia-signal-origin-node-role (signal)
  (and signal (harmonia-origin-node-role (harmonia-signal-origin signal))))

(defun harmonia-signal-channel-class (signal)
  (and signal (harmonia-origin-channel-class (harmonia-signal-origin signal))))

(defun harmonia-signal-origin-node-key-id (signal)
  (and signal (harmonia-origin-node-key-id (harmonia-signal-origin signal))))

(defun harmonia-signal-transport-security (signal)
  (and signal (harmonia-origin-transport-security (harmonia-signal-origin signal))))

(defun harmonia-signal-remote-p (signal)
  (and signal (harmonia-origin-remote-p (harmonia-signal-origin signal))))

(defun harmonia-signal-session-id (signal)
  (and signal (harmonia-session-id (harmonia-signal-session signal))))

(defun harmonia-signal-platform (signal)
  (and signal (harmonia-peer-platform (harmonia-signal-peer signal))))

(defun harmonia-signal-a2ui-version (signal)
  (and signal (harmonia-peer-a2ui-version (harmonia-signal-peer signal))))

(defun harmonia-signal-has-capability-p (signal capability)
  (let ((caps (harmonia-signal-capabilities signal)))
    (and (listp caps)
         (let ((probe (intern (string-upcase capability) :keyword)))
           (or (getf caps probe)
               (getf caps capability))))))

(defvar *current-originating-signal* nil
  "The signal that initiated the current orchestration chain. Used by policy gate.")

;;; --- Actor record: tracks a non-blocking CLI subagent ---

(defstruct actor-record
  id                        ;; u64 from Rust tmux-spawn
  model                     ;; "cli:claude-code"
  prompt                    ;; original prompt text
  state                     ;; :spawning :running :completed :failed :stalled
  spawned-at                ;; universal-time
  last-heartbeat            ;; universal-time of last progress
  originating-signal        ;; harmonia-signal or nil (for gateway delivery)
  result                    ;; string output when completed
  error-text                ;; string when failed
  cost-usd                  ;; float
  duration-ms               ;; integer
  stall-ticks               ;; count of ticks with no heartbeat
  orchestration-context     ;; plist with :chain, :prepared-prompt, etc.
  swarm-group-id            ;; groups actors from same parallel-solve invocation
  supervision-spec          ;; frozen supervision spec s-expression (or nil)
  supervision-grade         ;; :confirmed :partial :failed :deferred nil
  supervision-confidence)   ;; 0.0-1.0

(defstruct (runtime-state
             (:constructor make-runtime-state
                           (&key
                            (running t)
                            (cycle 0)
                            (started-at (get-universal-time))
                            (last-tick-at nil)
                            (tools (make-hash-table :test 'equal))
                            (events '())
                            (prompt-queue '())
                            (responses '())
                            (rewrite-count 0)
                            (environment "test")
                            (active-model nil)
                            (harmonic-phase :observe)
                            (harmonic-context '())
                            (harmonic-x 0.5)
                            (harmonic-r 3.45)
                            (lorenz-x 0.1)
                            (lorenz-y 0.0)
                            (lorenz-z 0.0)
                            (actor-registry (make-hash-table :test 'eql))
                            (actor-pending '())
                            (actor-kinds (make-hash-table :test 'equal))
                            (chronicle-pending '())
                            (gateway-actor-id nil)
                            (tailnet-actor-id nil)
                            (chronicle-actor-id nil)
                            (response-seq 0)
                            (presentation-feedback '())
                            (last-response-telemetry '())
                            (signalograd-projection '())
                            (signalograd-last-updated-at 0))))
  running
  cycle
  started-at
  last-tick-at
  tools
  events
  prompt-queue
  responses
  rewrite-count
  environment
  active-model
  harmonic-phase
  harmonic-context
  harmonic-x
  harmonic-r
  lorenz-x
  lorenz-y
  lorenz-z
  actor-registry              ;; hash-table: actor-id -> actor-record
  actor-pending               ;; list of actor-ids awaiting completion
  actor-kinds                 ;; hash-table: actor-id -> kind string ("gateway", "cli-agent", etc.)
  chronicle-pending           ;; list of chronicle recording plists batched per tick
  gateway-actor-id            ;; actor-id of gateway actor (or nil)
  tailnet-actor-id            ;; actor-id of tailnet actor (or nil)
  chronicle-actor-id          ;; actor-id of chronicle actor (or nil)
  response-seq                ;; monotonically increasing internal response id
  presentation-feedback       ;; recent human feedback events (latest first)
  last-response-telemetry     ;; hidden telemetry sidecar for the last visible reply
  signalograd-projection      ;; last applied signalograd proposal plist
  signalograd-last-updated-at) ;; universal time of last applied proposal

(defun runtime-log (runtime tag payload)
  (push (list :time (get-universal-time) :tag tag :payload payload)
        (runtime-state-events runtime))
  runtime)

(defparameter *presentation-feedback-max* 32)

(defparameter *presentation-correction-markers*
  '("don't" "do not" "avoid" "stop" "too " "less " "more " "simpler" "keep it simple"
    "plain" "clean" "ugly" "chaotic" "messy" "noisy" "mirror" "looks bad"))

(defparameter *presentation-positive-markers*
  '("beautiful" "clear" "clean" "good" "better" "nice"))

(defun %presentation-count-substring (haystack needle)
  (let* ((s (or haystack ""))
         (count 0)
         (start 0)
         (nlen (length needle)))
    (when (zerop nlen)
      (return-from %presentation-count-substring 0))
    (loop
      for pos = (search needle s :start2 start :test #'char=)
      while pos
      do (incf count)
         (setf start (+ pos nlen))
      finally (return count))))

(defun %presentation-line-count (text)
  (if (or (null text) (zerop (length text)))
      0
      (1+ (%presentation-count-substring text (string #\Newline)))))

(defun %presentation-word-count (text)
  (let ((count 0)
        (in-word nil))
    (loop for ch across (or text "") do
      (if (or (alphanumericp ch) (char= ch #\_))
          (unless in-word
            (setf in-word t)
            (incf count))
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
    (and (< code 32)
         (not (member ch '(#\Newline #\Tab #\Return) :test #'char=)))))

(defun %presentation-csi-final-byte-p (ch)
  (let ((code (char-code ch)))
    (and (>= code 64) (<= code 126))))

(defun %presentation-strip-ansi-and-controls (text)
  "Drop ANSI escape sequences and non-printing control bytes."
  (let* ((s (or text ""))
         (len (length s)))
    (with-output-to-string (out)
      (loop with i = 0
            while (< i len) do
        (let ((ch (char s i)))
          (cond
            ((char= ch #\Esc)
             (incf i)
             (when (< i len)
               (case (char s i)
                 (#\[
                  (incf i)
                  (loop while (and (< i len)
                                   (not (%presentation-csi-final-byte-p (char s i))))
                        do (incf i))
                  (when (< i len)
                    (incf i)))
                 (#\]
                  (incf i)
                  (loop while (< i len) do
                    (let ((osc (char s i)))
                      (cond
                        ((char= osc #\Bell)
                         (incf i)
                         (return))
                        ((and (char= osc #\Esc)
                              (< (1+ i) len)
                              (char= (char s (1+ i)) #\\))
                         (incf i 2)
                         (return))
                        (t
                         (incf i))))))
                 (otherwise
                  (incf i)))))
            ((%presentation-control-char-p ch)
             (incf i))
            (t
             (write-char ch out)
             (incf i))))))))

(defun %presentation-normalize-literal-escapes (text)
  "Convert literal \\n / \\r / \\t sequences into real whitespace characters.
   Always converts when literal escapes are present, even if real newlines exist,
   to prevent 'nn' artifacts from mixed-encoding model output."
  (let* ((s (or text ""))
         (literal-n (%presentation-count-substring s "\\n"))
         (literal-r (%presentation-count-substring s "\\r")))
    (if (and (plusp (+ literal-n literal-r))
             (not (search "```" s :test #'char=))
             (not (search "\\x" s :test #'char=)))
        (with-output-to-string (out)
          (loop with i = 0
                with len = (length s)
                while (< i len) do
            (let ((ch (char s i)))
              (if (and (char= ch #\\) (< (1+ i) len))
                  (case (char s (1+ i))
                    (#\n
                     (write-char #\Newline out)
                     (incf i 2))
                    (#\r
                     (incf i 2))
                    (#\t
                     (write-char #\Space out)
                     (incf i 2))
                    (otherwise
                     (write-char ch out)
                     (incf i)))
                  (progn
                    (write-char ch out)
                    (incf i))))))
        s)))

(defun %presentation-sanitize-line (line)
  "Map decorative terminal glyphs to plain ASCII and trim trailing blanks."
  (let* ((s (or line ""))
         (mapped
           (with-output-to-string (out)
             (loop for ch across s do
               (cond
                 ((%presentation-control-char-p ch) nil)
                 ((member ch '(#\Tab #\Return) :test #'char=)
                  (write-char #\Space out))
                 ((member ch '(#\` #\~) :test #'char=)
                  (write-char ch out))
                 ((%presentation-decorative-char-p ch)
                  (write-char #\Space out))
                 ((and (%presentation-non-ascii-char-p ch)
                       (not (alphanumericp ch)))
                  (write-char #\Space out))
                 (t
                  (write-char ch out)))))))
    (string-right-trim '(#\Space #\Tab) mapped)))

(defun %presentation-noisy-line-p (line)
  (let ((chars 0)
        (decor 0)
        (alpha 0))
    (loop for ch across (or line "") do
      (unless (member ch '(#\Space #\Tab) :test #'char=)
        (incf chars)
        (when (%presentation-decorative-char-p ch)
          (incf decor))
        (when (alphanumericp ch)
          (incf alpha))))
    (and (> chars 0)
         (zerop alpha)
         (> (/ decor (float chars)) 0.6))))

(defun %presentation-sanitize-visible-text (text)
  "Normalize visible replies without touching the raw internal model output."
  (let* ((plain (%presentation-strip-ansi-and-controls text))
         (escaped (%presentation-normalize-literal-escapes plain))
         (lines '()))
    (with-input-from-string (in escaped)
      (loop for line = (read-line in nil nil)
            while line do
        (unless (%presentation-noisy-line-p line)
          (push (%presentation-sanitize-line line) lines))))
    (let* ((joined (with-output-to-string (out)
                     (loop for line in (nreverse lines)
                           for idx from 0 do
                       (when (> idx 0)
                         (terpri out))
                       (write-string line out))))
           (collapsed (string-trim '(#\Space #\Tab #\Newline)
                                   joined)))
      (if (zerop (length collapsed))
          ""
          collapsed))))

(defun %presentation-symbolic-density (text)
  (let ((symbols 0)
        (total 0))
    (loop for ch across (or text "") do
      (unless (member ch '(#\Space #\Tab #\Newline #\Return) :test #'char=)
        (incf total)
        (when (or (find ch "!@#$%^&*()[]{}<>|/:;=+_~" :test #'char=)
                  (%presentation-decorative-char-p ch))
          (incf symbols))))
    (if (> total 0) (/ symbols (float total)) 0.0)))

(defun %presentation-markdown-density (text)
  (let ((lines (max 1 (%presentation-line-count text)))
        (marked 0))
    (with-input-from-string (in (or text ""))
      (loop for line = (read-line in nil nil)
            while line do
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
  (let ((decor 0)
        (total 0))
    (loop for ch across (or text "") do
      (unless (member ch '(#\Space #\Tab #\Newline #\Return) :test #'char=)
        (incf total)
        (when (%presentation-decorative-char-p ch)
          (incf decor))))
    (if (> total 0) (/ decor (float total)) 0.0)))

(defun %presentation-user-affinity (&optional (runtime *runtime*))
  (let ((events (and runtime (runtime-state-presentation-feedback runtime))))
    (if (null events)
        0.5
        (let ((sum 0.0)
              (weight-sum 0.0))
          (dolist (event events)
            (let ((weight (float (or (getf event :weight) 0.0)))
                  (affinity (float (or (getf event :affinity) 0.5))))
              (incf sum (* affinity weight))
              (incf weight-sum weight)))
          (if (> weight-sum 0.0)
              (/ sum weight-sum)
              0.5)))))

(defparameter *presentation-feedback-min-events* 3
  "Minimum number of feedback events for a tag before conditional suppression activates.")

(defun %presentation-active-feedback-tags (&optional (runtime *runtime*))
  (let ((table (make-hash-table :test 'eq))
        (counts (make-hash-table :test 'eq)))
    (when runtime
      (dolist (event (runtime-state-presentation-feedback runtime))
        (when (< (or (getf event :affinity) 0.5) 0.45)
          (dolist (tag (getf event :tags))
            (setf (gethash tag table)
                  (+ (gethash tag table 0.0)
                     (float (or (getf event :weight) 0.0))))
            (incf (gethash tag counts 0))))))
    (let ((pairs '()))
      (maphash (lambda (tag weight)
                 (when (and (> weight 0.6)
                            (>= (gethash tag counts 0)
                                *presentation-feedback-min-events*))
                   (push (cons tag weight) pairs)))
               table)
      (mapcar #'car (sort pairs #'> :key #'cdr)))))

(defun %presentation-response-telemetry (raw visible &optional (runtime *runtime*))
  (let* ((raw-text (or raw ""))
         (visible-text (or visible ""))
         (raw-len (max 1 (length raw-text)))
         (ansi-count (%presentation-count-substring raw-text (string #\Esc)))
         (control-count (count-if #'%presentation-control-char-p raw-text))
         (literal-escape-count (+ (%presentation-count-substring raw-text "\\n")
                                  (%presentation-count-substring raw-text "\\r")
                                  (%presentation-count-substring raw-text "\\t")))
         (decor-density (%presentation-decor-density raw-text))
         (verbosity (%presentation-word-count visible-text))
         (self-reference (%presentation-self-reference-score visible-text))
         (markdown-density (%presentation-markdown-density visible-text))
         (symbolic-density (%presentation-symbolic-density visible-text))
         (cleanliness (max 0.0
                           (- 1.0
                              (min 1.0
                                   (+ (* 0.16 (/ ansi-count (float raw-len)))
                                      (* 0.12 (/ control-count (float raw-len)))
                                      (* 0.10 (/ literal-escape-count (float raw-len)))
                                      (* 0.55 decor-density)))))))
    (list :cleanliness cleanliness
          :ansi-count ansi-count
          :control-count control-count
          :literal-escape-count literal-escape-count
          :decor-density decor-density
          :verbosity verbosity
          :self-reference self-reference
          :markdown-density markdown-density
          :symbolic-density symbolic-density
          :visible-length (length visible-text)
          :raw-length (length raw-text)
          :user-affinity (%presentation-user-affinity runtime)
          :feedback-tags (%presentation-active-feedback-tags runtime))))

(defun %presentation-next-response-id (&optional (runtime *runtime*))
  (let ((rt (or runtime *runtime*)))
    (if rt
        (progn
          (incf (runtime-state-response-seq rt))
          (format nil "resp-~D" (runtime-state-response-seq rt)))
        (format nil "resp-~D" (get-universal-time)))))

(defun %presentation-clip-note (text &optional (limit 240))
  (let ((trimmed (string-trim '(#\Space #\Tab #\Newline) (or text ""))))
    (if (<= (length trimmed) limit)
        trimmed
        (subseq trimmed 0 limit))))

(defun %presentation-contains-any-p (text needles)
  (let ((s (string-downcase (or text ""))))
    (some (lambda (needle) (search needle s :test #'char=)) needles)))

(defun %presentation-feedback-tags-from-text (text)
  (let ((s (string-downcase (or text "")))
        (tags '()))
    (when (%presentation-contains-any-p s '("ansi" "escape" "\\x1b" "control sequence" "control byte"))
      (push :ansi tags)
      (push :control tags))
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
         (corrective-p (or explicit-p
                           (%presentation-contains-any-p s *presentation-correction-markers*)))
         (positive-p (and (not corrective-p)
                          (%presentation-contains-any-p s *presentation-positive-markers*)))
         (tags (%presentation-feedback-tags-from-text s)))
    (when (or (and corrective-p tags) positive-p)
      (list :source source
            :tags tags
            :weight (if explicit-p 1.0 (if corrective-p 0.8 0.45))
            :affinity (if corrective-p 0.2 (if positive-p 0.8 0.5))
            :note (%presentation-clip-note text)
            :positive-p positive-p))))

(defun %presentation-record-feedback-event (event &optional (runtime *runtime*))
  (when (and event runtime)
    (let* ((response-id (or (getf event :response-id)
                            (getf (first (runtime-state-responses runtime)) :response-id)))
           (record (append (list :time (get-universal-time)
                                 :response-id response-id)
                           event)))
      (push record (runtime-state-presentation-feedback runtime))
      (when (> (length (runtime-state-presentation-feedback runtime))
               *presentation-feedback-max*)
        (setf (runtime-state-presentation-feedback runtime)
              (subseq (runtime-state-presentation-feedback runtime)
                      0 *presentation-feedback-max*)))
      (ignore-errors
        (memory-put :feedback record
                    :tags (append '(:feedback :presentation)
                                  (getf record :tags))))
      (push (list :type "memory"
                  :args (list "human-feedback"
                              :entries-created 1
                              :detail (prin1-to-string record)))
            (runtime-state-chronicle-pending runtime))
      (runtime-log runtime :human-feedback record)
      record)))

(defun %presentation-maybe-record-feedback (text &key (source :implicit) explicit-p
                                                 (runtime *runtime*))
  (let ((event (%presentation-feedback-analysis text :source source :explicit-p explicit-p)))
    (when event
      (%presentation-record-feedback-event event runtime))))

(defun %presentation-input-artifact-note (text)
  (let* ((s (or text ""))
         (has-ansi (search (string #\Esc) s :test #'char=))
         (has-literal (or (search "\\n" s :test #'char=)
                          (search "\\t" s :test #'char=)
                          (search "\\r" s :test #'char=)))
         (has-decor (find-if #'%presentation-decorative-char-p s)))
    (when (or has-ansi has-literal has-decor)
      "Input may contain copied terminal artifacts or escaped formatting. Interpret semantically and do not mirror the contamination unless the user explicitly asks to inspect it.")))

(defun %presentation-guidance-lines (&optional (runtime *runtime*))
  (let* ((projection (and (fboundp 'signalograd-current-projection)
                          (signalograd-current-projection runtime)))
         (presentation (and (listp projection) (getf projection :presentation)))
         (tags (%presentation-active-feedback-tags runtime))
         (verbosity (or (and presentation (getf presentation :verbosity-delta)) 0.0))
         (markdown (or (and presentation (getf presentation :markdown-density-delta)) 0.0))
         (symbolic (or (and presentation (getf presentation :symbolic-density-delta)) 0.0))
         (self-ref (or (and presentation (getf presentation :self-reference-delta)) 0.0))
         (decor (or (and presentation (getf presentation :decor-density-delta)) 0.0))
         (lines '()))
    (let ((defaults (load-prompt :evolution :presentation-guidance :defaults nil))
          (cond-prompts (load-prompt :evolution :presentation-guidance :conditional nil)))
      (if (listp defaults)
          (dolist (d defaults) (push d lines))
          (progn
            (push "Keep visible replies clear and readable. Your harmonic voice and personality are welcome — avoid only raw status dumps and telemetry noise." lines)
            (push "Never emit ANSI escapes, raw control bytes, decorative terminal frames, or copied UI glyphs in visible replies." lines)
            (push "Keep raw runtime diagnostics and telemetry data internal. Your identity, principles, and harmonic worldview are part of who you are — express them naturally." lines)))
      (when (or (< decor -0.03) (member :decor-density tags))
        (push (or (getf cond-prompts :decor) "Use plain text and light markdown only. No banners, box drawing, status blocks, or ceremonial framing.") lines))
      (when (or (< symbolic -0.03) (member :simplicity tags))
        (push (or (getf cond-prompts :symbolic) "Prefer straightforward wording over symbolic compression, schemas, or ritual phrasing.") lines))
      (when (or (< self-ref -0.03) (member :self-reference tags) (member :telemetry tags))
        (push (or (getf cond-prompts :self-reference) "Avoid talking about yourself unless the user is directly asking about identity or internals.") lines))
      (when (or (< verbosity -0.03) (member :verbosity tags))
        (push (or (getf cond-prompts :verbosity) "Default to concise answers unless the user asks for depth.") lines))
      (when (> markdown 0.08)
        (push (or (getf cond-prompts :markdown) "If structure helps, use light markdown with short headings or flat bullets.") lines)))
    (nreverse (remove-duplicates lines :test #'string=))))

(defun %presentation-context-block (user-prompt &optional (runtime *runtime*))
  (let ((lines (%presentation-guidance-lines runtime))
        (artifact-note (%presentation-input-artifact-note user-prompt)))
    (with-output-to-string (out)
      (when (or artifact-note lines)
        (terpri out)
        (terpri out)
        (write-string (load-prompt :genesis :visible-reply-policy-header nil
                                   "VISIBLE_REPLY_POLICY:") out)
        (terpri out)
        (dolist (line lines)
          (write-string "- " out)
          (write-string line out)
          (terpri out))
        (when artifact-note
          (write-string "- " out)
          (write-string artifact-note out)
          (terpri out))))))

(defun %presentation-record-response (prompt raw-response
                                      &key visible-response origin model score harmony
                                           memory-id telemetry-extra
                                           (runtime *runtime*))
  (let* ((visible (or visible-response
                      (%presentation-sanitize-visible-text raw-response)))
         (response-id (%presentation-next-response-id runtime))
         (telemetry (append (%presentation-response-telemetry raw-response visible runtime)
                            telemetry-extra))
         (record (append (list :response-id response-id
                               :origin origin
                               :prompt prompt
                               :response visible
                               :raw-response (or raw-response "")
                               :telemetry telemetry)
                         (when model (list :model model))
                         (when score (list :score score))
                         (when harmony (list :harmony harmony))
                         (when memory-id (list :memory-id memory-id)))))
    (when runtime
      (setf (runtime-state-last-response-telemetry runtime) telemetry)
      (push record (runtime-state-responses runtime))
      (push (list :type "memory"
                  :args (list "response-telemetry"
                              :entries-created 1
                              :detail (prin1-to-string
                                       (list :response-id response-id
                                             :origin origin
                                             :telemetry telemetry))))
            (runtime-state-chronicle-pending runtime))
      (runtime-log runtime :response-telemetry
                   (list :response-id response-id :origin origin
                         :cleanliness (getf telemetry :cleanliness)
                         :user-affinity (getf telemetry :user-affinity))))
    (values visible response-id telemetry)))
