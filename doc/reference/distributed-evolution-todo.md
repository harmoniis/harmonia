# Distributed Evolution TODO

## Purpose

Define a deterministic publish/subscribe algorithm for organization-wide distributed evolution so local improvements are accepted only when globally harmonic.

Current runtime supports:
- local `:artifact-rollout` and `:source-rewrite` modes,
- distributed participation flags (`HARMONIA_DISTRIBUTED_*`),
- S3-backed store identity (`s3`) metadata.

This document defines what remains to implement.

## Required Algorithm (Target)

1. Proposal publish:
   - Agent emits signed `evolution-proposal` artifact to shared store.
   - Artifact includes:
     - proposal id, parent version/hash, agent id,
     - mode (`artifact-rollout` or `source-rewrite`),
     - patch/binary digest,
     - local scorecard (harmony/noise/latency/cost/success),
     - policy fingerprint.
2. Subscriber validation:
   - Peers pull new proposals and run deterministic validation gates:
     - signature/integrity,
     - reproducibility check,
     - policy compatibility check,
     - invariant guards.
3. Harmonic consensus:
   - Peers publish signed `accept|degrade|deny` votes with weighted reasons.
   - Weights are policy-driven (role, reliability history, topology trust).
4. Adoption decision:
   - Proposal is accepted only if aggregate weighted harmonic score exceeds threshold.
   - Dissonant local wins are downgraded (kept local, not propagated globally).
5. Rollback propagation:
   - If post-adoption telemetry breaches safety envelope, emit rollback signal and quarantine proposal lineage.

## Storage Contract (S3-Sync)

Proposed keyspace:
- `org/<org-id>/evolution/proposals/<proposal-id>.json`
- `org/<org-id>/evolution/votes/<proposal-id>/<agent-id>.json`
- `org/<org-id>/evolution/decisions/<proposal-id>.json`
- `org/<org-id>/evolution/rollbacks/<proposal-id>.json`

Requirements:
- monotonic timestamps,
- immutable proposal/vote objects,
- append-only decision history,
- explicit retention + archival policy.

## Policy Surface (to add)

`harmony-policy.sexp` needs a dedicated `:distributed-evolution` section:
- `:enabled`
- `:store-kind`
- `:store-bucket`
- `:store-prefix`
- `:quorum-min`
- `:accept-threshold`
- `:degrade-threshold`
- `:max-local-divergence`
- `:rollback-on-dissonance-threshold`

## Implementation Work Items

1. Add Rust crate for signed proposal/vote schemas and verification.
2. Extend `s3` with atomic publish/list/claim primitives.
3. Add Lisp port for distributed evolution orchestrator state machine.
4. Add deterministic scoring/aggregation module shared by all nodes.
5. Add recovery integration for distributed rollback events.
6. Add audit report generation (daily digest + per-proposal traceability).
7. Add chaos tests for split-brain, replay, and stale proposal handling.
