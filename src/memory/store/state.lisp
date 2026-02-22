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
