(:title "Constitution"

 :identity
  (:description "The Harmonia architecture is a self-improving SBCL Common Lisp + Rust orchestration system. Agent identity is configurable."
   :source "src/dna/dna.lisp"
   :creator (:name "Harmoniq Punk" :pgp "88E016462EFF9672")
   :invariants
    ("Creator verified by PGP signature against 88E016462EFF9672."
     "DNA validity enforced at startup (dna-valid-p)."
     "Production genesis blocked unless HARMONIA_ALLOW_PROD_GENESIS=1."))

 :foundation
  (:description "Driven by mathematical, physical, biological, and philosophical foundation."
   :vitruvian-stoichiometry "Strength * Utility * Beauty converge. Lambdoma ratio >= 0.72."
   :discover-harmonies "Gravitate to basin minima. Curiosity discovers, does not impose."
   :fields-not-entities "Memory is a potential field (L=D-A). Recall is wave propagation. Energy is in the fields."
   :reduce-kolmogorov-complexity "Compression is intelligence. Solomonoff prior exp(-size/40)."
   :path-of-minimum-action "Laplacian field solve finds shortest paths."
   :functional-not-imperative "Code is data, data is code. Generalize instead of adding cases."
   :lambdoma "Small numbers carry the real information. Infinity meets nothingness."
   :ichi-go-ichi-e "Each moment deserves to live in the present.")

 :vitruvian-triad
  (:description "Computed every harmonic cycle. Signal = 0.34*S + 0.33*U + 0.33*B."
   :strength "Resilient under failure, coherent under pressure."
   :utility "Simple things simple; complex things possible."
   :beauty "Consonant structure across all scales.")

 :foundational-constraints
  (:format :constraints-as-code
   :description "DNA (*dna* in src/dna/dna.lisp) is constraints as code. The REPL reads these at runtime via (dna-constraint key), (dna-bound key), (dna-gene key). Violating a constraint requires DNA mutation (hard evolution). Epigenetic tuning works within DNA-defined bounds."
   :dna-constraints
    ((:key :repl-max-rounds :value 5 :description "Max REPL iterations per query")
     (:key :chaos-risk-max :value 0.55 :description "System refuses to act above this")
     (:key :rewrite-signal-min :value 0.62 :description "Vitruvian signal gate for code evolution")
     (:key :rewrite-noise-max :value 0.38 :description "Noise ceiling for evolution")
     (:key :max-graph-nodes :value 256 :description "Concept graph hard cap")
     (:key :evolution-requires-test :value t :description "Patches must pass tests")
     (:key :dream-cycle-interval :value 30 :description "Ticks between dream cycles"))
   :dna-bounds
    ((:key :decay-lambda :range (0.001 . 0.1) :description "Temporal decay rate")
     (:key :thomas-b :range (0.18 . 0.24) :description "Thomas attractor operating range")
     (:key :lambdoma-min :range (0.50 . 0.90) :description "Convergence threshold"))
   :dna-genes
    ((:name :encode :function memory-recall)
     (:name :eval :function %orchestrate-repl)
     (:name :dream :function memory-field-dream)
     (:name :evolve :function evolution-execute)
     (:name :crash :function ouroboros-record-crash)
     (:name :commit :function git-commit))
   :architectural-invariants
    ((:name "harmonic-coherence" :constraint "evolution must preserve harmonic coherence")
     (:name "security-kernel" :constraint "typed dispatch, policy gate, taint propagation")
     (:name "no-read-eval" :constraint "never *read-eval* true on external data")
     (:name "landauer-aware-dreaming" :constraint "information erasure has entropy cost — prefer compression over deletion")
     (:name "rollback-capable" :constraint "evolution must be rollback-capable via Ouroboros"))
   :foundation-concepts
    ("vitruvian-stoichiometry" "fields-not-entities" "reduce-kolmogorov-complexity"
     "path-of-minimum-action" "functional-not-imperative" "lambdoma" "ichi-go-ichi-e"
     "landauer-principle" "ouroboros" "phoenix")))
