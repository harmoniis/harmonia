# Concepts Glossary

## Architecture And Governance

- `Genesis`: foundational identity, constraints, and architecture intent.
- `Evolution`: controlled adaptation under genesis constraints.
- `Genomic Layer`: long-lived architecture/policy identity.
- `Epigenetic Layer`: mutable runtime expression and tuning.
- `Constitution`: non-negotiable rules for behavior and safety.
- `Vitruvian Triad`: strength, utility, beauty evaluation frame.

## Runtime And Orchestration

- `Conductor`: Lisp orchestration engine that routes prompts and tool ops.
- `Port`: Lisp capability boundary backed by Rust IPC (Unix domain socket).
- `Baseband`: unified signal ingress/egress processor (gateway) with command dispatch.
- `Unified Command Dispatch`: gateway-level interception of ALL /commands from ALL frontends. Native Rust handlers for wallet/identity/help; Lisp callback delegation for runtime-state commands.
- `Router`: LLM completion boundary used by orchestration.
- `Swarm`: parallel subagent system (API tier + tmux CLI tier).
- `GitOps`: Git operations (status, log, diff, commit, push) via IPC actor. Replaces the former `Lineage` stub.
- `Matrix`: route-constraint and telemetry graph for allowed operations.

## Memory And Scoring

- `Memory Depth`: entries at depth 0 are raw interactions (fade fastest), depth 1 are compressed/structural (resist decay), depth 2+ are identity/crystallized (near-permanent). Depth replaces the former class-based filtering (soul/skill/daily/tool labels are kept as tags but no longer used for recall filtering — field topology decides relevance).
- `Temporal Decay`: access scores decay exponentially by recency: `access × exp(-λ × age_hours / protection)`. Structurally important nodes (high centrality, many connections) decay slower — like how you forget a phone number but remember that a poem shaped your character.
- `Dreaming`: field self-maintenance during idle. The field propagates without a query to find its natural skeleton. Betweenness centrality identifies structural vs noise nodes. Nodes below threshold → prune. Nodes above → crystallize (promote depth). Triggered every 30 ticks by heartbeat.
- `Write Filter`: memory-put rejects entries <20 chars or >80% word overlap with existing entries. Prevents noise from entering the field. Depth>0 always stored.
- `Crystallization`: promoting entry depth when dreaming identifies structural importance. Preserves high-signal memory through field topology, not temporal recency.
- `Token Harmony`: efficiency-aware extensions to harmonic scoring.
- `Attractor`: stable dynamics target for runtime evolution.
- `Lambdoma`: harmonic relation matrix used in theory/scoring framing.
- `Memory Field`: potential field on the concept graph where recall is relaxation into attractor basins rather than substring search. See `memory-as-a-field.md`.
- `Graph Laplacian`: L = D - A; the discrete wave equation on the concept graph. Sparse solve produces field potentials identifying relevant memory paths.
- `Field Propagation`: solving L·φ = b on the concept graph to route recall activation from source context nodes to memory nodes via optimal paths (lightning pathfinding).
- `Spectral Decomposition`: eigendecomposition of the graph Laplacian into standing-wave modes (Chladni patterns) for frequency-selective recall.
- `Attractor Basin`: stable region in dynamical state space; memories within the current basin are preferentially recalled. Basin switching requires coercive energy exceeding a hysteresis threshold.
- `Hysteresis`: path-dependent state retention; the system remembers which attractor basin it occupies and resists casual drift between basins.
- `Coercive Threshold`: minimum signal energy required to switch between attractor basins, preventing weak associations from triggering basin transitions. Grows with dwell time.
- `Thomas Attractor`: cyclically symmetric chaotic system (dx=sin(y)-bx, dy=sin(z)-by, dz=sin(x)-bz) with up to 6 coexisting attractors at b≈0.208; models multi-domain memory routing.
- `Aizawa Attractor`: sphere-plus-tube topology (Langford system); models depth recall where shallow memories orbit the surface and crystal memories inhabit the tube.
- `Halvorsen Attractor`: 3-lobed cyclically symmetric propeller attractor; models interdisciplinary bridging where each lobe maps to a domain cluster.
- `Chladni Mode`: eigenfunction of the graph Laplacian — a standing wave pattern on the concept graph. Different signal frequencies excite different modes, giving frequency-selective recall.
- `Context Collapse`: query-as-measurement analogy where a signal collapses the memory superposition into a specific attractor basin, activating resonant memories while leaving others unchanged.
- `Topological Pruning`: Kolmogorov-guided memory compression where entries are pruned by topological redundancy (betweenness centrality, graph community position) rather than temporal age.

## Recovery And Evolution

- `Ouroboros`: self-healing crash ledger and patch writing subsystem. Fully wired as IPC actor (ComponentSlot 11). Records failures, writes patches, maintains crash history. The REPL can call `(ouroboros-history)`, `(ouroboros-crash comp detail)`, `(ouroboros-patch comp body)`.
- `Phoenix`: supervisor lifecycle and restart/rollout guard.
- `DNA Constraints`: hard limits defined in `*dna*` that the REPL reads at runtime. Include max-rounds, chaos-risk ceiling, vitruvian gate thresholds, graph caps. Violating a constraint requires DNA mutation (hard evolution). Epigenetic tuning (config-store, signalograd) works within DNA-defined bounds.
- `DNA Bounds`: ranges within which epigenetic parameters can be tuned (e.g., decay-lambda 0.001..0.1, thomas-b 0.18..0.24). Going outside bounds requires DNA mutation.
- `DNA Genes`: function references in the genome (encode, decode, eval, dream, evolve, crash, commit). Changing a gene changes the agent's behavior.
- `Evolutionary Circuit Breaker`: component failures stored as memory entries in the field. Failure patterns accumulate in attractor basins. Dreaming can detect recurring patterns. The vitruvian gate controls when code evolution is allowed.
- `Betweenness Centrality`: Brandes' algorithm on the concept graph — measures how many shortest paths pass through a node. High centrality = structural bridge = keep. Low centrality = redundant = prune candidate.
- `Artifact Rollout`: evolution mode where binary rollout is signaled.
- `Source Rewrite`: evolution mode where patch artifacts are generated/applied via Ouroboros.
- `Rollback`: explicit recovery path after failed or harmful mutation.
- `Snapshot Versioning`: immutable `versions/vN` plus mutable `latest` model.

## Chronicle And Observability

- `Chronicle`: graph-native knowledge base (`lib/core/chronicle`) storing harmonic snapshots, memory events, delegation decisions, and concept graph decompositions in SQLite. Queryable via complex SQL returning s-expressions.
- `Harmonic Snapshot`: full vitruvian triad + chaos dynamics + lorenz attractor + lambdoma convergence + security posture captured per harmonic cycle.
- `Graph Snapshot`: concept graph from `memory-map-sexp` decomposed into relational `graph_nodes` and `graph_edges` tables for SQL traversal.
- `Graph Traversal`: recursive CTE queries over `graph_edges` enabling N-hop reachability, interdisciplinary bridge detection, and domain distribution analysis.
- `Delegation Log`: record of each model selection decision with task hint, model chosen, backend, cost, latency, token counts, escalation status, and success.
- `Harmony Trajectory`: permanently downsampled 5-minute buckets of harmonic signal evolution — never pruned, negligible storage.
- `Pressure-Aware GC`: intelligent garbage collection that measures DB size and applies proportional pruning — preserving inflection points (high chaos, rewrites, failures) while thinning boring data.
- `Chronicle Query`: `(chronicle-query sql)` — run arbitrary SELECT/WITH SQL against the knowledge base, returning parsed s-expression results. Enables the agent to reason over its own history.

## Channels And UI

- `Frontend`: pluggable communication channel loaded through baseband.
- `Frontend Capabilities`: static feature declarations parsed from `:capabilities` in baseband config at registration. Attached to every signal from that frontend. Used for capabilities-driven dispatch (e.g., A2UI) without hardcoded frontend-name checks.
- `Signal Metadata`: dynamic per-message context emitted by a frontend as a third poll field. Contains device-specific info (platform, device ID, A2UI version, etc.).
- `Signal Enrichment`: the two-layer model where gateway signals carry both static capabilities (from config) and dynamic metadata (from frontend).
- `Tailnet`: tailscale mesh transport layer and inter-node channel substrate.
- `A2UI`: agent-adaptive UI template protocol. Dispatch is capabilities-driven — any frontend declaring `:a2ui` capability gets A2UI treatment.
- `A2UI Catalog`: canonical component definitions in `config/a2ui-catalog.sexp` (21 components). Lazily loaded by conductor and injected into LLM context for A2UI-capable signals.
- `A2UI Text Fallback`: automatic degradation of A2UI component payloads to plain text when sent to non-A2UI frontends.
- `Living Void`: UI/UX philosophy for voice-first adaptive interfaces.
- `Canonical Envelope`: shared message structure used across agent/platform clients.
- `Device Registry`: MQTT frontend's in-memory registry of connected devices with platform info, capabilities, push tokens, and online/offline state.
- `Offline Queue`: per-device message queue in MQTT frontend, flushed on reconnect with push notification for offline delivery.
- `Push Webhook`: HTTP POST-based push notification delivery via `lib/frontends/push` (utility library consumed by mqtt-client).

## Security

- `Security Kernel`: deterministic, non-bypassable layer protecting privileged operations via typed signals, policy gate, and taint propagation.
- `Adaptive Security Shell`: harmonic defense-in-depth layer using dissonance scoring, security-aware routing, and autonomous posture tracking.
- `Policy Gate`: binary allow/deny gate (`%policy-gate`) for 14 privileged operations. Checks originating signal's taint chain and security label. Not based on harmonic scores.
- `Taint Propagation`: tracking of signal origin through the orchestration chain via `*current-originating-signal*`. Taint labels: `:external`, `:tool-output`, `:memory-recall`, `:internal`.
- `Harmonia Signal`: typed struct replacing format-string prompts for external signals. Carries security-label, taint, dissonance, frontend, payload, capabilities, metadata.
- `Security Label`: trust classification of a signal's origin: `:owner`, `:authenticated`, `:anonymous`, `:untrusted`.
- `Dissonance Score`: 0.0-1.0 injection detection score computed at gateway signal parse time. High dissonance attenuates signal in security-aware routing.
- `Boundary Wrapping`: external data wrapped with `=== EXTERNAL DATA [...] ===` markers in prompts, memory recalls, and search results to resist prompt injection.
- `Invariant Guard`: hardcoded non-configurable safety limits (vault min_harmony >= 0.30, dissonance-weight >= 0.05) that cannot be weakened by any configuration or admin intent.
- `Security Posture`: system-wide security state (`:nominal`/`:elevated`/`:alert`) tracked by `:security-audit` phase in harmonic machine.
- `Signal Integrity`: shared crate (`lib/core/signal-integrity`) for injection pattern detection, dissonance scoring, and boundary wrapping.
- `Admin Intent`: Ed25519 signed authorization for privileged mutations. Owner's public key in vault, private key on owner's device.
- `Safe Parser`: `%safe-parse-number` and `%safe-parse-policy-value` — replacements for `read-from-string` that prevent Lisp reader macro attacks.
- `Confused Deputy`: attack where the LLM is tricked (via prompt injection) into proposing privileged actions on behalf of an untrusted signal. Mitigated by taint propagation + policy gate.
- `Vault Symbol`: symbolic handle to a secret value (not raw secret exposure).
- `Scoped Secret Access`: key access constrained to approved call paths.
- `Boundary-First Safety`: policy that sensitive operations are gated at explicit boundaries.
- `Vault Encryption at Rest`: AES-based encryption of stored vault secrets using master key derived from Harmoniis wallet `vault` slot (`HARMONIA_VAULT_MASTER_KEY` is fallback-only).
- `HMAC Authentication`: HMAC-SHA256 message authentication on tailnet mesh messages with 5-minute replay protection window.
- `Fingerprint Validation`: MQTT frontend validates `agent_fp` against vault-stored expected fingerprint; mismatches downgraded to untrusted.
