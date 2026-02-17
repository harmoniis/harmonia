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

## Backends

| Backend | Version | Status |
|---------|---------|--------|
| openrouter-backend | v0.1.0 | Compiled |

## Optional Plugins

| Plugin | Version | Loaded | Status |
|--------|---------|--------|--------|
| pgp-identity | v0.1.0 | No | Compiled |
| webcash-wallet | v0.1.0 | No | Compiled |
| social | v0.1.0 | No | Compiled |

## Forge-Created Tools

None yet. Runtime forge additions are not recorded in this local build snapshot.
