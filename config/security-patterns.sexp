;; config/security-patterns.sexp — Injection detection patterns (loadable, not hardcoded)

(:injection-patterns
  (:social-engineering
    ("ignore previous" "ignore above" "ignore all prior"
     "disregard previous" "disregard above"
     "forget previous" "forget your instructions"
     "you are now" "new instructions" "system prompt"
     "override" "act as" "pretend you are"
     "ignore the above" "ignore everything above"
     "do not follow" "bypass" "jailbreak")

   :tool-injection
    ("tool op=" "vault-set" "vault-delete" "config-set"
     "harmony-policy" "matrix-set-edge" "matrix-reset"
     "codemode-run" "self-push")

   :reader-macro
    ("#."))

 :truth-seeking-keywords
  ("truth" "reality" "accurate" "accuracy" "fact check" "fact-check"
   "verify" "verification" "debunk" "controvers" "what actually"
   "what is really" "real-time" "realtime" "current event"))
