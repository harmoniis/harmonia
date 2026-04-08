;;; compression.lisp — Idle-night non-destructive memory compression.

(in-package :harmonia)

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

(defun %crystal-score (entry)
  "Score a daily entry for crystallization (0.0-1.0)."
  (let* ((payload (memory-entry-content entry))
         (text (%entry-text entry))
         (norm (string-downcase text))
         ;; 0.3 weight: harmonic score from payload
         (harmonic-score (if (and (listp payload) (numberp (getf payload :score)))
                             (getf payload :score) 0.0))
         ;; 0.25 weight: fact density (digits + special chars ratio)
         (fact-chars (count-if (lambda (ch) (or (digit-char-p ch) (find ch ".:/@#$%"))) text))
         (fact-density (if (> (length text) 0) (/ fact-chars (float (length text))) 0.0))
         ;; 0.3 weight: decision word hits
         (decision-words '("prefer" "always" "never" "decided" "remember" "important"
                           "must" "should" "require"))
         (decision-hits (count-if (lambda (w) (search w norm :test #'char-equal)) decision-words))
         (decision-score (min 1.0 (/ decision-hits 3.0)))
         ;; 0.15 weight: access count boost
         (access-boost (min 1.0 (/ (memory-entry-access-count entry) 5.0))))
    (+ (* 0.3 (min 1.0 harmonic-score))
       (* 0.25 (min 1.0 fact-density))
       (* 0.3 decision-score)
       (* 0.15 access-boost))))

(defun %crystallize-before-compression ()
  "Tag uncompressed dailies scoring >= threshold with :crystal. Return count."
  (let ((threshold (if (fboundp 'signalograd-effective-harmony-number)
                       (signalograd-effective-harmony-number "memory/crystal-min-score" 0.7 *runtime*)
                       0.7))
        (count 0))
    (dolist (entry (%daily-uncompressed-entries))
      (when (>= (%crystal-score entry) threshold)
        (unless (member :crystal (memory-entry-tags entry))
          (push :crystal (memory-entry-tags entry))
          (incf count))))
    count))

(defun %find-compression-candidates ()
  "Group uncompressed daily entries by intent key. Returns a hash-table of intent -> entries."
  (let ((groups (make-hash-table :test 'equal)))
    (dolist (entry (%daily-uncompressed-entries))
      (when (and (>= (%daily-signal-score entry) *noise-score-threshold*)
                 (not (member :crystal (memory-entry-tags entry))))
        (push entry (gethash (%intent-key-from-daily entry) groups))))
    groups))

(defun %cross-link-concepts (group skill-id intent)
  "Create concept edges between source entries and the compressed skill entry."
  (let ((skill-concepts (%split-words intent)))
    (dolist (src group)
      (let ((src-concepts (%split-words (%entry-text src))))
        (mapcan (lambda (a)
                  (mapcar (lambda (b) (%upsert-concept-edge a b :compressed-link))
                          skill-concepts))
                src-concepts))
      (%index-entry-concepts skill-id :skill 1 (memory-entry-content src)
                             :reason :compression-source))))

(defun %merge-candidate-group (intent group)
  "Merge one candidate group into a compressed :skill entry. Returns 1 if created, 0 otherwise."
  (if (< (length group) 2)
      0
      (let ((source-ids (mapcar #'memory-entry-id group)))
        (let ((skill-id (memory-put :skill
                                    (%build-skill-summary intent group)
                                    :depth 1
                                    :tags (list :compressed :nightly :solomonoff :occam)
                                    :source-ids source-ids)))
          (%cross-link-concepts group skill-id intent))
        (%upsert-concept-edge "skill" "memory" :skill-memory)
        (dolist (id source-ids)
          (setf (gethash id *memory-compressed-source-ids*) t))
        1)))

(defun %crystallize-structural-entries (runtime)
  "Run crystallization pass and log results. Returns count of crystallized entries."
  (let ((n (%crystallize-before-compression)))
    (when (> n 0)
      (handler-case

          (chronicle-record-memory-event "crystallise"
          :entries-created n
          :node-count (hash-table-count *memory-concept-nodes*)
          :edge-count (hash-table-count *memory-concept-edges*)

        (error () nil)))
      (when runtime
        (runtime-log runtime :memory-crystallized (list :count n))))
    n))

(defun %compression-window-p (now)
  "Check whether conditions allow compression. Returns (values ok-p idle-for hour cfg) or nil."
  (let* ((cfg (%night-config))
         (idle-for (- now *memory-last-active-at*))
         (hour (%local-hour now)))
    (values (and (>= idle-for (getf cfg :idle-seconds))
                 (%within-night-window-p hour (getf cfg :start) (getf cfg :end))
                 (>= (- now *memory-last-compression-at*) (getf cfg :heartbeat-seconds)))
            idle-for hour cfg)))

(defun memory-compress-idle-night (&key (runtime *runtime*))
  "Non-destructive compression:
1) raw :daily layer is preserved
2) new :skill compressed layer is added
3) only runs during idle night heartbeat window."
  (let ((now (get-universal-time)))
    (multiple-value-bind (window-ok idle-for hour cfg) (%compression-window-p now)
      (declare (ignore cfg))
      (cond
        ((not window-ok)
         (when runtime
           (runtime-log runtime :memory-compress-skipped
                        (list :reason :window-check-failed :idle-for idle-for :hour hour)))
         0)
        (t
         (%crystallize-structural-entries runtime)
         (let* ((groups (%find-compression-candidates))
                (created 0))
           (maphash (lambda (intent group)
                      (incf created (%merge-candidate-group intent group)))
                    groups)
           (setf *memory-last-compression-at* now)
           (when runtime
             (runtime-log runtime :memory-compressed
                          (list :created created :hour hour :idle-for idle-for)))
           (when (> created 0)
             (handler-case
                 (chronicle-record-memory-event "compress"
                   :entries-created created
                   :node-count (hash-table-count *memory-concept-nodes*)
                   :edge-count (hash-table-count *memory-concept-edges*)
                   :detail (format nil "hour=~D idle=~Ds" hour idle-for))
               (error (e) (%log :warn "memory" "chronicle compress record failed: ~A" e))))
           created))))))

(defun memory-heartbeat (&key (runtime *runtime*))
  (memory-compress-idle-night :runtime runtime))
