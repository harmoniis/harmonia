;;; resonance-probe-long.lisp — extended probe with many cycles so Signalograd
;;; has time to dispatch and the runtime-state-signalograd-projection populates.

(in-package :cl-user)
(load #P"src/core/boot.lisp")

(defparameter *probe-output* "/tmp/harmonia-resonance-long.sexp")

(defun probe-log (fmt &rest args)
  (apply #'format *error-output* (concatenate 'string "[probe] " fmt "~%") args)
  (force-output *error-output*))

(probe-log "starting harmonia (no auto loop)")
(harmonia:start :run-loop nil)

;; Run a substantial warm-up so Signalograd dispatch fires multiple times.
(probe-log "warm-up: 90 harmonic ticks (approx10 full cycles)")
(harmonia:run-loop :max-cycles 90 :sleep-seconds 0.02)

;; Inspect Signalograd projection AFTER warm-up.
(let ((proj (harmonia::signalograd-current-projection harmonia:*runtime*)))
  (probe-log "post-warmup signalograd projection: ~S"
             (if (and (listp proj) proj)
                 (list :keys (loop for (k v) on proj by #'cddr collect k)
                       :confidence (getf proj :confidence))
                 proj)))

;; Real prompts.
(dolist (p '("Tell me what primitives you have for memory operations and explain how recall is scored."
             "Recall everything related to harmonic dynamics, attractors, and Lambdoma projection."
             "How do the signalograd kernel and the memory-field heat kernel relate to each other?"
             "Summarise three concepts you remember best, in order of confidence."
             "Are there any inconsistencies between how memory is stored vs how it is recalled?"))
  (probe-log "→ ~A" p)
  (handler-case (harmonia:run-prompt p :max-cycles 2)
    (error (e) (probe-log "  ERROR: ~A" e))))

(probe-log "post-prompt: 180 harmonic ticks (approx20 full cycles)")
(harmonia:run-loop :max-cycles 180 :sleep-seconds 0.02)

;; Final projection inspection.
(let ((proj (harmonia::signalograd-current-projection harmonia:*runtime*)))
  (probe-log "FINAL signalograd projection: ~S"
             (if (and (listp proj) proj)
                 (list :keys (loop for (k v) on proj by #'cddr collect k)
                       :confidence (getf proj :confidence)
                       :harmony-section (getf proj :harmony))
                 proj)))

;; Construct a real observation packet and dump the values of every newly
;; wired key so we can verify the loops carry live data, not zeros.
(let* ((ctx (harmonia::runtime-state-harmonic-context harmonia:*runtime*))
       (obs-sexp (harmonia::%signalograd-observation-sexp
                   (or ctx '(:cycle 0
                             :map (:concept-nodes nil :concept-edges nil)
                             :field-basin (:dwell-ticks 30)))
                   harmonia:*runtime*)))
  (probe-log "OBSERVATION SEXP (sample): ~A"
             (if (> (length obs-sexp) 320)
                 (concatenate 'string (subseq obs-sexp 0 320) "…")
                 obs-sexp))
  (dolist (key '(":field-recall-strength" ":field-basin-stability"
                 ":field-eigenmode-coherence" ":datamine-success-rate"
                 ":datamine-avg-latency" ":palace-graph-density"))
    (let ((pos (search key obs-sexp)))
      (probe-log "  ~A present=~A" key (if pos "yes" "NO")))))

;; Dump all events.
(let* ((events (harmonia::runtime-state-events harmonia:*runtime*))
       (sorted (sort (copy-list events) #'< :key (lambda (e) (getf e :time))))
       (by-tag (let ((h (make-hash-table)))
                 (dolist (e sorted)
                   (push e (gethash (getf e :tag) h)))
                 h)))
  (probe-log "captured ~D events across ~D distinct tags"
             (length sorted) (hash-table-count by-tag))
  (with-open-file (out *probe-output* :direction :output :if-exists :supersede :if-does-not-exist :create)
    (let ((*print-pretty* nil) (*print-readably* t))
      (dolist (e sorted)
        (format out "~S~%" e))))
  (probe-log "wrote ~A" *probe-output*)

  ;; Tag histogram
  (format t "~%── EVENT TAG HISTOGRAM ──~%")
  (let ((entries nil))
    (maphash (lambda (k v) (push (cons k (length v)) entries)) by-tag)
    (dolist (e (sort entries #'> :key #'cdr))
      (format t "  ~30A ~D~%" (car e) (cdr e))))

  ;; Resonance summary
  (let ((plans (gethash :harmonic-rewrite-plan by-tag))
        (calls (gethash :repl-llm-call by-tag))
        (sg-projs (gethash :signalograd-projection by-tag)))
    (format t "~%── RESONANCE GATE ──~%")
    (when plans
      (let ((ready 0) (lambdas nil) (chaos nil) (sigs nil) (confs nil))
        (dolist (e plans)
          (let* ((p (getf e :payload)) (vit (getf p :vitruvian)))
            (when (getf p :ready) (incf ready))
            (when (getf p :lambdoma-ratio) (push (getf p :lambdoma-ratio) lambdas))
            (when (getf p :chaos-risk) (push (getf p :chaos-risk) chaos))
            (when (getf p :confidence) (push (getf p :confidence) confs))
            (when (and vit (getf vit :signal)) (push (getf vit :signal) sigs))))
        (flet ((mn (xs) (if xs (/ (reduce #'+ xs) (length xs)) 'n/a))
               (mx (xs) (if xs (reduce #'max xs) 'n/a))
               (mi (xs) (if xs (reduce #'min xs) 'n/a)))
          (format t "  rewrite-ready true: ~D / ~D (approx,1F%)~%"
                  ready (length plans) (* 100.0 (/ ready (max 1 (length plans)))))
          (format t "  lambdoma-ratio   mean=~,3F min=~,3F max=~,3F~%" (mn lambdas) (mi lambdas) (mx lambdas))
          (format t "  chaos-risk       mean=~,3F min=~,3F max=~,3F~%" (mn chaos) (mi chaos) (mx chaos))
          (format t "  vitr signal      mean=~,3F min=~,3F max=~,3F~%" (mn sigs) (mi sigs) (mx sigs))
          (format t "  confidence       mean=~,3F min=~,3F max=~,3F~%" (mn confs) (mi confs) (mx confs)))))
    (format t "~%signalograd-projection events: ~D~%" (length (or sg-projs '())))
    (when sg-projs
      (let ((confs (mapcar (lambda (e) (getf (getf e :payload) :confidence)) sg-projs)))
        (format t "  confidence path: ~{~,3F~^ → ~}~%"
                (remove nil confs))))
    (format t "~%repl-llm-call events: ~D~%" (length (or calls '())))
    (when calls
      (let* ((rounds (mapcar (lambda (e) (getf (getf e :payload) :round)) calls))
             (ready-calls (count-if (lambda (e) (getf (getf e :payload) :rewrite-ready)) calls))
             (max-round (reduce #'max rounds :initial-value 0)))
        (format t "  ready-at-call: ~D / ~D (approx,1F%)~%"
                ready-calls (length calls) (* 100.0 (/ ready-calls (max 1 (length calls)))))
        (format t "  max round seen: ~D~%" max-round)
        (format t "  rounds histogram:~%")
        (loop for r from 1 to max-round
              for c = (count r rounds)
              when (> c 0)
              do (format t "    round ~D: ~D~%" r c))))))

(sb-ext:exit :code 0)
