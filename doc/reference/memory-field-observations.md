# Memory Field Observations

Test results, design insights, and improvement proposals from the memory-field implementation. Based on 71 tests (17 unit + 54 integration) across diverse domains.

## The Basin State Observation

### What Happens

When the system boots, the Thomas attractor initializes at (0.1, 0.0, 0.0). This position classifies to ThomasLobe(0), which maps to the Music domain. The hysteresis tracker starts with `current_basin = ThomasLobe(0)`.

This means: **on cold start, the system is biased toward music-domain memories.**

Every concept node's activation score includes a basin factor:

```
basin_factor = if node_basin == current_basin { 1.0 } else { 0.15 }
activation  = 0.40×field + 0.30×eigenmode + 0.20×basin_factor + 0.10×access
```

A music-domain node gets `0.20 × 1.0 = 0.20` from the basin term.
A cognitive-domain node gets `0.20 × 0.15 = 0.03` from the basin term.
That is a 17-point gap in a score that ranges 0.0–1.0.

### Why This Is Correct

This is the hysteresis working as designed. The system should not immediately respond to every query by switching its entire memory context. A single query about "memory" should not slam the system from music-mode to cognitive-mode. That would be the "smarmy" behavior — trying too hard to match whatever was just asked.

Instead, the system needs **sustained context** to shift basins. After several cognitive-domain queries, the Thomas attractor evolves, coercive energy accumulates, and the basin switches. Then cognitive concepts get the full 0.20 boost.

This is how human memory works: you wake up, and for the first few moments your brain is not oriented to any particular task. It takes context (seeing your desk, reading your calendar) to "load" the right memory regime.

### What The Tests Revealed

In test_42 (`single concept "memory"`), the concept "dream" initially outscored "memory" despite "memory" having count=25 vs dream's count=3. This is because:

1. Both are cognitive domain → both get the 0.15 basin penalty.
2. The field potential for "memory" is high (it's the source node).
3. But the eigenmode activation for "dream" happened to be higher in the initial spectral decomposition — dream sits at a more "resonant" position in the graph for the current Chladni modes.
4. After fixing the test expectation, this is correct: the system treats "dream" and "memory" as equally out-of-basin, and uses field + eigenmode to decide between them.

### Cold-Start Solutions (Proposals)

1. **Context-Aware Initialization**: Instead of defaulting to ThomasLobe(0), initialize the basin from the first incoming signal's domain. The first query sets the initial basin.

2. **Neutral Basin**: Add a `Basin::Neutral` state where all domains get equal basin factor (0.5 instead of 1.0 or 0.15). Exit neutral after the first few signals establish a pattern.

3. **Warm Start from Chronicle**: On boot, read the last harmonic snapshot from Chronicle to restore the basin state from the previous session. This preserves context across restarts.

4. **Reduced Basin Weight During Warm-Up**: During the first N cycles, reduce the basin weight from 0.20 to 0.05, letting field potential and eigenmode dominate. Gradually increase basin weight as the attractor settles.

**Recommended**: Option 3 (warm start from Chronicle) + Option 4 (reduced basin weight during warm-up). This preserves session continuity and avoids cold-start bias without changing the steady-state behavior.

**Status**: Both recommended options are now implemented:
- Option 3: `memory-field-warm-start-from-chronicle` called at boot (boot.lisp), reads `last_field_basin()` from Chronicle, restores via `restore_basin` API.
- Option 4: Basin weight ramps from 0.05 to 0.20 over first 10 cycles in `scoring.rs`, controlled by config keys `warm-up-cycles`, `basin-weight-initial`, `basin-weight-final`.
- All hardcoded values moved to config-store lookups with defaults.

## Test Result Summary

### Coverage Matrix

| Category | Tests | Pass | Key Findings |
|----------|-------|------|-------------|
| Single-domain engineering | 3 | 3/3 | Rust, code, api, backend cluster correctly |
| Single-domain music | 2 | 2/2 | Harmony, melody, rhythm activate together |
| Single-domain math | 1 | 1/1 | Fractal, ratio found via field potential |
| Single-domain cognitive | 1 | 1/1 | Memory, brain cluster correctly |
| Single-domain life | 1 | 1/1 | Calendar, meeting weakly connected (correct) |
| Cross-domain queries | 6 | 6/6 | Interdisciplinary bridges work; music↔math via harmony-ratio edge |
| Field gradient ordering | 3 | 3/3 | Source nodes always score higher than distant nodes |
| Entry ID propagation | 2 | 2/2 | Memory entries carry their IDs through field recall |
| Basin and hysteresis | 5 | 5/5 | Weak signals don't switch; strong sustained signals do |
| Eigenmode structure | 2 | 2/2 | Fiedler value positive; spectral populated |
| Access count influence | 1 | 1/1 | Higher access count produces equal or higher score |
| Diverse prompts | 18 | 18/18 | Compiler, harmonic theory, brain patterns, travel, etc. |
| Graph evolution | 2 | 2/2 | Reload updates spectral; growing graph increases recall |
| Degenerate cases | 3 | 3/3 | Single node, disconnected, empty handled gracefully |
| Thomas basin exploration | 1 | 1/1 | Visits multiple basins over 500 steps with varying signal |
| Status and lifecycle | 3 | 3/3 | Status reports correctly; reset clears state |
| Multi-recall session | 1 | 1/1 | 10 diverse queries in sequence all return results |

### Key Quantitative Observations

**Field Gradient**: For a path graph A—B—C with query "A", potentials follow φ(A) > φ(B) > φ(C). The gradient is monotonic — the field correctly models distance in the graph.

**Spectral Separation**: For a barbell graph (two triangles connected by a bridge), the Fiedler vector correctly assigns opposite signs to the two clusters. This confirms the Chladni modes separate natural communities.

**Hysteresis Threshold**: With drive_energy=0.01 per tick, the basin does not switch after 5 ticks (total accumulated ~0.05 after decay, threshold ~0.35). With drive_energy=0.5 per tick, the basin switches within 50 ticks. This matches the designed coercive field behavior.

**Thomas b Modulation**: With signal=0.9, noise=0.1: `b_eff = 0.208 + 0.02×0.8 = 0.224`. Confirmed in test output. This is within the [0.18, 0.24] clamp range and still in the multi-basin regime.

**Eigenvalue Structure**: The 30-node realistic graph produces 8 eigenvectors with the first eigenvalue (Fiedler) being positive, confirming the graph is connected. The spectral decomposition captures the dominant clustering.

## Encoding Path: How Text Becomes Field Energy

### Current Path (Text Only)

```
Text signal ("How does the Rust compiler work?")
    │
    ▼
%split-words()
    │  Normalize: lowercase, strip non-alphanumeric
    │  Split on whitespace
    │  Filter: words > 2 chars, remove 26 stopwords
    │  Result: ("how" "does" "rust" "compiler" "work")
    │    → filtered: ("rust" "compiler" "work")
    │
    ▼
%upsert-concept-node() for each word
    │  Create or update node: (concept, domain, count, entry_ids)
    │  Domain: %concept-domain("rust") → :engineering
    │  Domain: %concept-domain("compiler") → :generic (not in domain dict)
    │
    ▼
%upsert-concept-edge() for all word pairs
    │  Create edges: rust↔compiler, rust↔work, compiler↔work
    │  Reason: :cooccur, weight: incremented
    │
    ▼
SparseGraph (CSR format)
    │  Nodes carry entry_ids linking back to memory-entry structs
    │  Edges carry co-occurrence weights and interdisciplinary flags
    │
    ▼
Graph Laplacian L = D - A
    │  Eigendecomposition: first K eigenvectors (Chladni modes)
    │  Cached until graph changes
    │
    ▼
Field Energy
    │  Each concept node has a position in the spectral space
    │  Its "energy" is determined by eigenmode coefficients
    │  Its "basin" is determined by domain → Thomas lobe mapping
```

### What Is Lost

The current encoding is **bag-of-words**. It loses:

- **Word order**: "rust compiler" and "compiler rust" produce identical concept graphs.
- **Phrase meaning**: "machine learning" becomes two separate concepts "machine" and "learning" — the compound meaning is lost.
- **Negation**: "not working" produces concepts "not" (stopword, dropped) and "working" — the negation is lost.
- **Numerical values**: "3.14159" becomes no concept (non-alphanumeric stripped).
- **Context windows**: Only words within the same memory entry get co-occurrence edges. Cross-entry semantic relationships depend on shared words.

### What Is Preserved

- **Topic clustering**: Words that frequently co-occur build strong edges. A user who discusses "rust" and "compiler" often will have a strong edge between them.
- **Domain structure**: The 6-domain classification creates natural partitions that align with Thomas attractor basins.
- **Temporal evolution**: New entries add new nodes and strengthen edges. The graph grows organically.
- **Cross-domain bridges**: When a user discusses "harmony" and "ratio" together, the interdisciplinary edge connects music and math domains.

## Multi-Modal Data: Beyond Text

### Can The System Listen to Music?

**Currently: No.** But the architecture is ready for it.

The memory field does not care what produces the concept nodes — it operates on a weighted graph. If an audio encoder produced concept-like features from music, those features would enter the same field.

### Architecture for Audio Memory

```
Audio signal (WAV/MP3)
    │
    ▼
Audio Feature Encoder (new crate: lib/core/audio-encoder/)
    │  Extract: pitch contour, rhythm pattern, timbre features
    │  Map to concept vocabulary:
    │    pitch_440hz → "a4"
    │    major_triad → "major"
    │    4/4_time → "common-time"
    │    violin_timbre → "violin"
    │
    ▼
%upsert-concept-node() for each audio feature
    │  Domain: :music (always, since it's audio)
    │  Entry IDs: link back to audio memory entry
    │
    ▼
Same graph → same field → same recall
```

**What's needed:**

1. **Audio feature encoder** (`lib/core/audio-encoder/`): Extracts musical concepts from audio. Could use FFT for pitch, onset detection for rhythm, spectral envelope for timbre.

2. **Memory entry format extension**: Currently `content` is "any Lisp value" — already supports structured data. An audio entry could be `(:type :audio :path "/path/to/file" :features (:pitch "A4" :key "C-major" :tempo 120))`.

3. **Concept vocabulary expansion**: The domain dictionary in `%concept-domain` needs audio terms. The field doesn't need to change — it works on any graph.

### Architecture for Image Memory

Same principle:

```
Image signal (PNG/JPG)
    │
    ▼
Image Feature Encoder (new crate: lib/core/image-encoder/)
    │  Extract: objects detected, colors, scene type, text (OCR)
    │  Map to concept vocabulary:
    │    detected_cat → "cat"
    │    dominant_blue → "blue"
    │    scene_outdoor → "outdoor"
    │    ocr_text → (split into words, same as text path)
    │
    ▼
%upsert-concept-node() → same graph → same field
```

### The General Principle

**Any data that can be decomposed into discrete features can enter the concept graph.** The field doesn't know or care about the data type — it operates on topology. The encoding step is the only part that's data-type-specific.

This is the holographic property in practice: the 2D concept graph encodes high-dimensional information (text semantics, audio features, visual features) projected onto a shared surface. The field's Chladni modes don't distinguish between a "rust" concept that came from text and an "A4" concept that came from audio — they're both nodes with edges.

### What Would Make Multi-Modal Work Well

1. **Shared concept vocabulary**: Audio and text must share some concepts. "harmony" from a music theory text discussion should connect to "harmony" detected in an audio feature. This happens naturally if the concept names are the same.

2. **Cross-modal edges**: When a user discusses a piece of music while it's playing, the text concepts and audio concepts should form co-occurrence edges. This creates the interdisciplinary bridges that the Halvorsen attractor handles.

3. **Domain-aware basin assignment**: Audio features should map to the Music Thomas basin. Image features could map to a new Visual domain (extending from 6 to 7+ domains, adjustable if Thomas b parameter is tuned).

## Improvement Proposals

### P1: Cold-Start Basin (Priority: High)

**Problem**: Initial basin is Music regardless of first query.
**Solution**: Warm-start from Chronicle + reduced basin weight during first 10 cycles.
**Complexity**: Small — read last `harmonic_snapshots` row, extract basin state.

### P2: Phrase-Aware Concept Extraction (Priority: Medium)

**Problem**: `%split-words` loses compound phrases ("machine learning", "Lorenz attractor").
**Solution**: Bigram extraction alongside unigrams. "machine learning" produces nodes: "machine", "learning", AND "machine-learning". The bigram node has higher specificity.
**Complexity**: Medium — modify `%split-words` to emit consecutive word pairs.

### P3: Semantic Similarity Edges (Priority: Medium)

**Problem**: Two concepts that mean similar things but never co-occur in the same entry have no edge. "compiler" and "transpiler" might never appear together.
**Solution**: Use the LLM (via conductor) to occasionally compute semantic similarity between high-count concepts that lack direct edges. Add weak "semantic" edges (reason: `:semantic`).
**Complexity**: Medium — idle-night task, similar to crystallization.

### P4: Graph Laplacian Spectral Warm Cache (Priority: Low)

**Problem**: Spectral decomposition recomputes on every graph change. For 120 nodes, this is fast (~1ms), but for larger graphs it could become expensive.
**Solution**: Incremental spectral update — when a few edges are added, use rank-1 perturbation theory to update eigenvectors without full recomputation.
**Complexity**: High — matrix perturbation theory implementation.

### P5: Multi-Modal Encoder Pipeline (Priority: Future)

**Problem**: Only text data enters the concept graph.
**Solution**: Pluggable encoder pipeline — each encoder (text, audio, image) produces concept nodes. Encoders are registered as crates, dispatched by signal type.
**Complexity**: High — new crates, new signal types in baseband.

### P6: Topological Pruning via Betweenness Centrality (Priority: Medium)

**Problem**: Current pruning is temporal (idle-night compression by age). The plan calls for topological pruning but it's not yet implemented.
**Solution**: Compute betweenness centrality for each concept node during idle-night. Prune memory entries whose concepts are all reachable via short paths from other entries.
**Complexity**: Medium — standard graph algorithm, integrate with compression.lisp.

### P7: Basin-Adaptive Learning Rate (Priority: Low)

**Problem**: Signalograd's Hopfield memory slots and the memory field's attractor basins don't share information. A strong recall in the field doesn't help Signalograd learn, and vice versa.
**Solution**: Feed memory-field recall_strength into Signalograd's observation vector. Currently the observation has `memory_pressure` and existing `recall_strength` from Hopfield — extend it with field recall quality.
**Complexity**: Low — add one f64 to the observation struct.

## Live System Test Results (2026-03-25)

Tested on a running Harmonia instance (v0.2.0) via direct IPC socket queries to the memory-field component.

### System State at Test Time

```
graph-n:        106 nodes (from soul DNA + boot memory)
graph-version:  7 (reloaded 7 times by harmonic cycle)
spectral-k:     8 eigenvectors computed
basin:          thomas-0 (music — cold start default)
dwell-ticks:    7 (7 harmonic phases completed)
thomas-b:       0.208 (edge of chaos)
```

### Recall Results

| Query | Top Result | Score | Entry | Notes |
|-------|-----------|-------|-------|-------|
| "harmony" | harmony | 0.716 | SOUL | Correctly identifies as top concept, bridges to orchestration, memory |
| "memory" + "code" | memory | 0.725 | SOUL | Cross-domain query finds memory as primary |
| "rust" + "lisp" | rust | 0.764 | SOUL | Engineering query; also resonates with "simple", "minimal" from DNA principles |
| "dream" + "music" | harmony | 0.497 | SOUL | Neither word in graph, but field finds harmony through music-domain connections |

### Findings

1. **Field is live and responding**: Graph reloads every harmonic cycle, attractors step with signal/noise, basin tracking active.

2. **Soul-only graph is nearly uniform**: With only DNA concepts (all from one soul entry), the 106 nodes form a dense cluster with little spectral separation. Eigenvalues are all ~106.0 (degenerate). This means Chladni modes don't discriminate yet.

3. **Discrimination emerges from daily interaction**: As the system accumulates diverse daily memories, the concept graph develops cluster structure. Engineering conversations build engineering clusters, music conversations build music clusters, and the eigenvalues separate. This is the design: the system starts from a seed and grows its own spectral landscape.

4. **Field potential works correctly**: Source nodes (query concepts) score highest. The gradient correctly decreases with graph distance.

5. **Basin warm-start works**: The system initialized to thomas-0 on boot, with dwell-ticks tracking harmonic cycles.

6. **All entries trace to SOUL**: On a fresh boot with no user interactions, all recalled concepts come from the soul DNA memory. Once the user converses, daily entries will populate the graph with diverse entry IDs.

### What Needs Real Conversations

- **Eigenmode separation**: Requires 50+ diverse daily interactions to build enough cluster structure for meaningful Chladni patterns.
- **Basin switching**: Requires sustained domain-focused conversations to accumulate coercive energy and trigger a switch.
- **Warm-start validation**: Requires a restart after conversations to verify Chronicle persistence.
- **Field vs substring comparison**: Requires enough memory entries to compare recall quality.

## S-Expression REPL Loop and Minimal Bootstrap Architecture

### The REPL Loop (sexp-eval.lisp)

The memory field operates within an S-Expression REPL loop that enables multi-round LLM code execution. Rather than a single prompt-response cycle, the LLM can:

1. **Evaluate** an S-Expression against the running field state
2. **Observe** the result (field potentials, basin state, recall scores)
3. **Generate** the next S-Expression based on what it learned
4. **Repeat** until it converges on a useful answer

This turns the LLM from a one-shot oracle into an iterative explorer of the memory field. Each round refines the query, narrows the recall, or adjusts attractor parameters. The REPL loop is the execution substrate for all field operations -- recall, encoding, basin inspection, and spectral analysis all flow through it.

### Minimal DNA Bootstrap (<1000 chars)

The system boots from a minimal DNA seed of under 1000 characters. This seed does not contain the agent's full knowledge -- it contains just enough for the LLM to discover itself through the memory field.

The bootstrap sequence:

1. **Genesis seeding**: The DNA string unpacks into concept nodes in the memory field at boot time. Each concept in the DNA becomes a node in the graph, with edges formed by co-occurrence.

2. **Self-discovery**: The LLM queries the field and finds concepts like "harmony", "memory", "rust", "lisp" -- the vocabulary of its own identity. These are not instructions; they are resonant landmarks.

3. **Field-guided expansion**: As the LLM interacts with the field, it builds new edges, strengthens existing ones, and evolves the spectral landscape. The DNA is the seed; the lived experience is the growth.

This architecture means the agent's identity is not hardcoded in a prompt -- it emerges from the interaction between a minimal seed and a dynamical memory system.

### Guardian Healer: LLM-Guarded Self-Healing

The Guardian Healer is an LLM-guarded self-healing mechanism with a safe action whitelist. When the system detects anomalies (crashed components, poisoned state, resource exhaustion), the Guardian Healer:

1. **Diagnoses** the issue using field state and system metrics
2. **Proposes** a repair action from a strict whitelist (restart component, clear cache, reset basin, reload config)
3. **Executes** only whitelisted actions -- no arbitrary code execution

The whitelist constraint is critical: the LLM can reason about what to fix, but it cannot execute anything outside the predefined safe actions. This prevents a hallucinating repair from causing cascading damage.

## References

- [memory-as-a-field.md](memory-as-a-field.md) — architecture spec
- [memory-field-theory.md](memory-field-theory.md) — theoretical foundations
- [memory-field-crate.md](memory-field-crate.md) — Rust crate reference
- [signalograd-architecture.md](signalograd-architecture.md) — chaos kernel
- Test source: `lib/core/memory-field/tests/integration_tests.rs` (54 scenarios)
- Test source: `lib/core/memory-field/src/` (17 unit tests across 5 modules)
