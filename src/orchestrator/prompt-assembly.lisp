;;; prompt-assembly.lisp — LLM prompt composition with bootstrap memory.

(in-package :harmonia)

(defun dna-compose-llm-prompt (user-prompt &key (mode :orchestrate))
  "Compose full LLM prompt: DNA constitution + bootstrap context + user task."
  (let ((bootstrap (memory-bootstrap-context (or user-prompt "") :mode mode)))
    (format nil "~A~A~%~%USER_TASK:~%~A"
            (dna-system-prompt :mode mode)
            bootstrap
            (or user-prompt ""))))
