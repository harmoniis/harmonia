# Evolution Knowledge

Evolution documents describe how Harmonia changes while preserving genesis constraints.

This directory is the runtime-adjacent memory of:

- what changed,
- why it changed,
- how quality is measured,
- and what is next.

## Layout

- `latest/` — mutable current evolution snapshot (actively updated).
- `versions/vN/` — immutable historical snapshots.
- `version.sexp` — current numeric version read at boot.
- `signalograd.sexp` may appear in `latest/` and `versions/vN/` as the compact adaptive model checkpoint artifact.

## Reading Order (Latest)

1. `latest/current-state.md`
2. `latest/scorecard.md`
3. `latest/changelog.md`
4. `latest/rewrite-roadmap.md`

## Snapshot Rule

Every successful evolution step snapshots `latest/` into `versions/vN` and bumps `version.sexp`.
On next boot, runtime loads the tracked version metadata, restores the matching `signalograd` checkpoint when present, and keeps past versions available for reflection.
