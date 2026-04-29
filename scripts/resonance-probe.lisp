;;; resonance-probe.lisp — capture rewrite-ready / confidence / lambdoma /
;;; chaos-risk telemetry while running real prompts against the live LLM.
;;;
;;; Usage:
;;;   OPENROUTER_API_KEY=... sbcl --disable-debugger --script scripts/resonance-probe.lisp
;;;
;;; Output:
;;;   /tmp/harmonia-resonance.sexp — list of captured events as Lisp s-exprs.

(in-package :cl-user)

(load #P"src/core/boot.lisp")

(defparameter *probe-output* "/tmp/harmonia-resonance.sexp")
(defparameter *probe-deadline* (+ (get-universal-time) 480))

(defun within-deadline-p () (< (get-universal-time) *probe-deadline*))

(defun probe-log (fmt &rest args)
  (apply #'format *error-output* (concatenate 'string "[probe] " fmt "~%") args)
  (force-output *error-output*))

(defun safe-prompt (prompt)
  (when (within-deadline-p)
    (probe-log "→ ~A" prompt)
    (handler-case
        (let* ((t0 (get-internal-real-time))
               (resp (harmonia:run-prompt prompt :max-cycles 2))
               (elapsed-ms (truncate (* 1000 (/ (- (get-internal-real-time) t0)
                                                  (float internal-time-units-per-second))))))
          (probe-log "← ~Dms ~A" elapsed-ms
                     (if (and resp (> (length resp) 80))
                         (concatenate 'string (subseq resp 0 80) "…")
                         (or resp "<nil>")))
          resp)
      (error (e)
        (probe-log "  ERROR: ~A" e)
        nil))))

(probe-log "starting harmonia (no auto loop)")
(harmonia:start :run-loop nil)

;; Run several harmonic cycles before any prompts so the field has structure to recall.
(probe-log "warm-up: 6 harmonic cycles")
(harmonia:run-loop :max-cycles 6 :sleep-seconds 0.01)

;; Real-life complex prompts — varied domains, exercise memory, REPL primitives,
;; multi-round reasoning. Each is a single user input that ought to trigger
;; recall+reasoning+respond.
(safe-prompt "Tell me what primitives you have for memory operations and explain how recall is scored.")
(safe-prompt "Recall everything related to harmonic dynamics, attractors, and Lambdoma projection.")
(safe-prompt "How do the signalograd kernel and the memory-field heat kernel relate to each other?")
(safe-prompt "Summarise three concepts you remember best, in order of confidence.")
(safe-prompt "Are there any inconsistencies between how memory is stored vs how it is recalled?")

;; Run more cycles after prompts to let the harmonic machine evaluate the new state.
(probe-log "post-prompt: 18 harmonic cycles")
(harmonia:run-loop :max-cycles 18 :sleep-seconds 0.01)

;; Extract telemetry.
(let* ((events (harmonia::runtime-state-events harmonia:*runtime*))
       (relevant (remove-if-not
                   (lambda (e)
                     (member (getf e :tag)
                             '(:repl-llm-call :harmonic-rewrite-plan)))
                   events))
       (sorted (sort (copy-list relevant) #'< :key (lambda (e) (getf e :time)))))
  (probe-log "captured ~D total events, ~D resonance-relevant"
             (length events) (length sorted))
  (with-open-file (out *probe-output* :direction :output :if-exists :supersede :if-does-not-exist :create)
    (let ((*print-pretty* nil) (*print-readably* t) (*print-circle* nil))
      (format out ";; Harmonia resonance telemetry — ~D events~%" (length sorted))
      (format out ";; tags: :repl-llm-call (per LLM round), :harmonic-rewrite-plan (per cycle)~%~%")
      (dolist (e sorted)
        (format out "~S~%" e))))
  (probe-log "wrote ~A" *probe-output*)

  ;; Quick distribution summary
  (let ((llm-calls (remove-if-not (lambda (e) (eq (getf e :tag) :repl-llm-call)) sorted))
        (plans (remove-if-not (lambda (e) (eq (getf e :tag) :harmonic-rewrite-plan)) sorted)))
    (format t "~&~%── SUMMARY ──~%")
    (format t "harmonic-rewrite-plan events: ~D~%" (length plans))
    (when plans
      (let ((ready 0) (sigs nil) (lambdas nil) (chaos nil) (confs nil))
        (dolist (e plans)
          (let* ((p (getf e :payload))
                 (vit (getf p :vitruvian)))
            (when (getf p :ready) (incf ready))
            (when (getf p :lambdoma-ratio) (push (getf p :lambdoma-ratio) lambdas))
            (when (getf p :chaos-risk) (push (getf p :chaos-risk) chaos))
            (when (getf p :confidence) (push (getf p :confidence) confs))
            (when (and vit (getf vit :signal)) (push (getf vit :signal) sigs))))
        (flet ((mn (xs) (if xs (/ (reduce #'+ xs) (length xs)) 'n/a)))
          (format t "  rewrite-ready true: ~D / ~D (~,1F%)~%"
                  ready (length plans)
                  (* 100.0 (/ ready (max 1 (length plans)))))
          (format t "  mean lambdoma-ratio: ~,3F~%" (mn lambdas))
          (format t "  mean chaos-risk:     ~,3F~%" (mn chaos))
          (format t "  mean vitr signal:    ~,3F~%" (mn sigs))
          (format t "  mean confidence:     ~,3F~%" (mn confs)))))
    (format t "~%repl-llm-call events: ~D~%" (length llm-calls))
    (when llm-calls
      (let ((ready-at-call 0))
        (dolist (e llm-calls)
          (let ((p (getf e :payload)))
            (when (getf p :rewrite-ready) (incf ready-at-call))))
        (format t "  rewrite-ready true at call: ~D / ~D (~,1F%)~%"
                ready-at-call (length llm-calls)
                (* 100.0 (/ ready-at-call (max 1 (length llm-calls)))))))))

(sb-ext:exit :code 0)
