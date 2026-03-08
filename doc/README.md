# Harmonia Documentation

This directory contains developer-facing documentation for the `harmonia/` project.

## Canonical Sources

Two canonical corpora exist and both matter:

1. Runtime-adjacent corpus in this project:
- `src/boot/genesis/`
- `src/boot/evolution/`

2. Full long-form corpus in the parent workspace:
- `../../doc/agent/genesis/`
- `../../doc/agent/evolution/`

The long-form `doc/agent/*` corpus carries broader concept coverage (including UI/UX, A2UI, swarm protocols, and architecture narrative). The `src/boot/*` corpus is the concise runtime-near subset.

## Reference Atlas

Use `doc/reference/` as the structured map across both corpora.

Start with:

- `reference/README.md`
- `reference/system-map.md`
- `reference/migration-map.md`

The migration map is mandatory for coverage auditing: every canonical source topic must be represented there.

## Security Documentation

The security architecture is documented across multiple levels:

- **Reference**: `reference/security-architecture.md` — comprehensive security architecture reference.
- **Boot genesis**: `src/boot/genesis/runtime-architecture.md` — security architecture section, typed signal dispatch, policy gate in orchestration flow.
- **Boot genesis**: `src/boot/genesis/concepts.md` — security kernel and adaptive shell concepts.
- **Boot evolution**: `src/boot/evolution/latest/changelog.md` — v6 SignalGuard changelog entry.
- **Boot evolution**: `src/boot/evolution/latest/current-state.md` — security kernel runtime state.
- **Operations**: `reference/operations-runbook.md` — security verification procedures.
