#!/usr/bin/env bash
# voice-probe.sh — tests the voice subsystem (STT/TTS actor + config-driven routing) DIRECTLY
# via the port functions (no model in the loop). Phase 1: routing correctness + config + the
# actor/IPC boundary + graceful degradation. (Latency/streaming is the streaming phase.)
set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HARNESS="$REPO/scripts/dev/harness.sh"
DEV="${HARMONIA_DEV_ROOT:-$HOME/.harmoniis/harmonia-dev}"
SOCK="${TMPDIR:-/tmp}/harmonia/harmonia.sock"
[ -S "$SOCK" ] || "$HARNESS" bringup >/dev/null 2>&1

ASSERT="$(mktemp -t voice-assert-XXXX).lisp"
cat > "$ASSERT" <<'LISP'
(in-package :harmonia)
(defparameter *vp* 0) (defparameter *vf* 0)
(defun va (l ok) (if ok (progn (incf *vp*) (format t "  ✅ ~A~%" l))
                     (progn (incf *vf*) (format t "  ❌ ~A~%" l))))
(handler-case
 (progn
  (format t "~%── A. actor + config-driven routing ──~%")
  (va "voice actor reachable via IPC (no FFI)" (voice-port-ready-p))
  (va "voice-policy loaded (stt + tts endpoints)"
      (and (%voice-endpoints :stt) (%voice-endpoints :tts)))
  (va "eco STT routes to the FASTEST (groq turbo)"
      (string= (voice-select-endpoint :stt :eco) "groq/whisper-large-v3-turbo"))
  (va "eco TTS routes to the FASTEST (elevenlabs turbo, low-latency)"
      (string= (voice-select-endpoint :tts :eco) "elevenlabs/eleven_turbo_v2_5"))
  (va "auto STT favours low latency (turbo)"
      (string= (voice-select-endpoint :stt :auto) "groq/whisper-large-v3-turbo"))
  (va "premium STT differs from eco (quality-selected, not speed)"
      (not (string= (voice-select-endpoint :stt :premium)
                    (voice-select-endpoint :stt :eco))))
  (va "premium TTS = expressive multilingual (quality)"
      (string= (voice-select-endpoint :tts :premium) "elevenlabs/eleven_multilingual_v2"))

  (format t "~%── B. introspection ──~%")
  (va "offerings: stt + tts pools present"
      (let ((o (voice-offerings))) (and (getf o :stt) (getf o :tts))))
  (va "providers list includes whisper + elevenlabs + custom"
      (let ((p (format nil "~A" (voice-providers))))
        (and (search "whisper-groq" p) (search "elevenlabs" p) (search "custom-stt" p))))

  (format t "~%── C. graceful degradation (no crash) ──~%")
  (va "transcribe with no key/file -> nil (graceful)"
      (null (voice-transcribe "/nonexistent/harmonia-voice-probe.wav")))
  (va "custom endpoint unconfigured -> graceful nil (clear error path)"
      (null (voice-transcribe "/nonexistent.wav" :model "custom/stt")))
  (va "synthesize with empty text -> nil (guarded)"
      (null (voice-synthesize "")))

  (format t "~%════════ VOICE PROBE: ~A pass / ~A fail ════════~%" *vp* *vf*))
 (error (e) (format t "~%VOICE-PROBE ERROR: ~A~%" e) (incf *vf*)))
(sb-ext:exit :code (if (zerop *vf*) 0 1))
LISP

env HARMONIA_STATE_ROOT="$DEV" HARMONIA_SYSTEM_DIR="$DEV" HARMONIA_VAULT_DB="$DEV/vault.db" \
    HARMONIA_SOURCE_DIR="$REPO" HARMONIA_ENV=dev HARMONIA_LOG_LEVEL=error \
  sbcl --noinform --disable-debugger --load "$REPO/src/core/boot.lisp" \
       --eval '(harmonia:start :run-loop nil)' --load "$ASSERT"
RC=$?
rm -f "$ASSERT"
exit $RC
