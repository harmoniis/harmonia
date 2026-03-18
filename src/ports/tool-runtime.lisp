;;; tool-runtime.lisp — Port: search tool loading and dispatch via CFFI.
;;;
;;; NOTE: Whisper and ElevenLabs have moved to voice-runtime.lisp as proper
;;; voice provider backends. Search tools will migrate to tool-channel.lisp
;;; once they implement the ToolVtable contract.

(in-package :harmonia)

(defparameter *tool-libs* (make-hash-table :test 'equal))

(cffi:defcfun ("harmonia_search_exa_query" %exa-query) :pointer (query :string))
(cffi:defcfun ("harmonia_search_exa_last_error" %exa-last-error) :pointer)
(cffi:defcfun ("harmonia_search_exa_free_string" %exa-free-string) :void (ptr :pointer))

(cffi:defcfun ("harmonia_search_brave_query" %brave-query) :pointer (query :string))
(cffi:defcfun ("harmonia_search_brave_last_error" %brave-last-error) :pointer)
(cffi:defcfun ("harmonia_search_brave_free_string" %brave-free-string) :void (ptr :pointer))

(defun %load-tool (id file)
  (setf (gethash id *tool-libs*)
        (cffi:load-foreign-library (%release-lib-path file))))

(defun init-tool-runtime-port ()
  (ensure-cffi)
  (%load-tool "search-exa" "libharmonia_search_exa.dylib")
  (%load-tool "search-brave" "libharmonia_search_brave.dylib")
  t)

(defun tool-runtime-list ()
  "Return list of loaded tool names."
  (let ((names '()))
    (maphash (lambda (k v) (declare (ignore v)) (push k names)) *tool-libs*)
    (nreverse names)))

(defun %ptr-string (ptr free-fn)
  (if (cffi:null-pointer-p ptr)
      nil
      (unwind-protect
           (cffi:foreign-string-to-lisp ptr)
        (funcall free-fn ptr))))

(defun %last-error-string (getter free-fn)
  (let ((ptr (funcall getter)))
    (if (cffi:null-pointer-p ptr)
        ""
        (unwind-protect
             (cffi:foreign-string-to-lisp ptr)
          (funcall free-fn ptr)))))

(defun search-exa (query)
  (let ((ptr (%exa-query query)))
    (or (%ptr-string ptr #'%exa-free-string)
        (error "exa query failed: ~A"
               (%last-error-string #'%exa-last-error #'%exa-free-string)))))

(defun search-brave (query)
  (let ((ptr (%brave-query query)))
    (or (%ptr-string ptr #'%brave-free-string)
        (error "brave query failed: ~A"
               (%last-error-string #'%brave-last-error #'%brave-free-string)))))

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
