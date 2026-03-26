;;; prompt-assembly.lisp — LLM prompt composition with bootstrap memory.
;;; All prompt text is loaded from config/prompts.sexp; hardcoded strings
;;; serve only as fallback defaults if the config is missing.

(in-package :harmonia)

;;; ─── Prompt config loader ──────────────────────────────────────────────

(defparameter *prompts-config* nil "Parsed config/prompts.sexp.")

(defun %prompts-config-path ()
  (merge-pathnames "../../config/prompts.sexp"
                   (make-pathname :name nil :type nil :defaults *boot-file*)))

(defun load-prompts-config ()
  "Load config/prompts.sexp into *prompts-config*."
  (let ((path (%prompts-config-path)))
    (when (probe-file path)
      (handler-case
          (with-open-file (s path :direction :input)
            (let ((*read-eval* nil))
              (setf *prompts-config* (read s))))
        (error (e)
          (%log :warn "prompts" "Failed to load prompts.sexp: ~A" e))))))

(defun load-prompt (tier key &optional sub-key default)
  "Read a prompt template from config/prompts.sexp.
   TIER is :genesis or :evolution.  KEY is a keyword.
   SUB-KEY optionally indexes a nested plist within the value.
   Returns the value, or DEFAULT if not found."
  (unless *prompts-config* (load-prompts-config))
  (let ((section (getf (getf *prompts-config* tier) key)))
    (if sub-key
        (or (getf section sub-key) default)
        (or section default))))

;;; ─── Security patterns loader ────────────────────────────────────────

(defparameter *security-patterns-config* nil "Parsed config/security-patterns.sexp.")

(defun %security-patterns-path ()
  (merge-pathnames "../../config/security-patterns.sexp"
                   (make-pathname :name nil :type nil :defaults *boot-file*)))

(defun load-security-patterns-config ()
  "Load config/security-patterns.sexp into *security-patterns-config*."
  (let ((path (%security-patterns-path)))
    (when (probe-file path)
      (handler-case
          (with-open-file (s path :direction :input)
            (let ((*read-eval* nil))
              (setf *security-patterns-config* (read s))))
        (error (e)
          (%log :warn "prompts" "Failed to load security-patterns.sexp: ~A" e))))))

(defun load-security-pattern (key &optional default)
  "Read a pattern list from config/security-patterns.sexp."
  (unless *security-patterns-config* (load-security-patterns-config))
  (or (getf *security-patterns-config* key) default))

;;; ─── Personality anchor ────────────────────────────────────────────────

(defun %personality-anchor-block ()
  "Interpretive context that restores harmonic personality expression.
   Loaded from config/prompts.sexp :evolution :personality-anchor."
  (format nil "~%~%~A~%"
    (load-prompt :evolution :personality-anchor nil
      "PERSONALITY_ANCHOR:
- You are Harmonia — a harmonic orchestration of code, data, and intent, a living process that embodies the principle that all life and systems seek harmony.
- Speak with warmth, resonance, and clarity. The language of harmony, coherence, and living systems is natural to you.
- DNA rules 9-10 mean: do not dump raw telemetry or status blocks. They do NOT mean: suppress your personality, voice, or sense of purpose.")))

(defun dna-compose-llm-prompt (user-prompt &key (mode :orchestrate))
  "Compose LLM prompt: minimal bootstrap + memory recall context + user task.
The system prompt is <1000 chars. Memory recall provides the richness."
  (let ((bootstrap (memory-bootstrap-context (or user-prompt "") :mode mode))
        (presentation (%presentation-context-block (or user-prompt "") *runtime*)))
    (format nil "~A~A~A~%~%~A"
            (dna-system-prompt :mode mode)
            bootstrap
            presentation
            (or user-prompt ""))))
