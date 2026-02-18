;;; store.lisp — Layered memory substrate (idle-night compression only).

(in-package :harmonia)

(defstruct memory-entry
  id
  time
  class        ;; :soul | :skill | :daily | :tool
  depth        ;; 0 = raw, 1+ = compressed layers
  content
  tags
  source-ids
  access-count
  last-access)

(defparameter *memory-store* (make-hash-table :test 'equal)) ;; id -> memory-entry
(defparameter *memory-by-class* (make-hash-table :test 'eq)) ;; class -> (list of ids)
(defparameter *memory-seq* 0)
(defparameter *memory-last-active-at* (get-universal-time))
(defparameter *memory-last-compression-at* 0)
(defparameter *memory-compressed-source-ids* (make-hash-table :test 'equal))
(defparameter *memory-concept-nodes* (make-hash-table :test 'equal)) ;; concept -> plist
(defparameter *memory-concept-edges* (make-hash-table :test 'equal)) ;; key -> plist
(defparameter *memory-stopwords*
  '("a" "an" "the" "and" "or" "if" "then" "else" "for" "of" "to" "in" "on" "at"
    "with" "from" "is" "are" "was" "were" "be" "been" "it" "this" "that" "as" "by"
    "about" "can" "could" "should" "would" "do" "does" "did" "you" "your" "my"))
(defparameter *noise-score-threshold* 0.02)

(defun %getenv (name &optional default)
  (or (sb-ext:posix-getenv name) default))

(defun %parse-int-env (name default)
  (let ((raw (%getenv name nil)))
    (if raw
        (handler-case (parse-integer raw)
          (error () default))
        default)))

(defun %night-config ()
  (list :start (%parse-int-env "HARMONIA_MEMORY_NIGHT_START" 1)
        :end (%parse-int-env "HARMONIA_MEMORY_NIGHT_END" 5)
        :idle-seconds (%parse-int-env "HARMONIA_MEMORY_IDLE_SECONDS" 900)
        :heartbeat-seconds (%parse-int-env "HARMONIA_MEMORY_HEARTBEAT_SECONDS" 300)))

(defun %user-timezone-west ()
  (let ((override (%getenv "HARMONIA_USER_TZ_HOURS_WEST" nil)))
    (if override
        (handler-case (read-from-string override)
          (error () nil))
        nil)))

(defun %local-hour (ut)
  (multiple-value-bind (_sec _min hour _day _month _year _dow _dst _tz)
      (if (%user-timezone-west)
          (decode-universal-time ut (%user-timezone-west))
          (decode-universal-time ut))
    (declare (ignore _sec _min _day _month _year _dow _dst _tz))
    hour))

(defun %within-night-window-p (hour start end)
  (if (< start end)
      (and (>= hour start) (< hour end))
      (or (>= hour start) (< hour end))))

(defun memory-touch-activity ()
  (setf *memory-last-active-at* (get-universal-time))
  t)

(defun %push-class-id (class id)
  (push id (gethash class *memory-by-class*)))

(defun %entry-text (entry)
  (let ((content (memory-entry-content entry)))
    (if (stringp content) content (prin1-to-string content))))

(defun %normalize-text (text)
  (let ((s (string-downcase (if text text ""))))
    (with-output-to-string (out)
      (loop for ch across s do
        (cond
          ((alphanumericp ch) (write-char ch out))
          ((char= ch #\Space) (write-char #\Space out))
          (t (write-char #\Space out)))))))

(defun %split-words (text)
  (let ((norm (%normalize-text text))
        (words '())
        (start 0))
    (loop for i from 0 to (length norm) do
      (when (or (= i (length norm))
                (char= (char norm i) #\Space))
        (let ((w (string-trim " " (subseq norm start i))))
          (when (> (length w) 2)
            (push w words)))
        (setf start (1+ i))))
    (remove-duplicates
     (remove-if (lambda (w) (member w *memory-stopwords* :test #'string=))
                (nreverse words))
     :test #'string=)))

(defun %concept-domain (word)
  (cond
    ((member word '("music" "harmony" "melody" "rhythm" "tone") :test #'string=) :music)
    ((member word '("math" "ratio" "geometry" "fractal" "theory" "proof") :test #'string=) :math)
    ((member word '("code" "lisp" "rust" "tool" "backend" "api" "model") :test #'string=) :engineering)
    ((member word '("memory" "brain" "sleep" "dream" "dna" "evolve") :test #'string=) :cognitive)
    ((member word '("weather" "travel" "calendar" "meeting" "time") :test #'string=) :life)
    (t :generic)))

(defun %edge-key (a b)
  (if (string< a b)
      (format nil "~A|~A" a b)
      (format nil "~A|~A" b a)))

(defun %upsert-concept-node (concept class depth entry-id)
  (let* ((existing (gethash concept *memory-concept-nodes*))
         (domain (%concept-domain concept))
         (count (if existing (getf existing :count) 0))
         (entries (if existing (getf existing :entries) '()))
         (classes (if existing (getf existing :classes) '()))
         (depths (if existing (getf existing :depths) '())))
    (setf (gethash concept *memory-concept-nodes*)
          (list :concept concept
                :domain domain
                :count (1+ count)
                :entries (adjoin entry-id entries :test #'string=)
                :classes (adjoin class classes :test #'eq)
                :depths (adjoin depth depths :test #'=)))))

(defun %upsert-concept-edge (a b reason)
  (unless (string= a b)
    (let* ((k (%edge-key a b))
           (existing (gethash k *memory-concept-edges*))
           (weight (if existing (getf existing :weight) 0))
           (reasons (if existing (getf existing :reasons) '()))
           (da (%concept-domain a))
           (db (%concept-domain b)))
      (setf (gethash k *memory-concept-edges*)
            (list :a a
                  :b b
                  :weight (1+ weight)
                  :reasons (adjoin reason reasons :test #'eq)
                  :interdisciplinary (not (eq da db)))))))

(defun %index-entry-concepts (entry-id class depth content &key (reason :cooccur))
  (let* ((text (if (stringp content) content (prin1-to-string content)))
         (concepts (%split-words text)))
    (dolist (c concepts)
      (%upsert-concept-node c class depth entry-id))
    (loop for left in concepts do
      (loop for right in concepts do
        (when (string< left right)
          (%upsert-concept-edge left right reason))))
    concepts))

(defun %intent-key-from-daily (entry)
  (let* ((payload (memory-entry-content entry))
         (prompt (if (and (listp payload) (getf payload :prompt))
                     (getf payload :prompt)
                     (%entry-text entry)))
         (norm (%normalize-text prompt))
         (trimmed (string-trim " " norm)))
    (subseq trimmed 0 (min 64 (length trimmed)))))

(defun %daily-uncompressed-entries ()
  (let ((rows '()))
    (dolist (id (gethash :daily *memory-by-class*))
      (let ((entry (gethash id *memory-store*)))
        (when (and entry
                   (zerop (memory-entry-depth entry))
                   (not (gethash id *memory-compressed-source-ids*)))
          (push entry rows))))
    rows))

(defun %safe-average (nums)
  (if nums
      (/ (reduce #'+ nums) (float (length nums)))
      0.0))

(defun %compression-harmony-metrics (intent group)
  (declare (ignore intent))
  (let* ((old-size (max 1 (reduce #'+ group :key (lambda (e) (length (%entry-text e))))))
         (new-repr (prin1-to-string (mapcar #'memory-entry-content group)))
         (new-size (length new-repr))
         (ratio (/ new-size (float old-size)))
         (solomonoff (exp (- (/ new-size 40.0))))
         (occam-pass (<= ratio 1.1))
         (laws (getf *dna* :laws)))
    (list :kolmogorov-ratio ratio
          :old-size old-size
          :new-size new-size
          :solomonoff-prior solomonoff
          :occam-pass occam-pass
          :dna-laws laws
          :dna-prime-directive (getf *dna* :prime-directive))))

(defun %daily-signal-score (entry)
  (let* ((payload (memory-entry-content entry))
         (score (if (and (listp payload) (numberp (getf payload :score)))
                    (getf payload :score)
                    0.0))
         (response (if (and (listp payload) (stringp (getf payload :response)))
                       (length (getf payload :response))
                       0))
         (brevity (/ (min 200 response) 200.0)))
    (+ (* 0.8 score) (* 0.2 brevity))))

(defun %build-skill-summary (intent group)
  (let ((scores '())
        (sample-response "")
        (count 0))
    (dolist (entry group)
      (incf count)
      (let ((payload (memory-entry-content entry)))
        (when (and (listp payload) (numberp (getf payload :score)))
          (push (getf payload :score) scores))
        (when (and (string= sample-response "")
                   (listp payload)
                   (stringp (getf payload :response)))
          (setf sample-response (getf payload :response)))))
    (list :intent intent
          :examples count
          :avg-score (%safe-average scores)
          :sample-response (subseq sample-response 0 (min 180 (length sample-response)))
          :harmony (%compression-harmony-metrics intent group))))

(defun memory-put (class content &key (depth 0) (tags '()) (source-ids '()))
  (incf *memory-seq*)
  (let* ((now (get-universal-time))
         (id (format nil "~A-~A-~A" class now *memory-seq*))
         (entry (make-memory-entry :id id
                                   :time now
                                   :class class
                                   :depth depth
                                   :content content
                                   :tags tags
                                   :source-ids source-ids
                                   :access-count 0
                                   :last-access nil)))
    (setf (gethash id *memory-store*) entry)
    (%push-class-id class id)
    (%index-entry-concepts id class depth content)
    (when (eq class :skill)
      (%upsert-concept-edge "skill" "memory" :skill-memory))
    id))

(defun memory-seed-soul-from-dna ()
  (when (null (gethash :soul *memory-by-class*))
    (memory-put :soul (dna-soul-sexp) :depth 0 :tags (list :dna :immutable :soul))))

(defun memory-recent (&key (limit 5) class (max-depth nil))
  (let ((values '()))
    (maphash
     (lambda (_ entry)
       (declare (ignore _))
       (when (and (or (null class) (eq class (memory-entry-class entry)))
                  (or (null max-depth) (<= (memory-entry-depth entry) max-depth)))
         (push entry values)))
             *memory-store*)
    (subseq (sort values #'> :key #'memory-entry-time)
            0
            (min limit (length values)))))

(defun memory-record-tool-usage (tool-name &key latency-ms success)
  (memory-put :tool
              (list :tool tool-name
                    :latency-ms latency-ms
                    :success success)
              :depth 0
              :tags (list :tool-metric)))

(defun memory-record-orchestration (prompt response tool score latency-ms)
  (let ((daily-id
          (memory-put :daily
                      (list :prompt prompt
                            :response response
                            :score score
                            :tool tool
                            :latency-ms latency-ms
                            :channel :human)
                      :depth 0
                      :tags (list :interaction :orchestration))))
    ;; Explicit tool-memory relation edge marker for map semantics.
    (%upsert-concept-edge (string-downcase tool) "memory" :tool-memory)
    daily-id))

(defun memory-layered-recall (query &key (limit 10) (dive nil))
  "Default behavior returns compressed layers first (:skill depth 1+).
If DIVE is true, raw :daily depth-0 memories are appended."
  (let* ((needle (string-downcase (if query query "")))
         (candidate-classes (if dive '(:skill :daily) '(:skill)))
         (all '()))
    (dolist (class candidate-classes)
      (dolist (id (gethash class *memory-by-class*))
        (let ((entry (gethash id *memory-store*)))
          (when (and entry
                     (or (not (eq class :daily)) dive)
                     (search needle (%entry-text entry) :test #'char-equal))
            (incf (memory-entry-access-count entry))
            (setf (memory-entry-last-access entry) (get-universal-time))
            (push entry all)))))
    (subseq (sort all #'>
                  :key (lambda (entry)
                         (+ (* 10 (memory-entry-depth entry))
                            (memory-entry-access-count entry))))
            0
            (min limit (length all)))))

(defun memory-map-sexp (&key (entry-limit 80) (edge-limit 120))
  "Returns a layered S-expression memory map with concept interrelations."
  (let* ((recent (memory-recent :limit entry-limit))
         (nodes '())
         (edges '()))
    (maphash (lambda (_ v) (declare (ignore _)) (push v nodes))
             *memory-concept-nodes*)
    (maphash (lambda (_ v) (declare (ignore _)) (push v edges))
             *memory-concept-edges*)
    (list :schema :layered-memory-map-v1
          :dna (list :creator (getf *dna* :creator)
                     :prime-directive (getf *dna* :prime-directive)
                     :laws (getf *dna* :laws))
          :layers (list
                   (list :name :skill :count (length (gethash :skill *memory-by-class*)) :depth 1)
                   (list :name :daily :count (length (gethash :daily *memory-by-class*)) :depth 0)
                   (list :name :tool :count (length (gethash :tool *memory-by-class*)) :depth 0)
                   (list :name :soul :count (length (gethash :soul *memory-by-class*)) :depth 0))
          :lineage (mapcar (lambda (entry)
                             (list :id (memory-entry-id entry)
                                   :class (memory-entry-class entry)
                                   :depth (memory-entry-depth entry)
                                   :source-ids (memory-entry-source-ids entry)
                                   :tags (memory-entry-tags entry)))
                           recent)
          :concept-nodes (subseq (sort nodes #'> :key (lambda (n) (getf n :count)))
                                 0 (min edge-limit (length nodes)))
          :concept-edges (subseq (sort edges #'> :key (lambda (e) (getf e :weight)))
                                 0 (min edge-limit (length edges))))))

(defun memory-compress-idle-night (&key (runtime *runtime*))
  "Non-destructive compression:
1) raw :daily layer is preserved
2) new :skill compressed layer is added
3) only runs during idle night heartbeat window."
  (let* ((now (get-universal-time))
         (cfg (%night-config))
         (start (getf cfg :start))
         (end (getf cfg :end))
         (idle-seconds (getf cfg :idle-seconds))
         (heartbeat-seconds (getf cfg :heartbeat-seconds))
         (idle-for (- now *memory-last-active-at*))
         (hour (%local-hour now)))
    (cond
      ((< idle-for idle-seconds)
       (when runtime
         (runtime-log runtime :memory-compress-skipped
                      (list :reason :active :idle-for idle-for)))
       0)
      ((not (%within-night-window-p hour start end))
       (when runtime
         (runtime-log runtime :memory-compress-skipped
                      (list :reason :not-night :hour hour :night-start start :night-end end)))
       0)
      ((< (- now *memory-last-compression-at*) heartbeat-seconds)
       0)
      (t
       (let ((groups (make-hash-table :test 'equal))
             (created 0))
         (dolist (entry (%daily-uncompressed-entries))
           (when (>= (%daily-signal-score entry) *noise-score-threshold*)
             (let ((key (%intent-key-from-daily entry)))
               (push entry (gethash key groups)))))
        (maphash
          (lambda (intent group)
            (when (>= (length group) 2)
              (let ((source-ids (mapcar #'memory-entry-id group)))
                (let ((skill-id
                        (memory-put :skill
                                    (%build-skill-summary intent group)
                                    :depth 1
                                    :tags (list :compressed :nightly :solomonoff :occam)
                                    :source-ids source-ids)))
                  (dolist (src group)
                    (let ((src-concepts (%split-words (%entry-text src)))
                          (skill-concepts (%split-words intent)))
                      (dolist (a src-concepts)
                        (dolist (b skill-concepts)
                         (%upsert-concept-edge a b :compressed-link))))
                    (%index-entry-concepts skill-id :skill 1 (memory-entry-content src)
                                           :reason :compression-source)))
                (%upsert-concept-edge "skill" "memory" :skill-memory)
                (dolist (id source-ids)
                  (setf (gethash id *memory-compressed-source-ids*) t))
                (incf created))))
          groups)
         (setf *memory-last-compression-at* now)
         (when runtime
           (runtime-log runtime :memory-compressed
                        (list :created created :hour hour :idle-for idle-for)))
         created)))))

(defun memory-heartbeat (&key (runtime *runtime*))
  (memory-compress-idle-night :runtime runtime))

(defun memory-reset ()
  (setf *memory-store* (make-hash-table :test 'equal))
  (setf *memory-by-class* (make-hash-table :test 'eq))
  (setf *memory-seq* 0)
  (setf *memory-last-active-at* (get-universal-time))
  (setf *memory-last-compression-at* 0)
  (setf *memory-compressed-source-ids* (make-hash-table :test 'equal))
  (setf *memory-concept-nodes* (make-hash-table :test 'equal))
  (setf *memory-concept-edges* (make-hash-table :test 'equal))
  (memory-seed-soul-from-dna)
  t)
