;;; voice-routing.lisp — STT/TTS endpoint selection policy.
;;;
;;; The voice analogue of model-routing.lisp: a tier sets the eligible pool, and within the
;;; pool latency rules for calls — prefer the fastest endpoint (:premium prefers quality).
;;; Pure functional, config-driven (config/voice-policy.sexp). The Rust voice actor executes
;;; whichever endpoint id this returns.

(in-package :harmonia)

(defparameter *voice-policy-config-path*
  (merge-pathnames "../../config/voice-policy.sexp" *boot-file*))
(defparameter *voice-policy* nil
  "Loaded voice-policy.sexp plist: (:speech-to-text (...) :text-to-speech (...)).")

(defun voice-policy-load ()
  "Load config/voice-policy.sexp into *voice-policy*. Safe: no eval, no crash."
  (handler-case
      (when (probe-file *voice-policy-config-path*)
        (with-open-file (s *voice-policy-config-path* :direction :input)
          (let ((*read-eval* nil))
            (setf *voice-policy* (read s nil nil)))))
    (error () nil))
  *voice-policy*)

(defun %voice-endpoints (kind)
  "STT or TTS endpoint plists from the loaded policy."
  (getf (getf *voice-policy* (ecase kind (:stt :speech-to-text) (:tts :text-to-speech)))
        :endpoints))

(defun %voice-tier-eligible-p (endpoint tier)
  "Tier pool membership for the EXTERNAL fallback ladder — the voice analogue of %tier-model-pool."
  (let ((etier (or (getf endpoint :tier) :eco)))
    (case tier
      (:free    (eq etier :free))
      (:eco     (member etier '(:free :eco) :test #'eq))
      (:premium (member etier '(:eco :premium :pro :frontier) :test #'eq))
      (t        t))))            ; :auto, :call, anything else → whole pool (ranked below)

(defun %voice-custom-url-key (kind)
  (ecase kind (:stt "custom-stt-url") (:tts "custom-tts-url")))

(defun %voice-custom-id (kind)
  (ecase kind (:stt "custom/stt") (:tts "custom/tts")))

(defun %voice-custom-configured-p (kind)
  "Is the self-hosted custom OpenAI-compatible endpoint for KIND configured (its url is set)?
Reads config voice/custom-*-url; safe — never errors or blocks selection."
  (let ((url (and (fboundp 'config-get-for)
                  (ignore-errors (config-get-for "voice" (%voice-custom-url-key kind))))))
    (and (stringp url) (plusp (length url)))))

(defun %voice-select-external (kind tier)
  "External-provider selection used only when the self-hosted cluster is NOT configured:
:eco/:auto rank by latency (:speed); :premium ranks by :quality. Pure over the policy."
  (let* ((endpoints (%voice-endpoints kind))
         (pool (or (remove-if-not (lambda (e) (%voice-tier-eligible-p e tier)) endpoints)
                   endpoints))
         (rank-key (if (eq tier :premium)
                       (lambda (e) (or (getf e :quality) 5))
                       (lambda (e) (or (getf e :speed) 5))))
         (best (first (stable-sort (copy-list pool) #'> :key rank-key))))
    (or (getf best :id)
        (ecase kind
          (:stt "groq/whisper-large-v3-turbo")
          (:tts "elevenlabs/eleven_turbo_v2_5")))))

(defun voice-select-endpoint (kind &optional (tier *routing-tier*))
  "Endpoint id for KIND (:stt|:tts). The co-located SELF-HOSTED cluster is the DEFAULT for ALL
voice — SIP, WhatsApp, or ANY audio source — whenever it is configured (lowest controlled latency,
no external hop or rate limit). It falls back to an external provider only when the self-hosted
endpoint is not configured. An explicit :model overrides this entirely."
  (if (%voice-custom-configured-p kind)
      (%voice-custom-id kind)
      (%voice-select-external kind tier)))
