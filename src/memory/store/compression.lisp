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
