;;; voice-runtime.lisp — Port: voice provider routing via CFFI (whisper, elevenlabs).
;;;
;;; Routes through the voice-router backend which dispatches to the
;;; appropriate voice provider (Groq whisper, OpenAI whisper, ElevenLabs).

(in-package :harmonia)

(cffi:defcfun ("harmonia_voice_router_transcribe" %voice-transcribe) :pointer
  (audio-path :string) (model-hint :string))
(cffi:defcfun ("harmonia_voice_router_tts" %voice-tts) :int
  (text :string) (voice-id :string) (out-path :string) (model-hint :string))
(cffi:defcfun ("harmonia_voice_router_list_providers" %voice-list-providers) :pointer)
(cffi:defcfun ("harmonia_voice_router_last_error" %voice-last-error) :pointer)
(cffi:defcfun ("harmonia_voice_router_free_string" %voice-free-string) :void (ptr :pointer))

(defun init-voice-runtime-port ()
  (ensure-cffi)
  (%load-tool "voice-router" "libharmonia_voice_router.dylib")
  t)

(defun whisper-transcribe (audio-path &optional (model ""))
  "Transcribe audio using voice-router (Groq primary, OpenAI fallback)."
  (harmonic-matrix-route-or-error "orchestrator" "voice-router")
  (let ((ptr (%voice-transcribe audio-path model)))
    (or (%ptr-string ptr #'%voice-free-string)
        (error "whisper transcribe failed: ~A"
               (%last-error-string #'%voice-last-error #'%voice-free-string)))))

(defun elevenlabs-tts-to-file (text voice-id out-path &optional (model ""))
  "Synthesize speech using voice-router (ElevenLabs)."
  (harmonic-matrix-route-or-error "orchestrator" "voice-router")
  (let ((rc (%voice-tts text voice-id out-path model)))
    (unless (zerop rc)
      (error "elevenlabs tts failed: ~A"
             (%last-error-string #'%voice-last-error #'%voice-free-string)))
    out-path))

(defun voice-list-providers ()
  "List active voice providers as s-expression."
  (let ((ptr (%voice-list-providers)))
    (or (%ptr-string ptr #'%voice-free-string)
        "nil")))
