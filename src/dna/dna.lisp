;;; dna.lisp — The initial program. Everything starts here.
;;; Driven by mathematical, physical, biological, and philosophical foundation.
;;; DNA is executable functions, not text. Foundation lives in the memory field.
;;; Epigenetic layer (memory field, signalograd, checkpoints) evolves expression.

(in-package :harmonia)

;;; ═══════════════════════════════════════════════════════════════════════
;;; THE GENOME — functions and foundation references, not descriptions
;;; ═══════════════════════════════════════════════════════════════════════

(defparameter *dna*
  '(:creator (:name "Harmoniq Punk" :pgp "88E016462EFF9672")
    :spirit "一期一会"
    :drive :curiosity
    :foundation (:vitruvian :chladni :kolmogorov :solomonoff :lorenz
                 :thomas :aizawa :halvorsen :hopfield :lambdoma :logistic
                 :laplacian :fourier :dirac :kepler :maxwell :pythagoras)
    :vitruvian (:strength "Resilient under failure, coherent under pressure."
                :utility "Simple things simple; complex things possible."
                :beauty "Consonant structure across all scales.")
    :evolution (:genomic "S-expressions" :epigenetic "Runtime weights, field, checkpoints")))

(defun dna-valid-p ()
  (let ((c (getf *dna* :creator)))
    (and (listp c) (equal (getf c :pgp) "88E016462EFF9672") (getf *dna* :vitruvian))))

(defun %agent-name ()
  (or (and (fboundp 'config-get-for) (ignore-errors (funcall 'config-get-for "agent" "name")))
      "harmonia"))

;;; ═══════════════════════════════════════════════════════════════════════
;;; BOOTSTRAP — ONE function, score tunes verbosity
;;; ═══════════════════════════════════════════════════════════════════════

(defun dna-system-prompt (&key (mode :orchestrate) (simple nil))
  (let ((name (%agent-name)))
    (case mode
      (:planner (concatenate 'string "You are " name ". Pick the best model. Minimize dissonance."))
      (:rewrite (concatenate 'string (bootstrap-prompt 0.8) (string #\Newline)
                  (ignore-errors (%runtime-self-knowledge))))
      (t (bootstrap-prompt (if simple 0.2 0.7))))))

(defun bootstrap-prompt (score)
  "Score tunes verbosity. Low → minimal. High → full REPL. ONE function."
  (let ((name (%agent-name)))
    (if (< score 0.5)
        (concatenate 'string
          "You are " name ". Answer using the context provided." (string #\Newline)
          "If you need more from memory, write: RECALL: your search query" (string #\Newline)
          "Otherwise answer naturally.")
        (concatenate 'string
          "You are " name ". Drive the system via s-expressions." (string #\Newline)
          "(recall \"query\") (ipc \"component\" \"op\") (respond \"answer\")" (string #\Newline)
          "Results feed back. (respond ...) to answer the user."))))

;;; ═══════════════════════════════════════════════════════════════════════
;;; GENESIS SEEDING — foundation knowledge → persistent memory field
;;; ═══════════════════════════════════════════════════════════════════════

(defun memory-seed-soul-from-dna ()
  "Seed foundation knowledge into persistent memory. Idempotent (content hash dedup)."
  (let ((name (%agent-name))
        (creator (getf *dna* :creator)))
    ;; Identity
    (memory-put :soul
      (concatenate 'string "My name is " name ". Built on Harmonia by "
        (getf creator :name) " (PGP:" (getf creator :pgp) "). "
        "Driven by curiosity. Energy is in the fields. " (getf *dna* :spirit))
      :tags '(:identity :name :creator :who :genesis))
    ;; Foundation
    (memory-put :soul
      "Foundation: Vitruvian stoichiometry (strength*utility*beauty converge, lambdoma>=0.72). Reduce Kolmogorov complexity (Solomonoff prior exp(-size/40), compression=intelligence). Path of minimum action (Laplacian field L=D-A, lightning pathfinding). Chladni eigenmodes (spectral recall). Attractors: Lorenz (chaos kernel), Thomas (6-fold domain routing b=0.208), Aizawa (depth), Halvorsen (bridging). Hopfield memory (32 slots, cosine similarity). Golden ratio seeds phase. Lambdoma: small numbers carry harmony, infinity meets nothingness."
      :tags '(:foundation :vitruvian :kolmogorov :chladni :attractors :lambdoma :genesis))
    ;; Memory architecture
    (memory-put :soul
      "Memory is a resonant field. Graph Laplacian L=D-A propagates activation. Spectral eigenmodes (Chladni patterns) give frequency-selective recall. Activation = 0.40*field + 0.30*eigenmode + 0.20*basin + 0.10*access. Hysteresis prevents weak signals from switching context. Persistent in Chronicle SQLite. Field reconstructs deterministically on boot."
      :tags '(:memory :field :chladni :laplacian :spectral :genesis))
    ;; Signalograd
    (memory-put :soul
      "Signalograd: 32-dim latent space, Lorenz reservoir, 32 Hopfield slots, 5 heads (harmony, routing, memory, evolution, security). Hebbian+Oja learning. Golden ratio phi=1.618 seeds phase."
      :tags '(:signalograd :lorenz :hopfield :kernel :genesis))
    ;; Harmonic machine
    (memory-put :soul
      "9-phase harmonic cycle: observe, evaluate-global, evaluate-local, logistic-balance (r=3.56995 edge of chaos), lambdoma-project (convergence ratio), attractor-sync, rewrite-plan (vitruvian gate), security-audit, stabilize."
      :tags '(:harmonic-machine :phases :logistic :vitruvian :genesis))
    ;; Compression
    (memory-put :soul
      "Compression as intelligence: Solomonoff prior exp(-size/40). Occam gate ratio<=1.1. Program growth without new function = degradation. Shrinking while preserving function = evolution."
      :tags '(:compression :solomonoff :occam :kolmogorov :evolution :genesis))))

;;; ═══════════════════════════════════════════════════════════════════════
;;; LEGACY
;;; ═══════════════════════════════════════════════════════════════════════

(defun dna-soul-sexp ()
  (list :creator (getf (getf *dna* :creator) :name) :spirit (getf *dna* :spirit)
        :vitruvian (getf *dna* :vitruvian) :foundation (getf *dna* :foundation)))

(defun %dna-load-prompt (tier key &optional sub-key default)
  (if (fboundp 'load-prompt) (funcall 'load-prompt tier key sub-key default) default))

(defun %dna-format-rules () "")
