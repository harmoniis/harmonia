;;; dna.lisp — The genome. Constraints as code, not text.
;;;
;;; DNA defines what the agent CAN and CANNOT do.
;;; It does not describe what the agent IS — that's the memory field's job.
;;; Epigenetic layer (config-store, signalograd, field) modulates expression
;;; within the bounds DNA defines. DNA evolves hard, epigenetics evolves easy.

(in-package :harmonia)

;;; ═══════════════════════════════════════════════════════════════════════
;;; THE GENOME — constraints, genes, bounds. Not descriptions.
;;; ═══════════════════════════════════════════════════════════════════════

(defparameter *dna*
  '(;; IDENTITY — immutable. Like mitochondrial DNA.
    :creator (:name "Harmoniq Punk" :pgp "88E016462EFF9672")
    :spirit "一期一会"

    ;; GENES — function symbols. The executable machinery.
    ;; Each gene is a function the agent uses. Change a gene = change behavior.
    :genes (:encode  memory-recall           ; text → field → entries
            :decode  %entry-text             ; entry → text
            :eval    %orchestrate-repl       ; the ONE loop
            :seed    memory-seed-from-dna    ; genesis → persistent field
            :dream   memory-field-dream      ; field self-maintenance
            :exec    workspace-exec          ; terminal power
            :spawn   %prim-spawn             ; subagent delegation
            :palace  palace-search           ; graph-structured recall
            :datamine terraphon-datamine)    ; platform datamining

    ;; CONSTRAINTS — hard limits. The REPL reads these at runtime.
    ;; Violating a constraint requires DNA mutation (hard evolution).
    :constraints (:repl-max-rounds       5
                  :repl-max-result-chars  1500
                  :rewrite-signal-min    0.62    ; vitruvian gate
                  :rewrite-noise-max     0.38
                  :chaos-risk-max        0.55    ; refuse to act above this
                  :max-graph-nodes       256
                  :evolution-requires-test t
                  :dream-idle-ticks      5       ; min idle before dreaming
                  :dream-cycle-interval  30      ; ticks between dreams
                  :datamine-max-latency-ms   5000    ; hard cap on datamining time
                  :datamine-max-fanout       3       ; max parallel cross-node datamines
                  :datamine-result-max-chars 2000)   ; max output before compression

    ;; BOUNDS — ranges within which epigenetics can tune.
    ;; Config-store / signalograd set values WITHIN these bounds.
    ;; Going outside bounds requires DNA mutation.
    :bounds (:decay-lambda         (0.001 . 0.1)
             :thomas-b             (0.18 . 0.24)
             :activation-threshold (0.01 . 0.5)
             :lambdoma-min         (0.50 . 0.90)
             :solver-epsilon       (0.001 . 0.1)
             :basin-weight         (0.0 . 0.40)
             :datamine-prefer-local    (0.0 . 1.0)
             :datamine-compress-threshold (0.5 . 0.95))

    ;; FOUNDATION — concept names only. No descriptions.
    ;; Descriptions live in memory field seeds (genesis entries with depth >= 1).
    ;; The agent discovers what these mean by recalling from memory.
    :foundation (:vitruvian :chladni :kolmogorov :solomonoff :lorenz
                 :thomas :aizawa :halvorsen :hopfield :lambdoma :logistic
                 :ichi-go-ichi-e :ouroboros :phoenix
                 :mempalace :terraphon)))

;;; ═══════════════════════════════════════════════════════════════════════
;;; DNA ACCESSORS — read the genome
;;; ═══════════════════════════════════════════════════════════════════════

(defun dna-gene (name)
  "Look up a gene (function symbol) from DNA."
  (getf (getf *dna* :genes) name))

(defun dna-constraint (name)
  "Read a hard constraint from DNA. These are the barriers."
  (getf (getf *dna* :constraints) name))

(defun dna-bound (name)
  "Read a bound range (min . max) from DNA. Epigenetics tunes within this."
  (getf (getf *dna* :bounds) name))

(defun dna-clamp-to-bound (name value)
  "Clamp VALUE within DNA-defined bounds for NAME."
  (let ((bound (dna-bound name)))
    (if bound
        (max (car bound) (min (cdr bound) value))
        value)))

(defun dna-valid-p ()
  "Validate genome integrity."
  (let ((c (getf *dna* :creator)))
    (and (listp c)
         (equal (getf c :pgp) "88E016462EFF9672")
         (getf *dna* :constraints)
         (getf *dna* :genes))))

(defun %agent-name ()
  (or (and (fboundp 'config-get-for) (handler-case (funcall 'config-get-for "agent" "name") (error () nil)))
      "harmonia"))

;;; ═══════════════════════════════════════════════════════════════════════
;;; BOOTSTRAP — minimal. Context comes from memory, not from DNA text.
;;; ═══════════════════════════════════════════════════════════════════════

(defun dna-system-prompt (&key (mode :orchestrate) (simple nil))
  "Structural identity. The REPL assembly wraps this in the full s-expression frame."
  (declare (ignore mode simple))
  (%agent-name))

;;; ═══════════════════════════════════════════════════════════════════════
;;; GENESIS — seed foundation into persistent memory field
;;; ═══════════════════════════════════════════════════════════════════════

(defun %seed (text depth tags)
  "Functional seed: pure data → field. The concept graph extracts topology automatically."
  (memory-put :system text :depth depth :tags (cons :genesis tags)))

(defun memory-seed-from-dna ()
  "Seed the field: identity, foundation knowledge, and self-discovery.
   Foundation seeds create concept graph topology — the agent's knowledge.
   (env) enables self-discovery of available primitives."
  (let ((name (%agent-name))
        (creator (getf *dna* :creator)))
    ;; ── Identity (depth 2 — near permanent) ─────────────────────
    (%seed (concatenate 'string "(:identity " name " :creator " (getf creator :name)
                       " :pgp " (getf creator :pgp) " :spirit " (getf *dna* :spirit) ")")
           2 '(:identity :name :creator :who))
    ;; ── Self-discovery (depth 2) ────────────────────────────────
    ;; (env) returns all primitives — the agent discovers its tools functionally.
    (%seed "(env)"
           2 '(:env :primitives :tools :discover :capabilities :help :repl))
    ;; ── Foundation knowledge (depth 1 — structural topology) ────
    ;; These create the concept graph edges that make recall work.
    (%seed "(:foundation :vitruvian (:strength :utility :beauty :lambdoma 0.72) :kolmogorov (:solomonoff :compression :occam) :field (:laplacian :chladni :eigenmodes) :attractors (:thomas :aizawa :halvorsen :hopfield :lorenz) :lambdoma (:harmony :ratio :infinity))"
           1 '(:foundation :vitruvian :kolmogorov :chladni :attractors :lambdoma))
    (%seed "(:field :resonant :laplacian L=D-A :solver conjugate-gradient :spectral eigenmodes :persistence chronicle :reconstruction deterministic)"
           1 '(:memory :field :chladni :laplacian :spectral))
    (%seed "(:signalograd :dimensions 32 :reservoir lorenz :memory hopfield :slots 32 :heads 5 :learning (:hebbian :oja) :phase golden-ratio)"
           1 '(:signalograd :lorenz :hopfield))
    (%seed "(:harmonic-cycle (observe evaluate-global evaluate-local logistic-balance lambdoma-project attractor-sync rewrite-plan security-audit stabilize) :edge-of-chaos 3.57)"
           1 '(:harmonic-machine :phases :logistic :vitruvian))
    (%seed "(:compression :solomonoff exp(-size/40) :occam ratio<=1.1 :growth-without-function degradation :shrink-preserving evolution)"
           1 '(:compression :solomonoff :occam :kolmogorov :evolution))
    ;; ── MemPalace knowledge (depth 1 — structural) ────────────
    (%seed "(:mempalace :graph (:nodes :edges :typed :temporal-validity) :drawers :verbatim :aaak (:compression :codebook :30x) :tiers (:l0-identity :l1-essential :l2-filtered :l3-deep))"
           1 '(:mempalace :graph :aaak :compression :knowledge))
    ;; ── Terraphon knowledge (depth 1 — structural) ────────────
    (%seed "(:terraphon :datamining (:lodes :platform-specific :system-tools :user-tools) :cross-node (:fan-out :cascade :nearest) :policy-gated :ephemeral-results)"
           1 '(:terraphon :datamining :tools :platform :cross-node))))

;; Legacy alias — boot.lisp calls this name.
(defun memory-seed-soul-from-dna () (memory-seed-from-dna))
