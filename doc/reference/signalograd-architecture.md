# Signalograd Architecture

## Summary

`signalograd` is not a deep neural network in the usual backpropagation sense.

It is a tiny chaos-computing microframework embedded into Harmonia as an adaptive advisory kernel:

- temporal computation comes from a Lorenz-style chaotic reservoir / plastic CTRNN regime
- associative recall comes from a Hopfield-like attractor memory
- outputs are small bounded proposal heads, not unconstrained actions
- learning is local and online, not gradient descent
- persistence is two-tier: live working state plus evolution checkpoints

Its job is to accumulate compact operational knowledge from system telemetry and emit bounded soft weights that Lisp can interpret. It is not a second sovereign runtime.

## What Signalograd Is

- A telemetry-first nano learning framework.
- A source of adaptive overlays for harmony, routing, memory, evolution, and the adaptive security shell.
- A Rust kernel for numeric state updates plus persistence.
- A Lisp-interpreted advisory layer whose outputs remain clamped by deterministic policy.

## What Signalograd Is Not

- Not a transformer.
- Not a raw-text model.
- Not a backprop-trained deep net.
- Not a replacement for provider LLMs.
- Not allowed to mutate hard policy, matrix boundaries, or privileged security invariants directly.

Existing LLM backends remain downstream reasoning models. `signalograd` only biases how the system prepares, routes, remembers, and stabilizes.

## Core Model

`signalograd` is implemented as a 3-part micro-model:

1. `Chaotic reservoir`
- 32 continuous latent units.
- Driven by harmonic/runtime telemetry.
- Tuned near the edge of chaos rather than for stable fixed-point behavior.
- Lorenz-style dynamics are part of the temporal basis, not just decorative metadata.

2. `Attractor memory`
- 32 Hopfield-like memory slots.
- Stores compressed stable reservoir snapshots from successful or rewarded states.
- Recall is similarity-based, so past good operating modes can pull the current latent state toward a known attractor.

3. `Readout heads`
- Tiny bounded heads produce proposals for:
  - harmony
  - routing
  - memory
  - evolution
  - security shell

The evolution head currently influences rewrite and aggression deltas rather than being exposed as a standalone `:evolution` proposal section.

In the current Rust kernel these live in [lib.rs](/Users/george/workspace/harmoniis/agent/harmonia/lib/core/signalograd/src/lib.rs#L1).

## Learning Discipline

Learning is local only.

Allowed update styles:

- Hebbian reinforcement
- Oja-style normalization
- anti-Hebbian decorrelation where useful
- homeostatic normalization
- decay / forgetting
- eligibility-style temporal traces

Disallowed as the main training mechanism:

- gradient descent
- backpropagation through time
- global loss optimization over raw prompts

This keeps the kernel small, CPU-cheap, online, and structurally compatible with deterministic auditing.

## Telemetry-First Inputs

`signalograd` does not ingest raw prompt text in v1.

It learns from compact operational telemetry such as:

- harmonic state
- vitruvian signal/noise
- logistic chaos risk and rewrite aggression
- Lorenz boundedness
- lambdoma ratio
- actor load and stall pressure
- queue depth
- route success / latency / cost pressure
- memory pressure
- graph density and interdisciplinary structure
- security posture and security events
- supervisor error counters
- prior applied confidence from the last projection

The Lisp reflection layer assembles these observations in [signalograd.lisp](/Users/george/workspace/harmoniis/agent/harmonia/src/core/signalograd.lisp#L275).

If text-derived context is ever needed later, it should arrive only as a separate compact embedding produced elsewhere, not as raw text input to `signalograd`.

## Actor-Model Integration

All runtime interaction with `signalograd` is actor-shaped.

The Rust kernel registers the `signalograd` actor and posts proposals into the unified mailbox. Lisp never reaches into the kernel state directly during a harmonic cycle.

Current flow:

1. Harmonic machine reaches `:stabilize`.
2. Chronicle snapshots are recorded first.
3. Lisp sends `feedback` for the previous projection if one existed.
4. Lisp sends one new `observe` message built from the just-finished cycle.
5. Rust advances the kernel and posts a `:signalograd-proposal`.
6. The main loop consumes that mailbox signal.
7. Lisp sanitizes and applies the bounded overlay for the next cycle.

This preserves causality and keeps adaptive state transitions auditable.

Relevant runtime files:

- [harmonic-machine.lisp](/Users/george/workspace/harmoniis/agent/harmonia/src/core/harmonic-machine.lisp#L332)
- [loop.lisp](/Users/george/workspace/harmoniis/agent/harmonia/src/core/loop.lisp#L238)
- [signalograd.lisp](/Users/george/workspace/harmoniis/agent/harmonia/src/ports/signalograd.lisp#L46)

## Policy Boundaries

The most important architectural rule is bounded authority.

`signalograd` may:

- emit soft proposal deltas
- accumulate local learned state
- bias adaptive subsystems within policy clamps

`signalograd` may not:

- rewrite deterministic policy directly
- bypass matrix route constraints
- authorize privileged operations
- mutate hard security kernel invariants

Lisp is the semantic boundary. It interprets the proposal, clamps every field, and decides what becomes effective. That boundary lives in [signalograd.lisp](/Users/george/workspace/harmoniis/agent/harmonia/src/core/signalograd.lisp#L85).

This is why auditability is a feature, not a weakness: the kernel can adapt while the organism keeps identity and safety.

## Output Surface

The current bounded proposal surface is:

- `:harmony`
  - signal bias
  - noise bias
  - rewrite signal delta
  - rewrite chaos delta
  - aggression bias
- `:routing`
  - price / speed / success / reasoning weight deltas
  - vitruvian minimum delta
- `:memory`
  - recall limit delta
  - crystal threshold delta
- `:security-shell`
  - dissonance weight delta
  - anomaly threshold delta

These remain advisory overlays. Base policy still comes from `config/*.sexp` plus config-store.

## Persistence Model

Persistence is two-tier by design.

1. `Live working state`
- Path: `${HARMONIA_STATE_ROOT}/signalograd.sexp`
- Purpose: continual online learning during normal runtime
- Config key: `signalograd-core/state-path`
- Env override: `HARMONIA_SIGNALOGRAD_STATE_PATH`

2. `Evolution checkpoint state`
- Path: `src/boot/evolution/latest/signalograd.sexp`
- Accepted versions: `src/boot/evolution/versions/vN/signalograd.sexp`
- Purpose: version-matched adaptive memory that travels with accepted evolution

Boot restore order:

1. restore version-matched checkpoint if present
2. otherwise restore `latest/signalograd.sexp` if present
3. continue live local learning into the working-state file

Legacy compatibility:

- one-time import from `signalograd.json` still exists for migration

## Auditability

Every learned effect must stay inspectable.

Audit surfaces:

- `signalograd-status`
- `signalograd-snapshot`
- checkpoint digest in kernel status
- chronicle `signalograd_events`

Chronicle records:

- `observe`
- `feedback`
- `proposal`
- `checkpoint`
- `restore`

This makes the adaptive layer reconstructable without giving it control over non-auditable hidden behavior.

## Security Relationship

For security, `signalograd` only participates in the adaptive shell.

It may tune soft parameters such as:

- dissonance weighting
- anomaly threshold sensitivity

It may not alter:

- privileged-op allowlists
- deterministic policy gate logic
- taint propagation
- invariant guards

The security kernel remains deterministic and sovereign.

## Natural Constants

Natural constants and harmonic constants may be used as priors, seeds, or scaling hints.

Examples:

- Lorenz parameters
- Feigenbaum-style initialization constants
- harmonic ratios

They are not substitutes for measurement and they are not policy in themselves. The measured telemetry stream remains the real training signal.

## Implementation Map

Primary implementation files:

- Rust kernel: [lib.rs](/Users/george/workspace/harmoniis/agent/harmonia/lib/core/signalograd/src/lib.rs#L1)
- Lisp reflection and policy boundary: [signalograd.lisp](/Users/george/workspace/harmoniis/agent/harmonia/src/core/signalograd.lisp#L1)
- CFFI port: [signalograd.lisp](/Users/george/workspace/harmoniis/agent/harmonia/src/ports/signalograd.lisp#L1)
- Harmonic cycle integration: [harmonic-machine.lisp](/Users/george/workspace/harmoniis/agent/harmonia/src/core/harmonic-machine.lisp#L332)
- Boot restore path: [boot.lisp](/Users/george/workspace/harmoniis/agent/harmonia/src/core/boot.lisp#L289)
- Evolution snapshot integration: [evolution-versioning.lisp](/Users/george/workspace/harmoniis/agent/harmonia/src/core/evolution-versioning.lisp#L216)
- Chronicle audit path: [db.rs](/Users/george/workspace/harmoniis/agent/harmonia/lib/core/chronicle/src/db.rs#L281)

## Current Design Rule

The clean architectural reading is:

- deterministic kernel for identity and safety
- `signalograd` for adaptive epigenetic pressure fields
- Lisp for meaning, interpretation, and bounded application
- Rust for cheap state updates and checkpoint transport

That split is intentional and should be preserved.
