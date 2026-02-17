;;; conductor.lisp — Prompt orchestration entry points.

(in-package :harmonia)

(defun %extract-tag-value (prompt tag)
  (let* ((needle (format nil "~A=" tag))
         (start (search needle prompt :test #'char-equal)))
    (when start
      (let* ((from (+ start (length needle)))
             (space (position #\Space prompt :start from)))
        (subseq prompt from (or space (length prompt)))))))

(defun %maybe-handle-self-push-test (prompt)
  (when (search "self-push-test" prompt :test #'char-equal)
    (let* ((repo (%extract-tag-value prompt "repo"))
           (branch (%extract-tag-value prompt "branch")))
      (unless (and repo branch)
        (error "self-push-test requires repo=<path> and branch=<name>"))
      (with-open-file (out (merge-pathnames "SELF_PUSH_TEST_FROM_HARMONIA.txt" repo)
                           :direction :output :if-exists :supersede :if-does-not-exist :create)
        (format out "self-push by harmonia at ~A~%" (get-universal-time)))
      (git-commit-and-push repo branch "self push test from harmonia loop")
      (format nil "SELF_PUSH_OK repo=~A branch=~A" repo branch))))

(defun feed-prompt (prompt)
  (unless *runtime*
    (error "Runtime not initialized. Call HARMONIA:START first."))
  (setf (runtime-state-prompt-queue *runtime*)
        (append (runtime-state-prompt-queue *runtime*) (list prompt)))
  (runtime-log *runtime* :prompt-enqueued (list :prompt prompt))
  prompt)

(defun %select-model (prompt)
  (declare (ignore prompt))
  ;; Dev-first cheap path. Can expand to tiered policy later.
  "qwen/qwen3-coder:free")

(defun orchestrate-once (prompt)
  (let* ((model (%select-model prompt))
         (response (or (%maybe-handle-self-push-test prompt)
                       (backend-complete prompt model)))
         (score (harmonic-score prompt response))
         (memory-id (memory-put :daily (list :prompt prompt :response response :score score))))
    (push (list :prompt prompt
                :response response
                :model model
                :score score
                :memory-id memory-id)
          (runtime-state-responses *runtime*))
    (runtime-log *runtime* :orchestrated (list :model model :score score :memory-id memory-id))
    response))
