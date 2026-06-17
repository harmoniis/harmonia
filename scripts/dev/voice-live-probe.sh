#!/usr/bin/env bash
# voice-live-probe.sh — ON-DEMAND live round-trip + latency against the BER1-AI voice cluster
# (faster-whisper STT + Chatterbox TTS, fronted by the swapd router). This probe is NOT part of
# the hermetic no-regression suite: it makes real network calls and depends on a live endpoint + key.
#
# It exercises the FULL agent path end to end:
#   Lisp port (voice-synthesize / voice-transcribe)  ->  voice actor (s-expr IPC, NO FFI)
#     ->  custom OpenAI-compatible endpoint (/v1/audio/speech, /v1/audio/transcriptions).
# It configures the dev node's custom STT/TTS (config-store + vault), verifies the providers flip
# active (proving the Lisp-set -> Rust-get_own path), round-trips a known phrase (TTS -> WAV -> STT),
# and measures cold-vs-warm latency with a deterministic loop (latency is REPORTED, not pass/fail).
#
# Endpoint: router.harmoniis.com (as instructed).  Canonical/production endpoint is
# ai.telcovillage.com (per ber1-ai/docs/architecture.md) — override with HARMONIA_VOICE_LIVE_BASE.
# Key: required via HARMONIA_VOICE_LIVE_KEY (no secret is committed in this script).
set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HARNESS="$REPO/scripts/dev/harness.sh"
DEV="${HARMONIA_DEV_ROOT:-$HOME/.harmoniis/harmonia-dev}"
SOCK="${TMPDIR:-/tmp}/harmonia/harmonia.sock"

KEY="${HARMONIA_VOICE_LIVE_KEY:-}"
BASE="${HARMONIA_VOICE_LIVE_BASE:-https://router.harmoniis.com}"
if [ -z "$KEY" ]; then
  echo "❌ HARMONIA_VOICE_LIVE_KEY not set (BER1-AI Bearer key). Aborting (no live call)."; exit 2
fi
export VL_STT_URL="$BASE/v1/audio/transcriptions"
export VL_TTS_URL="$BASE/v1/audio/speech"
export VL_KEY="$KEY"
echo "── live voice probe  →  $BASE ──"

[ -S "$SOCK" ] || "$HARNESS" bringup >/dev/null 2>&1

ASSERT="$(mktemp -t voice-live-XXXX).lisp"
cat > "$ASSERT" <<'LISP'
(in-package :harmonia)
(defparameter *vp* 0) (defparameter *vf* 0)
(defun va (l ok) (if ok (progn (incf *vp*) (format t "  ✅ ~A~%" l))
                     (progn (incf *vf*) (format t "  ❌ ~A~%" l))))
(defun ms-since (t0)
  (round (* 1000 (/ (- (get-internal-real-time) t0) internal-time-units-per-second))))
(defun norm (s) (string-downcase (remove-if-not #'alphanumericp (or s ""))))
(defun genv (k) (or (sb-ext:posix-getenv k) ""))
(defun prov-active-p (provs id)
  (let ((e (find id provs :key (lambda (p) (and (listp p) (getf p :id))) :test #'equal)))
    (and e (getf e :active))))

(handler-case
 (let ((stt-url (genv "VL_STT_URL")) (tts-url (genv "VL_TTS_URL")) (key (genv "VL_KEY")))
  (format t "~%── A. configure custom STT/TTS on the dev node ──~%")
  (config-set-for "voice" "custom-stt-url" stt-url)
  (config-set-for "voice" "custom-tts-url" tts-url)
  (config-set-for "voice" "custom-stt-model" "whisper")
  (config-set-for "voice" "custom-tts-model" "chatterbox")
  (vault-set-secret "custom-stt-api-key" key)
  (vault-set-secret "custom-tts-api-key" key)
  (va "config persisted + readable (custom-stt-url)"
      (string= (config-get-for "voice" "custom-stt-url") stt-url))
  (let ((provs (voice-providers)))
    (format t "     providers: ~S~%" provs)
    ;; proves the Lisp config-set -> Rust get_own("voice", "custom-*-url") read path + vault policy
    (va "voice-providers: custom-stt :active t" (prov-active-p provs "custom-stt"))
    (va "voice-providers: custom-tts :active t" (prov-active-p provs "custom-tts")))

  (format t "~%── B. round-trip: Chatterbox TTS -> WAV -> Whisper STT ──~%")
  (let* ((phrase "Harmonia voice round trip, the quick brown fox.")
         (wav "/tmp/harmonia-live-rt.wav")
         (t0 (get-internal-real-time))
         (out (voice-synthesize phrase :out wav :model "custom/tts"))
         (synth-ms (ms-since t0)))
    (va "TTS produced an audio file (>1KB)"
        (and out (probe-file wav)
             (> (with-open-file (s wav :element-type '(unsigned-byte 8)) (file-length s)) 1000)))
    (format t "     synth latency: ~A ms~%" synth-ms)
    (let* ((t1 (get-internal-real-time))
           (txt (voice-transcribe wav :model "custom/stt"))
           (tr-ms (ms-since t1)))
      (format t "     transcript: ~S  (~A ms)~%" txt tr-ms)
      (va "STT transcript non-empty" (and txt (plusp (length txt))))
      ;; Substance recovery, not ASR perfection: >=2 of 3 distinctive words proves TTS+STT round-trip
      ;; (single-word ASR slips like "quick"->"click" are expected and must not fail the integration).
      (va "round-trip fidelity (>=2 of quick/brown/fox recovered)"
          (let ((n (norm txt)))
            (>= (count-if (lambda (w) (search w n)) '("quick" "brown" "fox")) 2)))))

  (format t "~%── C. latency: cold vs warm (5 iterations; REPORTED) ──~%")
  (let ((synth '()) (trans '()))
    (dotimes (i 5)
      (let* ((wav (format nil "/tmp/harmonia-live-lat-~A.wav" i))
             (t0 (get-internal-real-time))
             (o (voice-synthesize "low latency check one two three four" :out wav :model "custom/tts"))
             (ms (ms-since t0)))
        (declare (ignore o))
        (push ms synth)
        (let* ((t1 (get-internal-real-time))
               (tx (voice-transcribe wav :model "custom/stt"))
               (ms2 (ms-since t1)))
          (declare (ignore tx))
          (push ms2 trans))))
    (let* ((sy (reverse synth)) (tr (reverse trans))
           (sy-warm (reduce #'min (rest sy))) (tr-warm (reduce #'min (rest tr))))
      (format t "     TTS ms/iter: ~A   cold=~A  warm-min=~A~%" sy (first sy) sy-warm)
      (format t "     STT ms/iter: ~A   cold=~A  warm-min=~A~%" tr (first tr) tr-warm)
      ;; correctness sanity only — latency itself is infra-dependent and reported, not asserted hard
      (va "warm STT latency within a sane bound (< 15s)" (< tr-warm 15000))
      (va "warm TTS latency within a sane bound (< 20s)" (< sy-warm 20000))))

  (format t "~%── D. :call tier routes to the self-hosted cluster (SIP path) ──~%")
  ;; with custom configured, the :call tier (not a forced :model) must select the self-hosted endpoint
  (va "voice-select-endpoint :stt :call -> custom/stt"
      (string= (voice-select-endpoint :stt :call) "custom/stt"))
  (va "voice-select-endpoint :tts :call -> custom/tts"
      (string= (voice-select-endpoint :tts :call) "custom/tts"))
  (let* ((wav "/tmp/harmonia-live-call.wav")
         (out (voice-synthesize "Routing a call through the self hosted cluster." :out wav :tier :call))
         (txt (and out (voice-transcribe wav :tier :call))))
    (va "tier-routed (:call) round-trip recovers speech"
        (let ((n (norm txt))) (and (search "call" n) (search "cluster" n)))))

  (format t "~%════════ VOICE-LIVE: ~A pass / ~A fail ════════~%" *vp* *vf*))
 (error (e) (format t "~%VOICE-LIVE ERROR: ~A~%" e) (incf *vf*)))
(sb-ext:exit :code (if (zerop *vf*) 0 1))
LISP

env HARMONIA_STATE_ROOT="$DEV" HARMONIA_SYSTEM_DIR="$DEV" HARMONIA_VAULT_DB="$DEV/vault.db" \
    HARMONIA_SOURCE_DIR="$REPO" HARMONIA_ENV=dev HARMONIA_LOG_LEVEL=error \
    VL_STT_URL="$VL_STT_URL" VL_TTS_URL="$VL_TTS_URL" VL_KEY="$VL_KEY" \
  sbcl --noinform --disable-debugger --load "$REPO/src/core/boot.lisp" \
       --eval '(harmonia:start :run-loop nil)' --load "$ASSERT"
RC=$?
rm -f "$ASSERT"
exit $RC
