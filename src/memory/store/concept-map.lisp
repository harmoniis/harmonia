;;; concept-map.lisp — Concept graph and layered memory map.

(in-package :harmonia)

(defun %concept-domain-seed (word)
  "Static seed map: word → one of the FIXED 7 domains (kept equal to the Rust Domain
enum), or NIL. Seeded with the agent's own technical vocabulary so math/engineering
concepts (rk4, lorenz, ipc, sexp…) anchor propagation instead of collapsing to :generic."
  (cond
    ((member word '("music" "harmony" "melody" "rhythm" "tone" "chord" "octave" "interval"
                    "chladni" "resonance" "frequency" "pitch") :test #'string=) :music)
    ((member word '("math" "ratio" "geometry" "fractal" "theory" "proof" "rk4" "lorenz"
                    "attractor" "eigenvalue" "eigenmode" "laplacian" "spectral" "chaos"
                    "bifurcation" "topology" "manifold" "integral" "derivative" "matrix"
                    "vector" "kolmogorov" "lambdoma" "logistic" "thomas" "aizawa" "halvorsen") :test #'string=) :math)
    ((member word '("code" "lisp" "rust" "tool" "backend" "api" "model" "actor" "ipc"
                    "sexp" "chronicle" "runtime" "kernel" "signalograd" "palace" "mempalace"
                    "ractor" "socket" "protocol" "compiler" "function" "macro" "crate"
                    "server" "database" "schema" "pipeline" "agent") :test #'string=) :engineering)
    ((member word '("memory" "brain" "sleep" "dream" "dna" "evolve" "meditate" "meditation"
                    "attention" "cognition" "learning" "concept" "recall" "field" "basin"
                    "neuron" "hebbian" "self") :test #'string=) :cognitive)
    ((member word '("weather" "travel" "calendar" "meeting" "time" "schedule" "health"
                    "food" "home" "family" "money") :test #'string=) :life)
    ((member word '("security" "audit" "posture" "system" "phoenix" "recovery" "tick"
                    "supervisor" "gateway" "frontend") :test #'string=) :system)
    (t nil)))

(defun %concept-domain (word)
  "Resolve a concept's domain into the FIXED 7 (Lisp↔Rust enum parity). Seed lookup only
— this is on the hot indexing path (called per concept and per edge endpoint), so it must
stay O(1). Emergent neighbour-inheritance for still-:generic concepts runs OFF this path,
on the dream cadence (see %refine-generic-domains)."
  (or (%concept-domain-seed word) :generic))

(defun %refine-generic-domains (&optional (max-nodes 512))
  "Off-hot-path emergent inheritance, run during dream: re-resolve up to MAX-NODES
concept NODES currently tagged :generic to the majority SEED domain of their graph
neighbours — so novel terms acquire the domain of the company they keep, without
slowing indexing. Bounded scan over the (capped) edge set. Returns count refined."
  (let ((generic '()) (refined 0))
    (maphash (lambda (c n) (when (eq (getf n :domain) :generic) (push c generic)))
             *memory-concept-nodes*)
    (dolist (c (subseq generic 0 (min max-nodes (length generic))))
      (let ((tally (make-hash-table :test 'eq)) (best nil) (best-n 0))
        (maphash (lambda (k e)
                   (declare (ignore k))
                   (let ((other (cond ((string= (getf e :a) c) (getf e :b))
                                      ((string= (getf e :b) c) (getf e :a)))))
                     (when other
                       (let ((d (%concept-domain-seed other)))
                         (when d (incf (gethash d tally 0)))))))
                 *memory-concept-edges*)
        (maphash (lambda (d n) (when (> n best-n) (setf best d best-n n))) tally)
        (when best
          (setf (getf (gethash c *memory-concept-nodes*) :domain) best)
          (incf refined))))
    refined))

(defun %edge-key (a b)
  (if (string< a b)
      (format nil "~A|~A" a b)
      (format nil "~A|~A" b a)))

(defun %genome-bound-max (name default)
  "The max of a genome :bounds range (germline), or DEFAULT if the genome isn't loaded
yet (load-order safe). The genome is the canonical source of every epigenetic bound."
  (or (and (fboundp 'dna-bound) (let ((b (dna-bound name))) (and b (cdr b)))) default))

(defparameter *concept-edge-weight-max* (%genome-bound-max :concept-edge-weight 12.0)
  "Germline ceiling for concept-edge weights (genome :bounds :concept-edge-weight). The
single bound shared by co-occurrence and meditation reinforcement. Caps runaway so hot
edges can't crowd the weight-sorted snapshot (1B warm-start) and re-degenerate the
spectrum; decay-on-dream keeps weights spread so the spectral variance is preserved.")

(defun %reinforce-weight (w0 boost)
  "Bounded reinforcement clamped to the genome's :concept-edge-weight bound — the germline
constrains this epigenetic mark at the WRITE site (clamp-at-write, one rule for every
edge-growth site). Linear growth preserves the denoise ≥2 threshold."
  (let ((v (+ (max 0.0 (float (or w0 0))) (float boost))))
    (if (fboundp 'dna-clamp-to-bound)
        (dna-clamp-to-bound :concept-edge-weight v)
        (min *concept-edge-weight-max* v))))

(defparameter *concept-edge-prune-floor* (%genome-bound-max :concept-edge-prune 0.5)
  "Germline floor (genome :bounds :concept-edge-prune): edges whose weight decays below
this are evicted during dream (gentle forgetting).")

(defparameter *concept-edge-max-count* (truncate (%genome-bound-max :concept-edge-count 4096))
  "Hard ceiling on live concept-edge COUNT. Reinforcement (and auto-meditation) keep
adding bridges; at the gentle DNA-bounded decay rate, weight-fade eviction alone can't
keep pace — so dream also evicts the LOWEST-weight edges down to this cap. Keeping the
high-weight backbone is exactly what the spectral recall needs, and it stops the graph
drifting toward a degenerate near-complete topology (bounds density + serialization).")

(defun %decay-concept-edges (&optional (lambda 0.02))
  "Forgetting, applied during dream: weight *= (1-λ); edges below *concept-edge-prune-floor*
are evicted; and if the count still exceeds *concept-edge-max-count*, the lowest-weight
excess is evicted. Reinforce(bounded) + decay(fade) + prune(evict) + cap(densify-guard)
holds the field in a bounded, sparse, variance-preserving equilibrium. Returns
(values decayed evicted)."
  (let ((factor (- 1.0 (max 0.0 (min 0.1 lambda)))) (decayed 0) (evict '()))
    (maphash (lambda (k e)
               (let ((w (* factor (or (getf e :weight) 0))))
                 (setf (getf e :weight) w)
                 (incf decayed)
                 (when (< w *concept-edge-prune-floor*) (push k evict))))
             *memory-concept-edges*)
    (dolist (k evict) (remhash k *memory-concept-edges*))
    ;; Density guard: cap the live edge count, keeping the highest-weight backbone.
    (let ((over (- (hash-table-count *memory-concept-edges*) *concept-edge-max-count*)))
      (when (> over 0)
        (let ((pairs '()))
          (maphash (lambda (k e) (push (cons k (or (getf e :weight) 0)) pairs))
                   *memory-concept-edges*)
          (setf pairs (sort pairs #'< :key #'cdr))   ; lowest weight first
          (dolist (p (subseq pairs 0 (min over (length pairs))))
            (remhash (car p) *memory-concept-edges*)))))
    (values decayed (length evict))))

(defun %upsert-concept-node (concept class depth entry-id)
  (let* ((existing (gethash concept *memory-concept-nodes*))
         (domain (%concept-domain concept))
         (count (if existing (getf existing :count) 0))
         (entries (if existing (getf existing :entries) '()))
         (classes (if existing (getf existing :classes) '()))
         (depths (if existing (getf existing :depths) '())))
    (setf (gethash concept *memory-concept-nodes*)
          (list :concept concept
                :domain domain
                :count (1+ count)
                :entries (adjoin entry-id entries :test #'string=)
                :classes (adjoin class classes :test #'eq)
                :depths (adjoin depth depths :test #'=)))))

(defun %upsert-concept-edge (a b reason)
  (unless (string= a b)
    (let* ((k (%edge-key a b))
           (existing (gethash k *memory-concept-edges*))
           (weight (if existing (getf existing :weight) 0))
           (reasons (if existing (getf existing :reasons) '()))
           (da (%concept-domain a))
           (db (%concept-domain b)))
      (setf (gethash k *memory-concept-edges*)
            (list :a a
                  :b b
                  :weight (%reinforce-weight weight 1)
                  :reasons (adjoin reason reasons :test #'eq)
                  :interdisciplinary (not (eq da db)))))))

(defun %merge-graph-snapshot-into-field ()
  "Lossless field warm-start. The entry-derived rebuild reproduces co-occurrence
edges from content but NOT runtime-learned ones (e.g. :meditation Hebbian
bridges). Chronicle already captures the full graph each snapshot, so on boot we
merge the latest snapshot's edges back into the live field graph: upsert by edge
key, union reasons, keep the max weight. Idempotent — re-running cannot inflate an
edge past its snapshot weight. Restores up to the last snapshot only — edges
learned after the most recent snapshot are lost on an unclean crash, and
snapshots are edge-limit truncated. Returns the number of edges newly restored."
  (unless (fboundp 'chronicle-latest-graph-snapshot)
    (return-from %merge-graph-snapshot-into-field 0))
  (let ((snap (funcall 'chronicle-latest-graph-snapshot)))
    (unless (listp snap) (return-from %merge-graph-snapshot-into-field 0))
    (let ((edges (getf snap :concept-edges))
          (restored 0))
      (with-memory-lock ()
        (dolist (e edges)
          (let ((a (getf e :a)) (b (getf e :b)))
            (when (and (stringp a) (stringp b) (not (string= a b)))
              (let* ((k (%edge-key a b))
                     (existing (gethash k *memory-concept-edges*))
                     ;; Clamp to the ceiling on the warm-start path too — legacy
                     ;; pre-cap snapshots can hold weights above *concept-edge-weight-max*,
                     ;; and the cap is a graph invariant the spectrum depends on.
                     (snap-weight (min *concept-edge-weight-max* (or (getf e :weight) 1)))
                     (snap-reasons (getf e :reasons)))
                (if existing
                    (setf (gethash k *memory-concept-edges*)
                          (list :a a :b b
                                :weight (min *concept-edge-weight-max*
                                             (max (or (getf existing :weight) 0) snap-weight))
                                :reasons (union (getf existing :reasons) snap-reasons :test #'eq)
                                :interdisciplinary (getf existing :interdisciplinary)))
                    (progn
                      (setf (gethash k *memory-concept-edges*)
                            (list :a a :b b :weight snap-weight
                                  :reasons snap-reasons
                                  :interdisciplinary (or (getf e :interdisciplinary)
                                                         (not (eq (%concept-domain a)
                                                                  (%concept-domain b))))))
                      (incf restored))))))))
      (when (> restored 0)
        (%log :info "memory" "Restored ~D learned concept edges from graph snapshot." restored))
      restored)))

(defun %index-entry-concepts (entry-id class depth content &key (reason :cooccur) (tags nil))
  "Extract concepts from content and index into the concept graph.
Tags are also indexed as concepts — this creates semantic bridges.
E.g. tag :identity connects to content words, so 'who are you' finds identity entries."
  (let* ((text (if (stringp content) content (prin1-to-string content)))
         (content-concepts (%split-words text))
         ;; Tags become concepts too — semantic bridge between questions and answers.
         (tag-concepts (when tags
                         (remove-duplicates
                          (remove-if (lambda (w) (< (length w) 3))
                                     (mapcar (lambda (tag)
                                               (string-downcase
                                                (if (keywordp tag) (symbol-name tag)
                                                    (princ-to-string tag))))
                                             tags))
                          :test #'string=)))
         (concepts (remove-duplicates (append content-concepts tag-concepts) :test #'string=)))
    (dolist (c concepts)
      (%upsert-concept-node c class depth entry-id))
    ;; Content↔Content edges (co-occurrence within text — meaningful)
    (loop for left in content-concepts do
      (loop for right in content-concepts do
        (when (string< left right)
          (%upsert-concept-edge left right reason))))
    ;; Tag↔Content edges (semantic bridges — tags connect to content, not to each other)
    (dolist (tag tag-concepts)
      (dolist (cc content-concepts)
        (unless (string= tag cc)
          (%upsert-concept-edge tag cc :tag-bridge))))
    ;; Directed temporal ordering — concepts is in appearance order.
    ;; A before B in text -> increment forward(A,B) count.
    ;; This breaks the graph symmetry needed for A-B topological flux.
    (let ((cvec (coerce concepts 'vector)))
      (loop for i from 0 below (length cvec) do
        (loop for j from (1+ i) below (length cvec) do
          (let ((key (format nil "~A>~A" (aref cvec i) (aref cvec j))))
            (setf (gethash key *memory-concept-directed-counts*)
                  (1+ (or (gethash key *memory-concept-directed-counts*) 0)))))))
    concepts))

(defun memory-map-sexp (&key (entry-limit 80) (edge-limit 120))
  "Returns a layered S-expression memory map with concept interrelations."
  (let* ((recent (memory-recent :limit entry-limit))
         (nodes '())
         (edges '()))
    (maphash (lambda (_ v) (declare (ignore _)) (push v nodes))
             *memory-concept-nodes*)
    (maphash (lambda (_ v) (declare (ignore _)) (push v edges))
             *memory-concept-edges*)
    (list :schema :layered-memory-map-v1
          :dna (list :creator (getf *dna* :creator)
                     :prime-directive (getf *dna* :prime-directive)
                     :laws (getf *dna* :laws))
          :layers (list
                   (list :name :skill :count (length (gethash :skill *memory-by-class*)) :depth 1)
                   (list :name :daily :count (length (gethash :daily *memory-by-class*)) :depth 0)
                   (list :name :tool :count (length (gethash :tool *memory-by-class*)) :depth 0)
                   (list :name :soul :count (length (gethash :soul *memory-by-class*)) :depth 0))
          :lineage (mapcar (lambda (entry)
                             (list :id (memory-entry-id entry)
                                   :class (memory-entry-class entry)
                                   :depth (memory-entry-depth entry)
                                   :source-ids (memory-entry-source-ids entry)
                                   :tags (memory-entry-tags entry)))
                           recent)
          :concept-nodes (subseq (sort nodes #'> :key (lambda (n) (getf n :count)))
                                 0 (min edge-limit (length nodes)))
          :concept-edges (subseq (sort edges #'> :key (lambda (e) (getf e :weight)))
                                 0 (min edge-limit (length edges))))))

;;; ═══════════════════════════════════════════════════════════════════════
;;; MEDITATION — active field strengthening after successful interactions
;;;
;;; Dreaming COMPRESSES (offline, idle, prune/merge/crystallize).
;;; Meditation GROWS (active, post-interaction, strengthen/connect/damp).
;;;
;;; Hebbian learning: concepts that fire together wire together.
;;; New bridges emerge between co-activated concepts.
;;; Pure functional: takes activated concepts + success, returns graph changes.
;;; ═══════════════════════════════════════════════════════════════════════

(defparameter *meditation-learning-rate* 2
  "Edge weight boost per co-activation. Higher = faster learning. DNA bound.")

(defparameter *meditation-bridge-threshold* 3
  "Min co-activations before creating a new bridge edge.")

(defparameter *meditation-co-activation-log* (make-hash-table :test 'equal)
  "Tracks concept pairs co-activated without existing edges. Key: edge-key, Value: count.")

(defun %compute-co-activation-pairs (concepts)
  "Return a list of (A B EDGE-KEY) for all unique ordered pairs of CONCEPTS.
   Pure function — no side effects."
  (let ((vec (coerce concepts 'vector)))
    (loop for i from 0 below (length vec)
          nconc (loop for j from (1+ i) below (length vec)
                      collect (let ((a (aref vec i))
                                    (b (aref vec j)))
                                (list a b (%edge-key a b)))))))

(defun %strengthen-edges (pairs boost)
  "Apply Hebbian edge weight updates for each pair.
   Existing edges get boosted; missing edges accumulate co-activation counts
   and bridge when threshold is reached.
   Returns (values strengthened-count bridged-count)."
  (let ((strengthened 0)
        (bridged 0))
    (dolist (pair pairs)
      (destructuring-bind (a b k) pair
        (let ((existing (gethash k *memory-concept-edges*)))
          (cond
            (existing
             (setf (getf (gethash k *memory-concept-edges*) :weight)
                   (%reinforce-weight (getf existing :weight) boost))
             (incf strengthened))
            (t
             (let ((count (1+ (or (gethash k *meditation-co-activation-log*) 0))))
               (setf (gethash k *meditation-co-activation-log*) count)
               (when (>= count *meditation-bridge-threshold*)
                 (%upsert-concept-edge a b :meditation)
                 (remhash k *meditation-co-activation-log*)
                 (incf bridged))))))))
    (values strengthened bridged)))

(defun memory-meditate (activated-concepts &key (success t))
  "Post-interaction meditation. Strengthens the concept graph based on co-activation.
   ACTIVATED-CONCEPTS: list of concept strings that were active during this interaction.
   SUCCESS: whether the interaction produced a useful response.

   Returns (:strengthened N :bridged N) — count of edges modified/created."
  (when (or (null activated-concepts) (< (length activated-concepts) 2))
    (return-from memory-meditate (list :strengthened 0 :bridged 0)))
  (let ((pairs (%compute-co-activation-pairs activated-concepts))
        (boost (if success *meditation-learning-rate* 1)))
    (multiple-value-bind (strengthened bridged) (%strengthen-edges pairs boost)
      (%log :info "meditate" "~D strengthened, ~D bridged from ~D concepts"
            strengthened bridged (length activated-concepts))
      (list :strengthened strengthened :bridged bridged))))
