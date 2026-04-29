;;; memory-routing.sexp — Declarative memory class routing policy.
;;;
;;; Which memory classes go to which layers. Policy, not code logic.
;;; Loaded at boot by load-memory-routing-config.

(:routing
  ;; Classes eligible for L1 field (global concept graph)
  (:field-indexable (:soul :skill :genesis))
  ;; Classes routed to L3 palace at depth 0
  (:palace-worthy (:daily :interaction))
  ;; Classes routed to L3 palace only when depth > 0 (compressed entries)
  (:palace-worthy-with-depth (:skill))
  ;; All classes always go to L2 chronicle (system audit log)
  ;; Class-to-palace-room mapping
  (:class-defaults
    (:daily       :palace-room "daily")
    (:interaction :palace-room "interaction")
    (:skill       :palace-room "skill")
    (:soul        :palace-room "soul")
    (:genesis     :palace-room "genesis")
    (:tool        :palace-room "tool")
    (:other       :palace-room "other")))
