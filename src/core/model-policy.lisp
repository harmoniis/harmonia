;;; model-policy.lisp — Harmonic model selection policy.

(in-package :harmonia)

(defparameter *model-policy-config-path*
  (merge-pathnames "../../config/model-policy.sexp" *boot-file*))
(defparameter *model-policy-state-path*
  (or (sb-ext:posix-getenv "HARMONIA_MODEL_POLICY_PATH")
      (let ((root (or (sb-ext:posix-getenv "HARMONIA_STATE_ROOT")
                      (let ((base (or (sb-ext:posix-getenv "TMPDIR")
                                      (namestring (user-homedir-pathname)))))
                        (concatenate 'string (string-right-trim "/" base) "/harmonia")))))
        (concatenate 'string root "/model-policy.sexp"))))

(defparameter *model-profiles* '())
(defparameter *model-harmony-weights* '())

(declaim (ftype function backend-complete model-policy-get))

(defun %model-policy-read-file (path)
  (with-open-file (in path :direction :input)
    (let ((*read-eval* nil))
      (read in nil nil))))

(defun %model-policy-load-source ()
  (cond
    ((probe-file *model-policy-state-path*)
     (%model-policy-read-file *model-policy-state-path*))
    ((probe-file *model-policy-config-path*)
     (%model-policy-read-file *model-policy-config-path*))
    (t
     (error "model policy config missing: ~A" *model-policy-config-path*))))

(defun model-policy-load ()
  (let ((src (%model-policy-load-source)))
    (setf *model-profiles* (copy-tree (getf src :profiles)))
    (setf *model-harmony-weights* (copy-tree (getf src :weights)))
    (model-policy-get)))

(defun %profiles-by-cost ()
  (sort (copy-list *model-profiles*) #'< :key (lambda (p) (getf p :cost 9999))))

(defun %planner-profile-id ()
  (or (getf (find-if (lambda (p) (member :planner (getf p :tags))) *model-profiles*) :id)
      (getf (first *model-profiles*) :id)))

(defun model-policy-save ()
  (let ((path *model-policy-state-path*))
    (ensure-directories-exist path)
    (with-open-file (out path :direction :output :if-exists :supersede :if-does-not-exist :create)
      (let ((*print-pretty* t))
        (prin1 (model-policy-get) out)
        (terpri out)))
    path))

(defun %task-weights (task)
  (case task
    (:tooling '(:completion 0.30 :correctness 0.20 :speed 0.30 :price 0.20))
    (:vision '(:completion 0.42 :correctness 0.28 :speed 0.16 :price 0.14))
    (:critical-reasoning '(:completion 0.55 :correctness 0.25 :speed 0.10 :price 0.10))
    (:planning '(:completion 0.50 :correctness 0.24 :speed 0.14 :price 0.12))
    (:coding '(:completion 0.48 :correctness 0.26 :speed 0.14 :price 0.12))
    (t *model-harmony-weights*)))

(defun %model-weight (weights k)
  (or (getf weights k) 0.0))

(defun %task-kind (prompt)
  (let ((p (string-downcase prompt)))
    (cond
      ((or (search "ocr" p) (search "image" p) (search "vision" p)) :vision)
      ((or (search "rewrite" p) (search "evolution" p) (search "architecture" p)) :critical-reasoning)
      ((or (search "plan" p) (search "orchestrate" p) (search "decision" p)) :planning)
      ((or (search "tool op=" p) (search "send" p) (search "search " p)) :tooling)
      ((or (search "code" p) (search "compile" p) (search "bug" p)) :coding)
      (t :general))))

(defun %task-need (task)
  (case task
    (:vision '(:vision :ocr))
    (:critical-reasoning '(:thinking :frontier :reasoning))
    (:planning '(:planner :thinking :reasoning))
    (:tooling '(:fast :cheap))
    (:coding '(:coding :reasoning))
    (t '(:balanced :reasoning))))

(defun %model-score (profile task)
  (let* ((weights (%task-weights task))
         (tags (getf profile :tags))
         (need (%task-need task))
         (completion (getf profile :completion 1))
         (quality (getf profile :quality 1))
         (latency (getf profile :latency 10))
         (cost (getf profile :cost 10))
         (fit (loop for n in need count (member n tags)))
         (completion-s (* (%model-weight weights :completion) completion))
         (quality-s (* (%model-weight weights :correctness) quality))
         (speed-s (* (%model-weight weights :speed) (/ 10.0 (max 1 latency))))
         (price-s (* (%model-weight weights :price) (/ 10.0 (max 1 cost)))))
    (+ completion-s quality-s speed-s price-s (* 0.5 fit))))

(defun %profile-by-id (id)
  (find id *model-profiles* :key (lambda (p) (getf p :id)) :test #'string=))

(defun %model-candidates (task &key (limit 5))
  (let ((scored
          (mapcar (lambda (p) (cons (getf p :id) (%model-score p task)))
                  *model-profiles*)))
    (mapcar #'car
            (subseq (sort scored #'> :key #'cdr) 0 (min limit (length scored))))))

(defun %pick-heuristic-model (prompt)
  (let* ((task (%task-kind prompt))
         (cands (%model-candidates task :limit 1)))
    (or (first cands)
        (getf (first *model-profiles*) :id))))

(defun %planner-enabled-p ()
  (string= (or (sb-ext:posix-getenv "HARMONIA_MODEL_PLANNER") "1")
           "1"))

(defun %planner-model ()
  (or (sb-ext:posix-getenv "HARMONIA_MODEL_PLANNER_MODEL")
      (%planner-profile-id)))

(defun model-policy-get ()
  (list :profiles *model-profiles* :weights *model-harmony-weights*))

(defun model-policy-set-weight (metric value)
  (setf (getf *model-harmony-weights* metric) (coerce value 'float))
  (ignore-errors (model-policy-save))
  *model-harmony-weights*)

(defun model-policy-upsert-profile (id &key tier cost latency quality completion tags)
  (let* ((existing (%profile-by-id id))
         (profile (list :id id
                        :tier (or tier (and existing (getf existing :tier)) :custom)
                        :cost (or cost (and existing (getf existing :cost)) 5)
                        :latency (or latency (and existing (getf existing :latency)) 5)
                        :quality (or quality (and existing (getf existing :quality)) 5)
                        :completion (or completion (and existing (getf existing :completion)) 5)
                        :tags (or tags (and existing (getf existing :tags)) '(:custom)))))
    (setf *model-profiles*
          (cons profile
                (remove id *model-profiles* :key (lambda (p) (getf p :id)) :test #'string=)))
    (ignore-errors (model-policy-save))
    profile))

(defun choose-model (prompt)
  (let* ((task (%task-kind prompt))
         (cands (%model-candidates task :limit 6))
         (fallback (or (first cands)
                       (getf (first *model-profiles*) :id))))
    (if (not (%planner-enabled-p))
        fallback
        (handler-case
            (let* ((decision-prompt
                     (format nil
                             "Pick exactly one model id from this list for task kind ~A. Reply ONLY with model id.~%Candidates: ~{~A~^, ~}~%Prompt: ~A"
                             task cands prompt))
                   (picked (string-trim '(#\Space #\Newline #\Tab)
                                        (backend-complete decision-prompt (%planner-model)))))
              (if (%profile-by-id picked) picked fallback))
          (error (_)
            (declare (ignore _))
            fallback)))))

(defun model-escalation-chain (prompt chosen)
  (let* ((task (%task-kind prompt))
         (ordered (%model-candidates task :limit 6))
         (safe-models
           (mapcar (lambda (p) (getf p :id))
                   (subseq (%profiles-by-cost) 0 (min 3 (length *model-profiles*))))))
    (let ((chain (append ordered safe-models)))
      (setf chain (remove-duplicates chain :test #'string=))
      (if (member chosen chain :test #'string=)
          (let ((tail (member chosen chain :test #'string=)))
            (if tail tail chain))
          chain))))
