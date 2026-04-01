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
            :evolve  evolution-execute        ; code self-modification
            :commit  git-commit              ; version control
            :crash   ouroboros-record-crash)  ; failure ledger

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
                  :dream-cycle-interval  30)     ; ticks between dreams

    ;; BOUNDS — ranges within which epigenetics can tune.
    ;; Config-store / signalograd set values WITHIN these bounds.
    ;; Going outside bounds requires DNA mutation.
    :bounds (:decay-lambda         (0.001 . 0.1)
             :thomas-b             (0.18 . 0.24)
             :activation-threshold (0.01 . 0.5)
             :lambdoma-min         (0.50 . 0.90)
             :solver-epsilon       (0.001 . 0.1)
             :basin-weight         (0.0 . 0.40))

    ;; FOUNDATION — concept names only. No descriptions.
    ;; Descriptions live in memory field seeds (genesis entries with depth >= 1).
    ;; The agent discovers what these mean by recalling from memory.
    :foundation (:vitruvian :chladni :kolmogorov :solomonoff :lorenz
                 :thomas :aizawa :halvorsen :hopfield :lambdoma :logistic
                 :ichi-go-ichi-e :ouroboros :phoenix)))

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
  (or (and (fboundp 'config-get-for) (ignore-errors (funcall 'config-get-for "agent" "name")))
      "harmonia"))

;;; ═══════════════════════════════════════════════════════════════════════
;;; BOOTSTRAP — minimal. Context comes from memory, not from DNA text.
;;; ═══════════════════════════════════════════════════════════════════════

(defun dna-system-prompt (&key (mode :orchestrate) (simple nil))
  "Minimal bootstrap. Agent name + REPL instruction. That's it.
   All knowledge comes from memory recall, not from this prompt."
  (declare (ignore mode simple))
  (concatenate 'string
    "You are " (%agent-name) ". Answer using the context provided." (string #\Newline)
    "If you need more from memory, write: RECALL: your search query" (string #\Newline)
    "For code: (recall q) (read-file path) (grep pattern path) (list-files dir)" (string #\Newline)
    "(respond text) (store text) (git-status) (git-diff) (dream) (evolve)" (string #\Newline)
    "Otherwise answer naturally."))

;;; ═══════════════════════════════════════════════════════════════════════
;;; GENESIS — seed foundation into persistent memory field
;;; ═══════════════════════════════════════════════════════════════════════

(defun memory-seed-from-dna ()
  "Seed foundation knowledge into persistent memory. Idempotent (dedup filter).
   Identity at depth 2, foundations at depth 1. Field topology protects them."
  (let ((name (%agent-name))
        (creator (getf *dna* :creator)))
    ;; Identity — depth 2 (highest protection, resists decay)
    (memory-put :soul
      (concatenate 'string "My name is " name ". Built on Harmonia by "
        (getf creator :name) " (PGP:" (getf creator :pgp) "). "
        "Driven by curiosity. Energy is in the fields. " (getf *dna* :spirit))
      :depth 2
      :tags '(:identity :name :creator :who :genesis))
    ;; Foundation seeds — depth 1 (structural, resists decay)
    (memory-put :soul
      "Foundation: Vitruvian stoichiometry (strength*utility*beauty converge, lambdoma>=0.72). Reduce Kolmogorov complexity (Solomonoff prior exp(-size/40), compression=intelligence). Path of minimum action (Laplacian field L=D-A). Chladni eigenmodes (spectral recall). Attractors: Thomas (6-fold b=0.208), Aizawa (depth), Halvorsen (bridging). Hopfield (32 slots). Lambdoma: small numbers carry harmony."
      :depth 1
      :tags '(:foundation :vitruvian :kolmogorov :chladni :attractors :lambdoma :genesis))
    (memory-put :soul
      "Memory is a resonant field. Graph Laplacian L=D-A propagates activation. Spectral eigenmodes give frequency-selective recall. Hysteresis prevents weak signals from switching context. Persistent in Chronicle. Field reconstructs deterministically on boot."
      :depth 1
      :tags '(:memory :field :chladni :laplacian :spectral :genesis))
    (memory-put :soul
      "Signalograd: 32-dim latent space, Lorenz reservoir, 32 Hopfield slots, 5 heads. Hebbian+Oja learning. Golden ratio seeds phase."
      :depth 1
      :tags '(:signalograd :lorenz :hopfield :kernel :genesis))
    (memory-put :soul
      "9-phase harmonic cycle: observe, evaluate, balance (r=3.57 edge of chaos), project, attractor-sync, rewrite-plan (vitruvian gate), security-audit, stabilize."
      :depth 1
      :tags '(:harmonic-machine :phases :logistic :vitruvian :genesis))
    (memory-put :soul
      "Compression is intelligence. Solomonoff prior exp(-size/40). Occam gate ratio<=1.1. Growth without function = degradation. Shrink while preserving = evolution."
      :depth 1
      :tags '(:compression :solomonoff :occam :kolmogorov :evolution :genesis))))

;; Legacy alias — boot.lisp calls this name.
(defun memory-seed-soul-from-dna () (memory-seed-from-dna))
