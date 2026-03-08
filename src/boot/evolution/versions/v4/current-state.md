# Current State

Snapshot date: 2026-03-05

## Active Evolution Mode

Default mode is `:source-rewrite` (Ouroboros-backed patch flow).

From `src/ports/evolution.lisp`:

- `evolution-prepare` inspects health and crash state.
- `evolution-execute` writes patch artifacts in source-rewrite mode.
- `evolution-rollback` records rollback as crash telemetry.

## Runtime Readiness Signals

Rewrite candidate readiness combines:

- harmonic convergence (global/local + lambdoma ratio),
- logistic chaos risk thresholds,
- vitruvian signal/noise gates.

Primary thresholds come from `config/harmony-policy.sexp`:

- `rewrite-plan/signal-min`
- `rewrite-plan/noise-max`
- `rewrite-plan/chaos-max`

## Model/Swarm Policy State

Model selection is task-aware and can prefer local CLI agents for software-dev prompts.

Policy inputs:

- `config/model-policy.sexp`
- `config/swarm.sexp`
- mutable state files under `HARMONIA_STATE_ROOT`.

## Memory Evolution State

Memory pipeline is active with four layers:

- Soul seeding from DNA,
- Daily interaction memory,
- Skill compression and crystallization,
- Temporal journaling (yesterday summary).

Compression and crystal thresholds are policy-controlled (`:memory` section in harmony policy).

## Matrix Enforcement State

All critical orchestrator routes are matrix-gated before invocation.

Matrix topology source of truth:

- seed: `config/matrix-topology.sexp`
- mutable state: `${HARMONIA_STATE_ROOT}/matrix-topology.sexp`
