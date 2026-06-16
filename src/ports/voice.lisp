;;; voice.lisp — Port: speech-to-text + text-to-speech via the voice actor (IPC).
;;;
;;; STT/TTS for calls route through ONE config-driven actor (whisper / elevenlabs / a custom
;;; OpenAI-compatible endpoint). The Lisp side owns routing POLICY (voice-routing.lisp +
;;; voice-policy.sexp); the Rust actor owns execution. NO FFI — pure s-expr IPC, exactly like
;;; every other port.
;;;
;;; HOMOICONIC: IPC commands are Lisp LISTS serialized with %sexp-to-ipc-string.

(in-package :harmonia)

(defparameter *voice-ready* nil)

(defun voice-port-ready-p () *voice-ready*)

(defun init-voice-port ()
  (let ((reply (ipc-call (%sexp-to-ipc-string '(:component "voice" :op "ready")))))
    (setf *voice-ready* (and reply (ipc-reply-ok-p reply)))
    *voice-ready*))

(defun voice-transcribe (audio-path &key model)
  "Transcribe AUDIO-PATH via the routed STT endpoint (tier-selected unless :model given).
Returns the transcript string, or nil."
  (when (and (voice-port-ready-p) (stringp audio-path) (plusp (length audio-path)))
    (let* ((m (or model (voice-select-endpoint :stt)))
           (parsed (%parse-port-reply
                    (ipc-call (%sexp-to-ipc-string
                               `(:component "voice" :op "transcribe"
                                 :audio ,audio-path :model ,m))))))
      (getf parsed :text))))

(defun voice-synthesize (text &key voice out model)
  "Synthesize TEXT to an audio file via the routed TTS endpoint. Returns the output path, or nil.
VOICE/OUT default to the configured TTS voice + output path; MODEL is tier-selected unless given."
  (when (and (voice-port-ready-p) (stringp text) (plusp (length text)))
    (let* ((v (or voice (and (fboundp '%default-tts-voice) (%default-tts-voice)) ""))
           (o (or out (and (fboundp '%default-tts-output) (%default-tts-output))
                  (concatenate 'string (%state-root) "/tts.mp3")))
           (m (or model (voice-select-endpoint :tts)))
           (parsed (%parse-port-reply
                    (ipc-call (%sexp-to-ipc-string
                               `(:component "voice" :op "synthesize"
                                 :text ,text :voice ,v :out ,o :model ,m))))))
      (getf parsed :path))))

(defun voice-offerings ()
  "The STT + TTS offerings the voice actor exposes (for introspection / routing)."
  (%parse-port-reply (ipc-call (%sexp-to-ipc-string '(:component "voice" :op "offerings")))))

(defun voice-providers ()
  "Active/inactive voice providers (whisper-groq, whisper-openai, elevenlabs, custom-*)."
  (getf (%parse-port-reply
         (ipc-call (%sexp-to-ipc-string '(:component "voice" :op "providers"))))
        :providers))
