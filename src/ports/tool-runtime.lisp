;;; tool-runtime.lisp — Port: search/voice tool loading and dispatch via CFFI.

(in-package :harmonia)

(defparameter *tool-libs* (make-hash-table :test 'equal))

(cffi:defcfun ("harmonia_search_exa_query" %exa-query) :pointer (query :string))
(cffi:defcfun ("harmonia_search_exa_last_error" %exa-last-error) :pointer)
(cffi:defcfun ("harmonia_search_exa_free_string" %exa-free-string) :void (ptr :pointer))

(cffi:defcfun ("harmonia_search_brave_query" %brave-query) :pointer (query :string))
(cffi:defcfun ("harmonia_search_brave_last_error" %brave-last-error) :pointer)
(cffi:defcfun ("harmonia_search_brave_free_string" %brave-free-string) :void (ptr :pointer))

(cffi:defcfun ("harmonia_whisper_transcribe" %whisper-transcribe) :pointer (audio-path :string))
(cffi:defcfun ("harmonia_whisper_last_error" %whisper-last-error) :pointer)
(cffi:defcfun ("harmonia_whisper_free_string" %whisper-free-string) :void (ptr :pointer))

(cffi:defcfun ("harmonia_elevenlabs_tts_to_file" %eleven-tts) :int (text :string) (voice-id :string) (out-path :string))
(cffi:defcfun ("harmonia_elevenlabs_last_error" %eleven-last-error) :pointer)
(cffi:defcfun ("harmonia_elevenlabs_free_string" %eleven-free-string) :void (ptr :pointer))

(defun %load-tool (id file)
  (setf (gethash id *tool-libs*)
        (cffi:load-foreign-library (%release-lib-path file))))

(defun init-tool-runtime-port ()
  (ensure-cffi)
  (%load-tool "search-exa" "libharmonia_search_exa.dylib")
  (%load-tool "search-brave" "libharmonia_search_brave.dylib")
  (%load-tool "whisper" "libharmonia_whisper.dylib")
  (%load-tool "elevenlabs" "libharmonia_elevenlabs.dylib")
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
  (format nil
          "You are the truth-seeking search subagent. Use live web and X search when useful. Prioritize factual accuracy over style.~%~%Query: ~A~%~%Return concise markdown with these headings only: Summary, Evidence, Uncertainty. Include source links or domains when available."
          query))

(defun search-grok-live (query)
  (backend-complete (%grok-live-search-prompt query) "x-ai/grok-4.1-fast"))

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

(defun whisper-transcribe (audio-path)
  (let ((ptr (%whisper-transcribe audio-path)))
    (or (%ptr-string ptr #'%whisper-free-string)
        (error "whisper transcribe failed: ~A"
               (%last-error-string #'%whisper-last-error #'%whisper-free-string)))))

(defun elevenlabs-tts-to-file (text voice-id out-path)
  (let ((rc (%eleven-tts text voice-id out-path)))
    (unless (zerop rc)
      (error "elevenlabs tts failed: ~A"
             (%last-error-string #'%eleven-last-error #'%eleven-free-string)))
    out-path))
