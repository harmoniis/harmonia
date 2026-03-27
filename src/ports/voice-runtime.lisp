;;; voice-runtime.lisp — Port: voice provider routing (whisper, elevenlabs).
;;;
;;; NOTE: voice-router is not yet wired as an IPC component.
;;; Wrappers return errors until the Rust actor is connected.

(in-package :harmonia)

(defun init-voice-runtime-port ()
  "Stub: voice-router IPC component not yet wired."
  t)

(defun whisper-transcribe (audio-path &optional (model ""))
  "Transcribe audio using voice-router (Groq primary, OpenAI fallback)."
  (declare (ignorable audio-path model))
  (%log :warn "voice-runtime" "whisper-transcribe called on unwired IPC stub")
  (error "whisper transcribe failed: voice-router not yet wired as IPC component"))

(defun elevenlabs-tts-to-file (text voice-id out-path &optional (model ""))
  "Synthesize speech using voice-router (ElevenLabs)."
  (declare (ignorable text voice-id out-path model))
  (%log :warn "voice-runtime" "elevenlabs-tts-to-file called on unwired IPC stub")
  (error "elevenlabs tts failed: voice-router not yet wired as IPC component"))

(defun voice-list-providers ()
  "List active voice providers as s-expression."
  (%log :warn "voice-runtime" "voice-list-providers called on unwired IPC stub")
  "nil")
