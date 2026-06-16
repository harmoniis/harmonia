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
  "Tier pool membership — the voice analogue of %tier-model-pool."
  (let ((etier (or (getf endpoint :tier) :eco)))
    (case tier
      (:free    (eq etier :free))
      (:eco     (member etier '(:free :eco) :test #'eq))
      (:premium (member etier '(:eco :premium :pro :frontier) :test #'eq))
      (:auto    t)
      (t        t))))

(defun voice-select-endpoint (kind &optional (tier *routing-tier*))
  "Choose the STT/TTS endpoint id for KIND (:stt|:tts) at TIER. For calls latency rules:
within the eligible tier pool prefer the fastest (:speed); :premium prefers quality. Falls
back to a sane low-latency default per kind when the policy is absent or empty."
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
