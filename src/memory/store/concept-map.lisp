;;; concept-map.lisp — Concept graph and layered memory map.

(in-package :harmonia)

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

(defun %index-entry-concepts (entry-id class depth content &key (reason :cooccur) (tags nil))
  "Extract concepts from content and index into the concept graph.
Tags are also indexed as concepts — this creates semantic bridges.
E.g. tag :identity connects to content words, so 'who are you' finds identity entries."
  (let* ((text (if (stringp content) content (prin1-to-string content)))
         (content-concepts (%split-words text))
         ;; Tags become concepts too — semantic bridge between questions and answers.
         (tag-concepts (when tags
                         (remove-duplicates
                          (remove-if (lambda (w) (< (length w) 3))
                                     (mapcar (lambda (tag)
                                               (string-downcase
                                                (if (keywordp tag) (symbol-name tag)
                                                    (princ-to-string tag))))
                                             tags))
                          :test #'string=)))
         (concepts (remove-duplicates (append content-concepts tag-concepts) :test #'string=)))
    (dolist (c concepts)
      (%upsert-concept-node c class depth entry-id))
    (loop for left in concepts do
      (loop for right in concepts do
        (when (string< left right)
          (%upsert-concept-edge left right reason))))
    concepts))

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
