# Code Harmony: Strength, Utility, Beauty

This document defines the non-negotiable coding standard for Harmonia.

## Canon

1. **Vitruvian Triad**: `firmitas` (strength), `utilitas` (utility), `venustas` (beauty).
2. **SICP standard**: code is written for humans first; machines second.
3. **Unix philosophy**: separate policy from mechanism, keep interfaces clean, prefer data-driven configuration over hardcoded branches.
4. **Zen of Python**: beautiful/explicit/simple/readable are practical engineering constraints.

## Translation To Harmonia

1. **Strength (`firmitas`)**
- Fail loudly with typed errors and restart paths.
- Keep crash recovery and rollback testable.
- Avoid hidden coupling between modules.

2. **Utility (`utilitas`)**
- Expose runtime set/get for policies (matrix topology, model policy, parallel width, vault secrets).
- Keep operations scriptable through `tool op=...` so orchestration can evolve without recompilation.
- Prefer stable interfaces over ad-hoc patches.

3. **Beauty (`venustas`)**
- Minimize hardcoded constants in executable paths.
- Move policy values into declarative config files.
- Make behavior inspectable (`*-report`, `*-get-*`, persisted policy state).

4. **Living Observability (4D Harmony)**
- Harmony cannot be evaluated in darkness; every subsystem must emit input/output/error signals.
- Matrix state is 4D: topology + routing + memory + time/revision history.
- Temporal feedback is required for self-awareness and adaptation.

## Mandatory Rules

1. No hardcoded secrets. Secret ingestion is generic (`HARMONIA_VAULT_SECRET__<SYMBOL>`), and runtime updates use `tool op=vault-set`.
2. No hardcoded routing topology in execution code. Matrix policy must be loaded from config/persistent state and be editable via set/get operations.
3. No hardcoded model scoring table in execution code. Model policy must be loadable/savable and mutable at runtime.
4. No hardcoded operational width for subagent fan-out. Width must be runtime-settable/gettable.
5. Every mutable policy must provide:
- `get`
- `set`
- `save`
- `load`
- observable report
6. Every critical operation must emit matrix event telemetry (`input`, `output`, `error`) so the agent can search history and reason about evolution over time.

## Reference Sources

- Vitruvian triad summary:
  - https://www.britannica.com/topic/architecture/Commodity-firmness-and-delight-the-ultimate-synthesis
  - https://www.lib.uchicago.edu/collex/exhibits/firmness-commodity-and-delight/
- SICP preface principle:
  - https://sicp.sourceacademy.org/chapters/prefaces96.html
- Unix philosophy rules:
  - https://www.catb.org/esr/writings/taoup/html/
- SICP preface (human readability and complexity control):
  - https://sicp.sourceacademy.org/chapters/prefaces96.html
- Zen of Python (official PEP):
  - https://peps.python.org/pep-0020/
