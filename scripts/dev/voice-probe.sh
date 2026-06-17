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
  ;; Self-hosted cluster is the DEFAULT for ALL voice (SIP, WhatsApp, any audio) when configured.
  (va "STT default = self-hosted when configured, else external"
      (string= (voice-select-endpoint :stt)
               (if (%voice-custom-configured-p :stt) "custom/stt" (%voice-select-external :stt *routing-tier*))))
  (va "TTS default = self-hosted when configured, else external"
      (string= (voice-select-endpoint :tts)
               (if (%voice-custom-configured-p :tts) "custom/tts" (%voice-select-external :tts *routing-tier*))))
  (va "every tier prefers self-hosted when configured (SIP/WhatsApp/any source)"
      (or (not (%voice-custom-configured-p :stt))
          (every (lambda (t*) (string= (voice-select-endpoint :stt t*) "custom/stt"))
                 '(:eco :auto :premium :call :free))))
  ;; External FALLBACK ladder (pure, config-independent): used only when self-hosted is absent.
  (va "external eco STT = fastest (groq turbo)"
      (string= (%voice-select-external :stt :eco) "groq/whisper-large-v3-turbo"))
  (va "external eco TTS = fastest (elevenlabs turbo)"
      (string= (%voice-select-external :tts :eco) "elevenlabs/eleven_turbo_v2_5"))
  (va "external premium STT differs from eco (quality, not speed)"
      (not (string= (%voice-select-external :stt :premium) (%voice-select-external :stt :eco))))
  (va "external premium TTS = expressive multilingual (quality)"
      (string= (%voice-select-external :tts :premium) "elevenlabs/eleven_multilingual_v2"))

  (format t "~%── B. introspection ──~%")
  (va "offerings: stt + tts pools present"
      (let ((o (voice-offerings))) (and (getf o :stt) (getf o :tts))))
  (va "providers list includes whisper + elevenlabs + custom"
      (let ((p (format nil "~A" (voice-providers))))
        (and (search "whisper-groq" p) (search "elevenlabs" p) (search "custom-stt" p))))

  (format t "~%── C. graceful degradation (no crash) ──~%")
  (va "transcribe with no key/file -> nil (graceful)"
      (null (voice-transcribe "/nonexistent/harmonia-voice-probe.wav")))
  (va "custom STT on a missing file -> nil (no crash, config-agnostic)"
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
