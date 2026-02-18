# Tool Registry State

Current state of loaded .so tools. Updated by the agent.

## Core Tools (Always Loaded)

| Tool | Version | Call Frequency | Status |
|------|---------|---------------|--------|
| phoenix | v0.1.0 | boot-time | Compiled |
| ouroboros | v0.1.0 | health/repair path | Compiled |
| vault | v0.1.0 | secret boundary path | Compiled |
| memory | v0.1.0 | memory path | Compiled |
| mqtt-client | v0.1.0 | signaling path | Compiled |
| http | v0.1.0 | tool path | Compiled |
| s3-sync | v0.1.0 | snapshot path | Compiled |
| git-ops | v0.1.0 | dna path | Compiled |
| rust-forge | v0.1.0 | forge path | Compiled |
| cron-scheduler | v0.1.0 | scheduler path | Compiled |
| push-sns | v0.1.0 | push path | Compiled |
| recovery | v0.1.0 | watchdog path | Compiled |
| browser | v0.1.0 | web path | Compiled |
| fs | v0.1.0 | sandboxed I/O path | Compiled |
| parallel-agents | v0.1.0 | subagent orchestration path | Compiled |
| search-exa | v0.1.0 | primary web verification path | Compiled |
| search-brave | v0.1.0 | fallback web verification path | Compiled |
| harmonic-matrix | v0.1.0 | constrained route mesh path | Compiled |
| config-store | v0.1.0 | runtime mutable config KV path | Compiled |

## Backends

| Backend | Version | Status |
|---------|---------|--------|
| openrouter-backend | v0.1.0 | Compiled |

## Optional Plugins

| Plugin | Version | Loaded | Status |
|--------|---------|--------|--------|
| pgp-identity | v0.1.0 | No | Compiled |
| webcash-wallet | v0.1.0 | No | Compiled |
| whatsapp | v0.1.0 | Yes | Compiled |
| telegram | v0.1.0 | Yes | Compiled |
| slack | v0.1.0 | Yes | Compiled |
| mattermost | v0.1.0 | Yes | Compiled |
| nostr | v0.1.0 | Yes | Compiled |
| email-client | v0.1.0 | Yes | Compiled |
| whisper | v0.1.0 | Yes | Compiled |
| elevenlabs | v0.1.0 | Yes | Compiled |

## Forge-Created Tools

None yet. Runtime forge additions are not recorded in this local build snapshot.

## Runtime Policy Files

- `config/tools.sexp` — default tool registry
- `config/model-policy.sexp` — default model harmony policy
- `config/matrix-topology.sexp` — default routing topology
- `config/parallel-policy.sexp` — default subagent fan-out policy
- `config/harmony-policy.sexp` — default harmonic evolution constants/policy
- `${HARMONIA_STATE_ROOT:-/tmp/harmonia}/model-policy.sexp` — persisted mutable model policy state
- `${HARMONIA_STATE_ROOT:-/tmp/harmonia}/matrix-topology.sexp` — persisted mutable matrix topology state
- `${HARMONIA_STATE_ROOT:-/tmp/harmonia}/parallel-policy.sexp` — persisted mutable subagent policy state
- `${HARMONIA_STATE_ROOT:-/tmp/harmonia}/harmony-policy.sexp` — persisted mutable harmony policy state
- `${HARMONIA_CONFIG_DB:-${HARMONIA_STATE_ROOT:-/tmp/harmonia}/config.db}` — runtime mutable non-secret config DB

## Vault Secret Ingest Policy

- `HARMONIA_VAULT_SECRET__<SYMBOL>=<VALUE>`: direct ingest path (`<SYMBOL>` normalizes to lowercase, `__` -> `-`).
- `HARMONIA_VAULT_IMPORT`: dynamic import map from env names to one or more vault symbols.
- Format: `ENV_A=symbol_one|symbol_two,ENV_B=symbol_three`.
- Example:
  - `OPENROUTER_API_KEY=openrouter,EXA_API_KEY=exa_api_key|exa,BRAVE_SEARCH_API_KEY=brave_api_key|brave`
- Vault persistence is DB-backed (`HARMONIA_VAULT_DB`, default `/tmp/harmonia/vault.db`).
- Lisp/C policy boundary:
  - Allowed: set value, check if key exists, list keys.
  - Denied: read secret value over C API.

## Matrix Store Policy

- Runtime-selectable matrix store backends:
  - `memory` (in-memory only)
  - `sqlite` (persistent 4D state: nodes, edges, tools, route samples, events)
  - `graph` (interface contract reserved; explicit runtime error until adapter is implemented)
- Env defaults:
  - `HARMONIA_MATRIX_STORE_KIND=memory|sqlite`
  - `HARMONIA_MATRIX_DB=/tmp/harmonia/harmonic-matrix.db`
- Runtime switch from agent loop:
  - `tool op=matrix-set-store kind=sqlite path=/tmp/harmonia/hmatrix-runtime.db`
- Runtime introspection:
  - `tool op=matrix-get-store`

## Runtime Config Policy

- Runtime mutable non-secret values are stored in SQLite config store.
- Scope/key API:
  - `config-set key=<k> value=<v>`
  - `config-get key=<k>`
  - `config-list`
- Current keys used by core orchestration:
  - `openrouter.default_model`
  - `openrouter.fallback_models`
  - `model.default`
  - `model.safe_fallbacks`
  - `model.planner.model`
  - `model.planner.enabled`
  - `matrix.route.signal_default`
  - `matrix.route.noise_default`
  - `matrix.topology.path`
  - `parallel.policy.path`
  - `parallel.subagent_count`
