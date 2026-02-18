;;; harmonic-genesis-loop.lisp — local feedback loop for harmonic genesis contracts.

(in-package :cl-user)

(load #P"src/core/boot.lisp")

(defun fail (fmt &rest args)
  (apply #'format *error-output* (concatenate 'string "FAIL: " fmt "~%") args)
  (sb-ext:exit :code 2))

(defun assert-true (pred fmt &rest args)
  (unless pred
    (apply #'fail fmt args)))

(defun find-event (tag events)
  (find tag events :key (lambda (e) (getf e :tag))))

(defun run-live-prompt-with-retry (prompt &key (attempts 3))
  (loop for i from 1 to attempts do
    (handler-case
        (return (harmonia:run-prompt prompt :max-cycles 2))
      (error (e)
        (format t "~&WARN prompt-attempt=~D failed: ~A~%" i e)
        (when (= i attempts)
          ;; Preserve continuity: record failed live interaction as noise event.
          (harmonia::memory-record-orchestration
           prompt
           (format nil "LIVE_PROVIDER_ERROR: ~A" e)
           "openrouter"
           0.0
           0)
          (harmonia::runtime-log harmonia:*runtime* :live-provider-error
                                 (list :prompt prompt :error (princ-to-string e)))
          (return nil))))))

(harmonia:start :run-loop nil)

;; Human-like prompt stream across domains to shape deeper concept graph layers.
(dolist (p '("Explain how music ratios can help memory compression."
             "Explain how music ratios can help memory compression for planning."
             "Relate beauty, utility, and strength for software architecture."))
  (run-live-prompt-with-retry p))

;; Deterministic local anchor for compression tests even under provider turbulence.
(harmonia::memory-record-orchestration
 "anchor harmonic memory compression pattern"
 "anchor-response-1"
 "openrouter"
 0.2
 1)
(harmonia::memory-record-orchestration
 "anchor harmonic memory compression pattern"
 "anchor-response-2"
 "openrouter"
 0.25
 1)

;; Force idle-night maintenance window for deterministic local test feedback.
(setf harmonia::*memory-last-active-at* 0)
(harmonia::memory-heartbeat)

;; Run enough cycles to execute full harmonic state-machine phases.
(harmonia:run-loop :max-cycles 14 :sleep-seconds 0.01)

;; Final deterministic compression pass (post-loop) for stable local assertions.
(harmonia::memory-record-orchestration
 "anchor harmonic memory compression pattern"
 "anchor-response-3"
 "openrouter"
 0.3
 1)
(harmonia::memory-record-orchestration
 "anchor harmonic memory compression pattern"
 "anchor-response-4"
 "openrouter"
 0.35
 1)
(setf harmonia::*memory-last-active-at* 0)
(harmonia::memory-heartbeat)

(let* ((events (harmonia::runtime-state-events harmonia:*runtime*))
       (map (harmonia:memory-map-sexp :entry-limit 300 :edge-limit 400))
       (layers (getf map :layers))
       (edges (getf map :concept-edges))
       (plan-ev (find-event :harmonic-rewrite-plan events))
       (plan (and plan-ev (getf plan-ev :payload)))
       (vitruvian (and plan (getf plan :vitruvian)))
       (soul-count (getf (find :soul layers :key (lambda (l) (getf l :name))) :count))
       (daily-count (getf (find :daily layers :key (lambda (l) (getf l :name))) :count))
       (skill-count (getf (find :skill layers :key (lambda (l) (getf l :name))) :count))
       (tool-memory-edge
         (find-if (lambda (e) (member :tool-memory (getf e :reasons))) edges))
       (skill-memory-edge
         (find-if (lambda (e)
                    (or (member :skill-memory (getf e :reasons))
                        (and (string= "skill" (getf e :a))
                             (string= "memory" (getf e :b)))
                        (and (string= "memory" (getf e :a))
                             (string= "skill" (getf e :b)))))
                  edges)))
  (assert-true (>= soul-count 1) "soul layer missing")
  (assert-true (>= daily-count 5) "daily layer too small: ~D" daily-count)
  (assert-true (>= skill-count 1) "skill layer missing after idle compression")
  (assert-true tool-memory-edge "no tool-memory relation edge in map")
  (assert-true skill-memory-edge "no skill-memory relation edge in map")
  (assert-true plan "no harmonic rewrite plan event found")
  (assert-true vitruvian "vitruvian payload missing from plan")
  (assert-true (<= 0.0 (getf vitruvian :strength) 1.0) "invalid strength score: ~S" (getf vitruvian :strength))
  (assert-true (<= 0.0 (getf vitruvian :utility) 1.0) "invalid utility score: ~S" (getf vitruvian :utility))
  (assert-true (<= 0.0 (getf vitruvian :beauty) 1.0) "invalid beauty score: ~S" (getf vitruvian :beauty))
  (assert-true (<= 0.0 (getf vitruvian :signal) 1.0) "invalid signal score: ~S" (getf vitruvian :signal))
  (assert-true (<= 0.0 (getf plan :lambdoma-ratio) 1.0) "invalid lambdoma ratio: ~S" (getf plan :lambdoma-ratio))
  (format t "~&GENESIS_LOOP_OK soul=~D daily=~D skill=~D signal=~,3F lambdoma=~,3F~%"
          soul-count daily-count skill-count (getf vitruvian :signal) (getf plan :lambdoma-ratio)))

(sb-ext:exit :code 0)
