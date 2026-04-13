;;; state.lisp — Memory entry struct, store state, and persistence.

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
(defparameter *memory-concept-directed-counts* (make-hash-table :test 'equal)
  "Directed co-occurrence counts. Key: 'A>B' (ordered as A appeared before B).
   Value: integer count. Tracks temporal ordering for A-B topological flux.")

;;; ─── Thread safety ──────────────────────────────────────────────────
;;; All mutations to memory state go through this mutex.
;;; Hash-table operations in SBCL are NOT atomic — concurrent mutations
;;; can corrupt the table and lose entries silently.
(defvar *memory-lock* (sb-thread:make-mutex :name "memory-store"))

(defmacro with-memory-lock (() &body body)
  "Execute BODY with the memory store mutex held."
  `(sb-thread:with-mutex (*memory-lock*) ,@body))
(defparameter *memory-stopwords*
  '("a" "an" "the" "and" "or" "if" "then" "else" "for" "of" "to" "in" "on" "at"
    "with" "from" "is" "are" "was" "were" "be" "been" "it" "this" "that" "as" "by"
    "about" "can" "could" "should" "would" "do" "does" "did" "you" "your" "my"
    "who" "what" "how" "when" "where" "why" "which" "whom" "whose"
    "tell" "explain" "describe" "show" "give" "let" "know" "think"
    "me" "his" "her" "its" "our" "their" "them" "they" "she" "he" "we"
    "not" "but" "have" "has" "had" "will" "shall" "may" "might"
    "just" "very" "also" "some" "any" "all" "each" "every" "more" "most"
    "here" "there" "now" "then" "got" "get" "got" "like" "one" "two"))
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

(defun memory-reset ()
  "Wipe all in-memory state and re-seed from DNA. Thread-safe."
  (with-memory-lock ()
    (setf *memory-store* (make-hash-table :test 'equal))
    (setf *memory-by-class* (make-hash-table :test 'eq))
    (setf *memory-seq* 0)
    (setf *memory-last-active-at* (get-universal-time))
    (setf *memory-last-compression-at* 0)
    (setf *memory-last-journal-day* 0)
    (setf *memory-compressed-source-ids* (make-hash-table :test 'equal))
    (setf *memory-concept-nodes* (make-hash-table :test 'equal))
    (setf *memory-concept-edges* (make-hash-table :test 'equal))
    (setf *memory-concept-directed-counts* (make-hash-table :test 'equal)))
  ;; Re-seed outside lock (calls memory-put which takes its own lock).
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
  "Persist a memory entry to Chronicle. Dedup via content hash."
  (when (and (fboundp 'ipc-call) (fboundp 'ipc-available-p) (funcall 'ipc-available-p))
    (let* ((content-str (if (stringp content) content (princ-to-string content)))
           (tags-str (format nil "~{~A~^ ~}" (or tags '())))
           (source-str (format nil "~{~A~^ ~}" (or source-ids '()))))
      (funcall 'ipc-call
               (%sexp-to-ipc-string
                `(:component "chronicle" :op "persist-entry"
                  :id ,id :ts ,ts :content ,content-str
                  :tags ,tags-str :source-ids ,source-str))))))

(defun %load-memories-from-chronicle ()
  "Load all persistent memory entries from Chronicle into RAM.
Rebuilds the concept graph from loaded entries.
Called once at boot, before memory-field initialization."
  (when (not (fboundp 'ipc-call))
    (return-from %load-memories-from-chronicle 0))
  (let ((reply (funcall 'ipc-call "(:component \"chronicle\" :op \"load-all-entries\")")))
    (when (and reply (funcall 'ipc-reply-ok-p reply))
      (let* ((*read-eval* nil)
             (parsed (handler-case (read-from-string reply) (error () nil)))
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
                     ;; Infer depth from class — soul is identity (deep), skill is compressed.
                     (depth (cond
                              ((eq class :soul) 2)
                              ((eq class :skill) 1)
                              ((eq class :tool) 1)
                              (t 0)))
                     (entry (make-memory-entry :id id
                                               :time ts
                                               :class class
                                               :depth depth
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
                  (handler-case

                      (%index-entry-concepts id class 0 content)

                    (error () nil)))))))
        (%log :info "memory" "Loaded ~D memories, ~D concept nodes."
              count (hash-table-count *memory-concept-nodes*))
        count))))
