;;; state.lisp — Memory state, lifecycle, and recall.

(in-package :harmonia)

(declaim (ftype function %index-entry-concepts %upsert-concept-edge))

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
(defparameter *memory-last-journal-day* 0)

(defun %memory-config-int (key default)
  "Read an integer config value from config-store (memory scope) with fallback."
  (let ((raw (and (fboundp 'config-get-for)
                  (funcall 'config-get-for "memory" key))))
    (if raw
        (handler-case (parse-integer raw)
          (error () default))
        default)))

(defun %night-config ()
  (list :start (%memory-config-int "night-start" 1)
        :end (%memory-config-int "night-end" 5)
        :idle-seconds (%memory-config-int "idle-seconds" 900)
        :heartbeat-seconds (%memory-config-int "heartbeat-seconds" 300)))

(defun %user-timezone-west ()
  (let ((raw (and (fboundp 'config-get-for)
                  (funcall 'config-get-for "memory" "user-tz-hours-west"))))
    (when raw
      (handler-case
          (let ((*read-eval* nil))
            (read-from-string raw))
        (error () nil)))))

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

(defun memory-put (class content &key (depth 0) (tags '()) (source-ids '()))
  (when (and (fboundp '%trace-level-p) (%trace-level-p :verbose))
    (ignore-errors
      (trace-event "memory-write" :tool
                   :metadata (list :class class :depth depth))))
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
    (%index-entry-concepts id class depth content :tags tags)
    (when (eq class :skill)
      (%upsert-concept-edge "skill" "memory" :skill-memory))
    ;; Persist to Chronicle (non-blocking, fire-and-forget).
    (ignore-errors (%persist-entry-to-chronicle id now content tags source-ids))
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

(defun memory-record-orchestration (prompt response tool score latency-ms &key harmony)
  (let ((daily-id
          (memory-put :daily
                      (list :prompt prompt
                            :response response
                            :score score
                            :tool tool
                            :latency-ms latency-ms
                            :harmony harmony
                            :channel :human)
                      :depth 0
                      :tags (list :interaction :orchestration))))
    (%upsert-concept-edge (string-downcase tool) "memory" :tool-memory)
    daily-id))

(defun memory-layered-recall (query &key (limit 10) (dive nil))
  "Recall memories. Uses field activation when available, falls back to substring."
  (if (and (fboundp 'memory-field-port-ready-p)
           (funcall 'memory-field-port-ready-p))
      (%memory-field-layered-recall query :limit limit :dive dive)
      (%memory-substring-layered-recall query :limit limit :dive dive)))

(defun %memory-substring-layered-recall (query &key (limit 10) (dive nil))
  "Substring-based recall (original algorithm). Fallback when field engine unavailable."
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

(defun %memory-field-layered-recall (query &key (limit 10) (dive nil))
  "Field-based recall. Returns memory entries sorted by field activation score.
Falls back to substring recall on any error — the field is an enhancement, not a gate."
  (handler-case
      (let* ((field-result (funcall 'memory-field-recall query :limit (* limit 3)))
             (activations (and (listp field-result) (getf field-result :activations)))
             (candidate-classes (if dive '(:skill :daily) '(:skill)))
             (all '()))
        (if (null activations)
            (%memory-substring-layered-recall query :limit limit :dive dive)
            (progn
              (dolist (act activations)
                (when (listp act)
                  (let ((entries (getf act :entries))
                        (score (or (getf act :score) 0.0)))
                    (dolist (entry-id entries)
                      (when (stringp entry-id)
                        (let ((entry (gethash entry-id *memory-store*)))
                          (when (and entry
                                     (member (memory-entry-class entry) candidate-classes)
                                     (or (not (eq (memory-entry-class entry) :daily)) dive))
                            (incf (memory-entry-access-count entry))
                            (setf (memory-entry-last-access entry) (get-universal-time))
                            (push (cons score entry) all))))))))
              (if (null all)
                  (%memory-substring-layered-recall query :limit limit :dive dive)
                  (let ((unique (remove-duplicates all
                                  :key (lambda (pair) (memory-entry-id (cdr pair)))
                                  :test #'string=)))
                    (mapcar #'cdr
                            (subseq (sort unique #'> :key #'car)
                                    0
                                    (min limit (length unique)))))))))
    (error ()
      ;; Any error in field recall → graceful fallback to substring.
      (%memory-substring-layered-recall query :limit limit :dive dive))))

(defun %current-day-number (&optional ut)
  (floor (or ut (get-universal-time)) 86400))

(defun %yesterday-dailies ()
  "Return daily entries from the previous calendar day."
  (let* ((now (get-universal-time))
         (today-day (%current-day-number now))
         (yesterday-day (1- today-day))
         (results '()))
    (dolist (id (gethash :daily *memory-by-class*))
      (let ((entry (gethash id *memory-store*)))
        (when (and entry
                   (= (%current-day-number (memory-entry-time entry)) yesterday-day))
          (push entry results))))
    (sort results #'> :key #'memory-entry-time)))

(defun memory-bootstrap-skills (query &key (limit 3) (max-chars 1200))
  "Return string block of top-N skills matching QUERY."
  (let* ((needle (string-downcase (or query "")))
         (scored '()))
    (dolist (id (gethash :skill *memory-by-class*))
      (let* ((entry (gethash id *memory-store*))
             (text (%entry-text entry))
             (score (+ (* 10 (memory-entry-depth entry))
                       (memory-entry-access-count entry)
                       (if (search needle (string-downcase text) :test #'char-equal) 20 0))))
        (when entry
          (push (cons score entry) scored))))
    (let* ((sorted (subseq (sort scored #'> :key #'car) 0 (min limit (length scored))))
           (out (make-string-output-stream))
           (total 0))
      (dolist (pair sorted)
        (let* ((entry (cdr pair))
               (text (%entry-text entry))
               (clipped (subseq text 0 (min 300 (length text)))))
          (when (< total max-chars)
            (write-string "- " out)
            (write-string clipped out)
            (terpri out)
            (incf total (+ 2 (length clipped) 1)))))
      (get-output-stream-string out))))

(defun memory-bootstrap-recent (&key (limit 5) (max-chars 800))
  "Return string block of last K daily prompts."
  (let* ((recents (memory-recent :limit limit :class :daily :max-depth 0))
         (out (make-string-output-stream))
         (total 0))
    (dolist (entry recents)
      (let* ((payload (memory-entry-content entry))
             (prompt (if (and (listp payload) (getf payload :prompt))
                         (getf payload :prompt)
                         ""))
             (clipped (subseq prompt 0 (min 120 (length prompt)))))
        (when (and (> (length clipped) 0) (< total max-chars))
          (write-string "- " out)
          (write-string clipped out)
          (terpri out)
          (incf total (+ 2 (length clipped) 1)))))
    (get-output-stream-string out)))

(defun memory-bootstrap-context (query &key (mode :orchestrate))
  "Assemble full bootstrap block. Returns empty string for :planner mode."
  (when (eq mode :planner)
    (return-from memory-bootstrap-context ""))
  (let* ((skill-limit (signalograd-memory-bootstrap-skill-limit *runtime*))
         (skill-chars (truncate (if (fboundp 'harmony-policy-number)
                                    (harmony-policy-number "memory/bootstrap-skill-chars" 1200)
                                    1200)))
         (recent-limit (truncate (if (fboundp 'harmony-policy-number)
                                     (harmony-policy-number "memory/bootstrap-recent-limit" 5)
                                     5)))
         (recent-chars (truncate (if (fboundp 'harmony-policy-number)
                                     (harmony-policy-number "memory/bootstrap-recent-chars" 800)
                                     800)))
         (skills (memory-bootstrap-skills (or query "") :limit skill-limit :max-chars skill-chars))
         (recent (memory-bootstrap-recent :limit recent-limit :max-chars recent-chars))
         (has-skills (> (length skills) 0))
         (has-recent (> (length recent) 0)))
    (if (or has-skills has-recent)
        (with-output-to-string (out)
          (terpri out)
          (terpri out)
          (when has-skills
            (write-string "RELEVANT_SKILLS:" out)
            (terpri out)
            (write-string skills out))
          (when has-recent
            (write-string "RECENT_CONTEXT:" out)
            (terpri out)
            (write-string recent out)))
        "")))

(defun memory-semantic-recall-block (query &key (limit 5) (max-chars 1500))
  "Call memory-layered-recall and format as MEMORY_RECALL: block."
  (let* ((results (memory-layered-recall query :limit limit))
         (out (make-string-output-stream))
         (total 0))
    (when results
      (terpri out)
      (terpri out)
      (write-string "MEMORY_RECALL:" out)
      (terpri out)
      (dolist (entry results)
        (let* ((text (%entry-text entry))
               (depth (memory-entry-depth entry))
               (prefix (format nil "[d~D] " depth))
               (clipped (subseq text 0 (min 300 (length text)))))
          (when (< total max-chars)
            (write-string "- " out)
            (write-string prefix out)
            (write-string clipped out)
            (terpri out)
            (incf total (+ 2 (length prefix) (length clipped) 1))))))
    (get-output-stream-string out)))

(defun memory-maybe-journal-yesterday ()
  "On first interaction of a new day, create a skill summarizing yesterday's dailies."
  (let ((today (%current-day-number)))
    (when (= *memory-last-journal-day* today)
      (return-from memory-maybe-journal-yesterday nil))
    (setf *memory-last-journal-day* today)
    (let ((yesterdays (%yesterday-dailies)))
      (when (and yesterdays (> (length yesterdays) 0))
        (let* ((count (length yesterdays))
               (scores (remove nil
                         (mapcar (lambda (e)
                                   (let ((p (memory-entry-content e)))
                                     (when (and (listp p) (numberp (getf p :score)))
                                       (getf p :score))))
                                 yesterdays)))
               (avg-score (if scores (/ (reduce #'+ scores) (float (length scores))) 0.0))
               (topics (remove-duplicates
                         (mapcan (lambda (e)
                                   (let* ((p (memory-entry-content e))
                                          (prompt (if (and (listp p) (stringp (getf p :prompt)))
                                                      (getf p :prompt) ""))
                                          (words (%split-words prompt)))
                                     (subseq words 0 (min 3 (length words)))))
                                 yesterdays)
                         :test #'string=)))
          (memory-put :skill
                      (format nil "Daily journal: ~D interactions, avg score ~,2F, topics: ~{~A~^, ~}"
                              count avg-score topics)
                      :depth 1
                      :tags (list :journal :daily-summary :temporal)))))))

(defun memory-reset ()
  (setf *memory-store* (make-hash-table :test 'equal))
  (setf *memory-by-class* (make-hash-table :test 'eq))
  (setf *memory-seq* 0)
  (setf *memory-last-active-at* (get-universal-time))
  (setf *memory-last-compression-at* 0)
  (setf *memory-last-journal-day* 0)
  (setf *memory-compressed-source-ids* (make-hash-table :test 'equal))
  (setf *memory-concept-nodes* (make-hash-table :test 'equal))
  (setf *memory-concept-edges* (make-hash-table :test 'equal))
  (memory-seed-soul-from-dna)
  t)

;;; ═══════════════════════════════════════════════════════════════════════
;;; PERSISTENT MEMORY — Chronicle as the durable store
;;; ═══════════════════════════════════════════════════════════════════════

(defun %simple-split (string char)
  "Split STRING by CHAR. No dependencies."
  (let ((result '()) (start 0))
    (loop for i from 0 to (length string) do
      (when (or (= i (length string)) (char= (char string i) char))
        (let ((w (subseq string start i)))
          (when (> (length w) 0) (push w result)))
        (setf start (1+ i))))
    (nreverse result)))

(defun %persist-entry-to-chronicle (id ts content tags source-ids)
  "Persist a memory entry to Chronicle. Dedup via content hash.
Non-blocking — errors are silently ignored."
  (when (fboundp 'ipc-call)
    (let* ((content-str (if (stringp content) content (princ-to-string content)))
           (tags-str (format nil "~{~A~^ ~}" (or tags '())))
           (source-str (format nil "~{~A~^ ~}" (or source-ids '()))))
      (funcall 'ipc-call
               (format nil "(:component \"chronicle\" :op \"persist-entry\" :id \"~A\" :ts ~D :content \"~A\" :tags \"~A\" :source-ids \"~A\")"
                       (sexp-escape-lisp id) ts
                       (sexp-escape-lisp content-str)
                       (sexp-escape-lisp tags-str)
                       (sexp-escape-lisp source-str))))))

(defun %load-memories-from-chronicle ()
  "Load all persistent memory entries from Chronicle into RAM.
Rebuilds the concept graph from loaded entries.
Called once at boot, before memory-field initialization."
  (when (not (fboundp 'ipc-call))
    (return-from %load-memories-from-chronicle 0))
  (let ((reply (funcall 'ipc-call "(:component \"chronicle\" :op \"load-all-entries\")")))
    (when (and reply (funcall 'ipc-reply-ok-p reply))
      (let* ((*read-eval* nil)
             (parsed (ignore-errors (read-from-string reply)))
             ;; Strip :ok marker if present.
             (plist (if (and (listp parsed) (eq (car parsed) :ok))
                        (cdr parsed)
                        parsed))
             (count (or (getf plist :count) 0))
             (entries (getf plist :entries)))
        (when (and entries (listp entries) (> count 0))
          (%log :info "memory" "Loading ~D persistent memories from Chronicle..." count)
          (dolist (entry-plist entries)
            (when (listp entry-plist)
              (let* ((id (getf entry-plist :id))
                     (ts (or (getf entry-plist :ts) (get-universal-time)))
                     (content (getf entry-plist :content))
                     (tags-str (or (getf entry-plist :tags) ""))
                     (source-str (or (getf entry-plist :source-ids) ""))
                     (access (or (getf entry-plist :access-count) 0))
                     ;; Infer class from id prefix or tags.
                     (class (cond
                              ((search "SOUL" (or id "")) :soul)
                              ((search "SKILL" (or id "")) :skill)
                              ((search "TOOL" (or id "")) :tool)
                              (t :daily)))
                     (tags (mapcar (lambda (s) (intern (string-upcase s) :keyword))
                                   (remove-if (lambda (s) (= (length s) 0))
                                              (%simple-split tags-str #\Space))))
                     (entry (make-memory-entry :id id
                                               :time ts
                                               :class class
                                               :depth 0
                                               :content content
                                               :tags tags
                                               :source-ids nil
                                               :access-count access
                                               :last-access nil)))
                (when (and id content)
                  (setf (gethash id *memory-store*) entry)
                  (%push-class-id class id)
                  (incf *memory-seq*)
                  ;; Index concepts into graph.
                  (ignore-errors
                    (%index-entry-concepts id class 0 content)))))))
        (%log :info "memory" "Loaded ~D memories, ~D concept nodes."
              count (hash-table-count *memory-concept-nodes*))
        count))))
