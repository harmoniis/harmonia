# Routing Architecture

Intelligent LLM routing with tier-based model selection, complexity encoding, and signalograd-adaptive feedback.

## Overview

```
Signal arrives from any frontend (TUI, MQTT, WhatsApp, Telegram, ...)
    │
    ▼
Gateway constructs ChannelEnvelope
    │
    ├─ ComplexityEncoder scores prompt (14 dimensions, <10μs)
    │  → RoutingContext attached to envelope (stack-allocated, 130 bytes)
    │
    ├─ Command? → /auto /eco /premium /free → config-store write → RouterActor notified
    │            → /route → RouterActor returns status sexp
    │
    └─ Prompt? → passes through to Lisp orchestrator
                    │
                    ▼
                Lisp reads RoutingContext from envelope
                    │
                    ├─ %load-routing-tier → reads active tier from config-store
                    ├─ %tier-model-pool → filters profiles by tier
                    ├─ %auto-tier-pool-from-routing-context → maps encoder tier to model sub-pool
                    ├─ %seed-score-with-bias → applies tier weight bias + signalograd deltas
                    └─ choose-model → returns best model for prompt + tier
                        │
                        ▼
                    Provider Router dispatches to native backend or OpenRouter
```

## Routing Tiers

Users switch tiers via slash commands from **any authenticated frontend** (TUI, MQTT, WhatsApp, Telegram, Signal, etc.). Requires `Owner` or `Authenticated` security label.

| Command | Tier | Behaviour | Model Pool |
|---------|------|-----------|------------|
| `/auto` | Intelligent (default) | Encoder classifies complexity → maps to sub-pool | All models; simple→eco pool, complex→premium pool |
| `/eco` | Cost-optimized | Cheapest viable model for the task | `:micro` and `:lite` profiles (cost ≤ 2) |
| `/premium` | Quality-optimized | Best models regardless of cost | `:pro`, `:frontier`, `:fast-smart`, `:thinking` (quality ≥ 7) |
| `/free` | Zero-cost | Local CLI tools only (claude-code, codex) | No LLM API calls |
| `/route` | (status) | Display current tier, pool, stats, signalograd deltas | — |

Tier persists across sessions via config-store key `router/router/active-tier`.

## Complexity Encoder

**Crate**: `lib/core/complexity-encoder/`

Fast, rule-based prompt classifier inspired by ClawRouter's 14-dimension scoring. Runs in the gateway baseband layer — every non-command signal gets a `RoutingContext` before reaching Lisp.

### 14 Dimensions

| # | Dimension | Weight | Description |
|---|-----------|--------|-------------|
| 0 | Token count | 0.08 | Length-based: <30 chars = -0.8, >2000 = 1.0 |
| 1 | Code presence | 0.12 | Backtick blocks, language keywords (function, struct, impl) |
| 2 | Reasoning markers | 0.14 | prove, theorem, analyze, deduce, chain of thought |
| 3 | Technical terms | 0.08 | algorithm, distributed, consensus, mutex, transformer |
| 4 | Creative markers | 0.04 | write a story, compose, brainstorm, narrative |
| 5 | Simple indicators | 0.06 | hello, thanks, yes/no — **negative** dimension (reduces score) |
| 6 | Multi-step patterns | 0.10 | first/then/next/finally, step 1/2/3, phase markers |
| 7 | Question complexity | 0.06 | Question marks, wh-words density |
| 8 | Imperative verbs | 0.06 | implement, refactor, deploy, build, optimize |
| 9 | Constraint indicators | 0.08 | must, ensure, thread-safe, idempotent, backward compat |
| 10 | Output format | 0.04 | json, csv, markdown, table, schema |
| 11 | Reference complexity | 0.04 | citation, RFC, specification, peer-reviewed |
| 12 | Negation complexity | 0.04 | not, without, unless, except, avoid |
| 13 | Domain specificity | 0.06 | medical, quantum, blockchain, cryptograph, aerospace |

### Classification

```
Score = weighted_sum(dimensions) normalized to [0.0, 1.0]

Simple    : score < 0.25
Medium    : 0.25 ≤ score < 0.50
Complex   : 0.50 ≤ score < 0.75
Reasoning : score ≥ 0.75
```

### Special Overrides

- **2+ formal reasoning markers** (prove, theorem, formal verification, derive, deduce, chain of thought) → **force Reasoning** at 0.97 confidence
- **Code block + technical terms** (dim[1] > 0.5 and dim[3] > 0.3) → escalate one tier
- **Simple indicators only + length < 50** → force Simple at 0.95 confidence

### Performance

- **7μs per call** in release (SIMD-optimized `str::contains`)
- Single allocation: one `to_ascii_lowercase()` buffer
- All keyword sets: compile-time `&[&str]` in `.rodata`
- `RoutingContext`: fully stack-allocated (`Copy` trait), 130 bytes, zero heap

### FFI

```c
// Score a prompt, returns s-expression string (caller frees)
char* harmonia_complexity_encoder_score(const char* text);
// Returns: (:tier "complex" :score 0.6700 :confidence 0.8900 :dimensions (0.1 0.8 ...))

void  harmonia_complexity_encoder_free_string(char* ptr);
int   harmonia_complexity_encoder_healthcheck();  // returns 1
```

## RoutingContext on ChannelEnvelope

Every non-command signal carries a `RoutingContext` computed by the encoder:

```rust
#[derive(Copy, Clone)]  // fully stack-allocated
pub struct RoutingContext {
    pub tier: ComplexityTier,          // 1 byte enum
    pub score: f64,                     // aggregate [0, 1]
    pub confidence: f64,                // [0.5, 1.0]
    pub active_tier: UserTier,          // 1 byte enum
    pub dimensions: [f64; 14],          // 112 bytes, no indirection
}
```

S-expression in envelope:
```lisp
(:routing (:tier "complex" :score 0.6500 :confidence 0.8200 :active-tier "auto" :dimensions (0.1 0.8 0.3 ...)))
```

Commands (`/auto`, `/eco`, etc.) have `:routing nil` — they are intercepted by the gateway and never reach Lisp.

## RouterActor

**Location**: `lib/core/runtime/src/actors.rs`

Async ractor actor owning routing state. Registered with supervisor, receives ticks every 5s.

### State (zero-heap)

```rust
pub struct RouterState {
    active_tier_idx: u8,                              // 1 byte
    tier_stats: [TierStats; 4],                       // 128 bytes, indexed by tier
    history: [Option<RouteHistoryEntry>; 32],          // ring buffer, fixed capacity
    cascade_entries: [Option<CascadeEntry>; 4],        // bounded cascade slots
}
```

- **TierStats**: requests, successes, total_cost_usd, total_latency_ms per tier
- **RouteHistoryEntry**: inline byte arrays for model_id (48B cap) and task_kind (24B cap) — no String heap allocation
- **Ring buffer**: oldest entry overwritten, O(1) insert

### Messages

| Message | Action |
|---------|--------|
| `Tick` | Sync tier from config-store + expire stale cascade entries (>30s) |
| `Signal(tier-changed)` | Update `active_tier_idx` |
| `Signal(route-feedback)` | Record model/task/success/latency/cost in tier stats + history |
| `Dispatch` | Return status sexp for `/route` command |

## Actor Protocol

`ActorKind::Router` added to the protocol with three `MessagePayload` variants:

```rust
TierChanged { tier: String }
RouteFeedback { request_id, model_id, task_kind, tier, success, latency_ms, cost_usd_estimate, complexity_score }
CascadeEscalate { request_id, failed_model, reason }
```

## Lisp Tier-Aware Model Selection

**File**: `src/core/model-policy.lisp`

### Key Functions

| Function | Purpose |
|----------|---------|
| `%load-routing-tier` | Read active tier from config-store → `*routing-tier*` |
| `%tier-model-pool` | Filter `*model-profiles*` by tier criteria |
| `%tier-weight-bias` | Additive bias per tier: eco boosts :price, premium boosts :completion |
| `%auto-tier-pool-from-routing-context` | Map encoder tier to model sub-pool for auto mode |
| `%seed-score-with-bias` | `%seed-score` variant with tier bias before signalograd modulation |
| `%score-and-rank-within-tier` | Re-rank filtered models using biased weights |
| `%selection-chain-tiered` | Wrap `%selection-chain` with tier filtering |
| `choose-model` | Entry point — accepts optional `routing-ctx` from envelope |

### Weight Bias by Tier

| Tier | :price | :speed | :completion | :correctness |
|------|--------|--------|-------------|-------------|
| eco | +0.15 | +0.10 | -0.05 | — |
| premium | -0.12 | — | +0.10 | +0.08 |
| free | +0.30 | +0.05 | — | — |
| auto | (unbiased) | — | — | — |

Signalograd routing deltas (`routing_price_delta`, `routing_speed_delta`, etc.) modulate on top of these biases — the kernel can override tier preferences when system state demands it.

### Auto Tier Complexity Mapping

| Encoder Output | Model Sub-Pool |
|---------------|----------------|
| Simple | Eco pool (micro/lite models) |
| Medium | Full pool (all models) |
| Complex | Premium pool (pro/frontier) |
| Reasoning | Premium pool (pro/frontier) |

## Self-Rewriting Routing Rules

**Data**: `*routing-rules-sexp*` in `model-policy.lisp`

```lisp
(:version 1
 :task-tier-hints ((:task :memory-ops :preferred-tier :eco)
                   (:task :critical-reasoning :preferred-tier :premium))
 :model-bans ()
 :model-boosts ()
 :cascade-config (:max-escalations 3 :confidence-threshold 0.7))
```

The harmonic machine's `:stabilize` phase calls `%maybe-rewrite-routing-rules` which:
- Bans models with <50% success rate after 5+ samples
- Persists changes via `model-policy-save`
- Frequency controlled by signalograd's `evolution_aggression_bias`

## Actor Message Flow

### Tier Change (/eco from WhatsApp)
```
WhatsApp frontend → Gateway intercept_commands
  → execute_tier_change("eco")
    → config-store write: router/router/active-tier = "eco"
    → response: "[system] Routing tier: eco"
  → RouterActor Tick (every 5s)
    → reads config-store → updates active_tier_idx
```

Config-store is the source of truth. RouterActor syncs on tick — no direct messaging needed. This means tier changes from any source (CLI edit, API, any frontend) are picked up automatically.

### Route Feedback (after LLM call)
```
Lisp model-policy-record-outcome
  → writes swarm_model_scores.sexp (persistence)
  → IPC call to RouterActor: (:route-feedback :model "..." :tier "..." :success t ...)
    → RouterActor.handle(Signal)
      → state.record_feedback() → updates tier_stats + ring buffer history
```

### /route Status Query
```
Any frontend → Gateway intercept_commands → /route delegated
  → IPC dispatch to "router" component
    → RouterActor.handle(Dispatch) → returns status_sexp()
      → active tier, history count, per-tier success rates, last 5 routes
```

## Signalograd Integration

### Existing (unchanged)

- `signalograd-routing-weight` modulates price/speed/success/reasoning weights
- Kernel produces `routing_price_delta`, `routing_speed_delta`, `routing_success_delta`, `routing_reasoning_delta`, `routing_vitruvian_min_delta`
- `%task-weights` consumes these deltas per request

### New Observation

`:route-tier` field added to signalograd observation — feeds into the kernel's routing head, letting it learn tier-specific patterns over time.

## Config Store Keys

| Component | Scope | Key | Default | Description |
|-----------|-------|-----|---------|-------------|
| router | router | active-tier | "auto" | Current routing tier |
| router | router | cascade-max-escalations | "3" | Max cascade attempts |
| router | router | eco-budget-usd | "0.001" | Eco per-request budget ceiling |

## Security

- Routing commands require `Owner` or `Authenticated` security label
- Anonymous and Untrusted frontends receive "Permission denied"
- Only `/exit` is TUI-restricted
- Tier state is per-agent (global), not per-frontend or per-conversation

## File Index

| File | Layer | What |
|------|-------|------|
| `lib/core/complexity-encoder/src/scorer.rs` | Rust | 14-dimension scoring engine |
| `lib/core/complexity-encoder/src/dimensions.rs` | Rust | Individual dimension scorers |
| `lib/core/complexity-encoder/src/keywords.rs` | Rust | Static keyword sets |
| `lib/core/complexity-encoder/src/tier.rs` | Rust | ComplexityTier enum + ComplexityProfile |
| `lib/core/complexity-encoder/src/lib.rs` | Rust | Public API + FFI + sexp formatting |
| `lib/core/baseband-channel-protocol/src/lib.rs` | Rust | RoutingContext, ComplexityTier, UserTier |
| `lib/core/gateway/src/baseband.rs` | Rust | Encoder call + RoutingContext on envelope |
| `lib/core/gateway/src/command_dispatch.rs` | Rust | /auto /eco /premium /free /route |
| `lib/core/actor-protocol/src/lib.rs` | Rust | Router ActorKind + message payloads |
| `lib/core/runtime/src/actors.rs` | Rust | RouterActor + optimized state |
| `lib/core/signalograd/src/model.rs` | Rust | route_tier observation field |
| `src/core/model-policy.lisp` | Lisp | Tier pools, weight bias, tiered selection |
| `src/core/system-commands.lisp` | Lisp | /route status command |
| `src/core/harmonic-machine.lisp` | Lisp | Routing rules self-rewrite |
| `src/core/signalograd.lisp` | Lisp | Tier observation in signalograd |
| `config/model-policy.sexp` | Config | Model profiles + routing rules |
