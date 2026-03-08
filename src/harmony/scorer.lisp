;;; scorer.lisp — Harmonic scoring helpers.

(in-package :harmonia)

(declaim (ftype function harmony-policy-number))

(defun %score-policy-number (path default)
  (if (fboundp 'harmony-policy-number)
      (harmony-policy-number path default)
      default))

(defun %token-estimate (text)
  ;; Cheap token estimator (~4 chars/token) for stable, model-agnostic weighting.
  (max 1 (round (/ (float (length (or text ""))) 4.0))))

(defun %clamp-score (x)
  (max 0.0 (min 1.0 x)))

(defun %codemode-efficiency (context)
  (let* ((tool-calls (max 0 (or (getf context :tool-calls) 0)))
         (llm-calls (max 0 (or (getf context :llm-calls) 0)))
         (final-toks (max 1 (or (getf context :final-tokens) 1)))
         (inter-toks (max 0 (or (getf context :intermediate-tokens) 0)))
         (sources (max 0 (or (getf context :datasource-count) 0)))
         (chain-score (%clamp-score (/ tool-calls (float (max 1 llm-calls)))))
         (relay-ratio (/ inter-toks (float final-toks)))
         (relay-budget (%score-policy-number "score/codemode-relay-budget" 2.0))
         (relay-score (%clamp-score (- 1.0 (/ relay-ratio (max 0.1 relay-budget)))))
         (sources-score (%clamp-score (/ sources 3.0)))
         (w-chain (%score-policy-number "score/codemode-chain-weight" 0.45))
         (w-relay (%score-policy-number "score/codemode-relay-weight" 0.40))
         (w-sources (%score-policy-number "score/codemode-sources-weight" 0.15)))
    (%clamp-score (+ (* w-chain chain-score)
                     (* w-relay relay-score)
                     (* w-sources sources-score)))))

(defun harmonic-score (prompt response &key context)
  "Harmony score including token efficiency and code-mode orchestration efficiency."
  (let* ((prompt-toks (%token-estimate prompt))
         (response-toks (%token-estimate response))
         (inter-toks (max 0 (or (getf context :intermediate-tokens) 0)))
         (p-len (max 1 (length (or prompt ""))))
         (r-len (length (or response "")))
         (density (/ (min r-len 1200) (float p-len)))
         (base (%clamp-score (min 1.0 (/ density 12.0))))
         (eff-den (+ prompt-toks response-toks inter-toks))
         (token-eff (%clamp-score (/ response-toks (float (max 1 eff-den)))))
         (codemode (%codemode-efficiency
                    (append (list :final-tokens response-toks
                                  :intermediate-tokens inter-toks)
                            context)))
         (w-base (%score-policy-number "score/base-weight" 0.40))
         (w-token (%score-policy-number "score/token-efficiency-weight" 0.35))
         (w-codemode (%score-policy-number "score/codemode-efficiency-weight" 0.25))
         (score (%clamp-score (+ (* w-base base)
                                 (* w-token token-eff)
                                 (* w-codemode codemode)))))
    score))
