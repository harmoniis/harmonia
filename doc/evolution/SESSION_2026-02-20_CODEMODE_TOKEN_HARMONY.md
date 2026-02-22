# Session 2026-02-20: CodeMode + Token Harmony Metrics

## Why

Harmony scoring and model policy were over-focused on completion/speed/price.
This session adds explicit token-minimization and code-mode orchestration quality as first-class weighted dimensions.

## Implemented

- `src/harmony/scorer.lisp`
  - `harmonic-score` now accepts `:context`.
  - Added token-efficiency score (`response_tokens / total_tokens`).
  - Added code-mode efficiency score:
    - chain ratio (`tool-calls / llm-calls`)
    - relay budget penalty (`intermediate_tokens / final_tokens`)
    - datasource fan-in signal.
  - All weights are runtime-tunable from `harmony-policy`.

- `config/harmony-policy.sexp`
  - Added `:score` namespace:
    - `:base-weight`
    - `:token-efficiency-weight`
    - `:codemode-efficiency-weight`
    - `:codemode-relay-budget`
    - `:codemode-chain-weight`
    - `:codemode-relay-weight`
    - `:codemode-sources-weight`

- `src/core/model-policy.lisp`
  - Added model-selection metrics:
    - `:token-efficiency`
    - `:orchestration-efficiency`
  - Added task kind `:codemode`.
  - Scoring now rewards profiles tagged for token-lean orchestration (`:token-efficient`, `:codemode`, `:tool-use`).

- `config/model-policy.sexp`
  - Weights include token/orchestration efficiency.
  - Profiles annotated with orchestration/token tags where appropriate.

- `src/orchestrator/conductor.lisp`
  - Added `tool op=codemode-run`:
    - `steps=op:key=value,key2=value|op2:key=value`
    - runs multi-step tool pipelines locally in one turn.
    - avoids LLM relay between intermediate tool calls.
  - Added orchestration context capture:
    - `:mode`, `:llm-calls`, `:tool-calls`, `:datasource-count`, `:intermediate-tokens`
  - Context is fed into `harmonic-score` and runtime logs.

- `src/memory/store/state.lisp`
  - `memory-record-orchestration` now stores `:harmony` context alongside prompt/response.

## Runtime control

- Tune model weights live:
  - `tool op=model-policy-set-weight metric=token-efficiency value=0.18`
  - `tool op=model-policy-set-weight metric=orchestration-efficiency value=0.14`

- Tune score weights live:
  - `tool op=harmony-policy-set path=score/token-efficiency-weight value=0.40`
  - `tool op=harmony-policy-set path=score/codemode-efficiency-weight value=0.30`

## DNA-backed system constitution

- `src/dna/dna.lisp` now embeds:
  - creator lineage
  - evolution purpose
  - cosmic-view / all-life ethic
  - Vitruvian core constraints
- `dna-compose-llm-prompt` injects this constitution into:
  - orchestrator OpenRouter calls (`mode :orchestrate`)
  - model planner calls (`mode :planner`)
- Soul memory seeding now carries purpose + ethic fields so rewrites remain anchored.
