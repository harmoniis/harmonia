# Harmonia Documentation

This directory contains developer-facing documentation for the `harmonia/` project.

## Canonical Sources

Two canonical layers exist:

1. Agent-facing (loaded at boot):
- `src/boot/genesis/*.sexp` — immutable identity and architecture
- `src/boot/evolution/latest/*.sexp` — current evolution state

2. Developer-facing (this directory):
- `doc/genesis/*.md` — markdown mirrors of boot genesis
- `doc/evolution/*.md` — markdown mirrors of boot evolution
- `doc/reference/` — structured reference atlas

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
- **Boot genesis**: `src/boot/genesis/runtime-architecture.sexp` — security architecture section, typed signal dispatch, policy gate in orchestration flow.
- **Boot genesis**: `src/boot/genesis/concepts.sexp` — security kernel and adaptive shell concepts.
- **Boot evolution**: `src/boot/evolution/latest/changelog.sexp` — v6 SignalGuard changelog entry.
- **Boot evolution**: `src/boot/evolution/latest/current-state.sexp` — security kernel runtime state.
- **Operations**: `reference/operations-runbook.md` — security verification procedures.
