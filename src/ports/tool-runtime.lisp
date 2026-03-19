;;; tool-runtime.lisp — Port: search tool loading and dispatch.
;;;
;;; NOTE: Search tools (exa, brave) are not yet wired as IPC components.
;;; Wrappers return errors until the Rust actors are connected.
;;; search-web falls through to grok-live which uses backend-complete.

(in-package :harmonia)

(defparameter *tool-libs* (make-hash-table :test 'equal))

(defun init-tool-runtime-port ()
  "No-op: search tools will be initialized when IPC components are wired."
  (%log :info "tool-runtime" "Tool runtime port initialized (IPC stub — not yet wired)")
  t)

(defun tool-runtime-list ()
  "Return list of loaded tool names."
  (let ((names '()))
    (maphash (lambda (k v) (declare (ignore v)) (push k names)) *tool-libs*)
    (nreverse names)))

(defun search-exa (query)
  (declare (ignorable query))
  (%log :warn "tool-runtime" "search-exa called on unwired IPC stub")
  (error "exa query failed: search-exa not yet wired as IPC component"))

(defun search-brave (query)
  (declare (ignorable query))
  (%log :warn "tool-runtime" "search-brave called on unwired IPC stub")
  (error "brave query failed: search-brave not yet wired as IPC component"))

(defun %grok-live-search-prompt (query)
  (let ((template (load-prompt :evolution :grok-live-search nil
                   "You are the truth-seeking search subagent. Use live web and X search when useful. Prioritize factual accuracy over style.

Query: ~A

Return concise markdown with these headings only: Summary, Evidence, Uncertainty. Include source links or domains when available.")))
    (format nil template query)))

(defun %preferred-truth-seeking-model ()
  "Return the first model with :truth-seeking feature, or fallback."
  (or (and (fboundp '%truth-seeking-models)
           (car (funcall '%truth-seeking-models)))
      "x-ai/grok-4.1-fast"))

(defun search-grok-live (query)
  (backend-complete (%grok-live-search-prompt query)
                    (%preferred-truth-seeking-model)))

(defun search-web (query)
  (harmonic-matrix-route-or-error "orchestrator" "search-exa")
  (handler-case
      (let ((res (search-exa query)))
        (harmonic-matrix-observe-route "orchestrator" "search-exa" t 1)
        (harmonic-matrix-observe-route "search-exa" "memory" t 1)
        res)
    (error (_)
      (declare (ignore _))
      (harmonic-matrix-observe-route "orchestrator" "search-exa" nil 1)
      (harmonic-matrix-route-or-error "orchestrator" "search-brave")
      (handler-case
          (let ((res (search-brave query)))
            (harmonic-matrix-observe-route "orchestrator" "search-brave" t 1)
            (harmonic-matrix-observe-route "search-brave" "memory" t 1)
            res)
        (error (__)
          (declare (ignore __))
          (harmonic-matrix-observe-route "orchestrator" "search-brave" nil 1)
          (harmonic-matrix-route-or-error "orchestrator" "provider-router")
          (let ((res (search-grok-live query)))
            (harmonic-matrix-observe-route "orchestrator" "provider-router" t 1)
            (harmonic-matrix-observe-route "provider-router" "memory" t 1)
            res))))))

;;; whisper-transcribe and elevenlabs-tts-to-file are now defined in
;;; voice-runtime.lisp, which routes through the voice-router backend.
