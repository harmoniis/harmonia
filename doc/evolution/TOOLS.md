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
- `/tmp/harmonia/model-policy.sexp` — persisted mutable model policy state
- `/tmp/harmonia/matrix-topology.sexp` — persisted mutable matrix topology state
- `/tmp/harmonia/parallel-policy.sexp` — persisted mutable subagent policy state
- `/tmp/harmonia/harmony-policy.sexp` — persisted mutable harmony policy state
