# Harmonic Score Trajectory

Tracks the agent's harmonic score over time. Visualizes movement through attractor space.

## Metrics

- **Program Size:** Total S-expression count (Kolmogorov proxy)
- **Tool Consonance:** Ratio simplicity across tool call frequencies
- **Memory Compression:** Encoding efficiency (bits per concept)
- **Self-Similarity:** Pattern consistency across function/module/system levels
- **Rewrite Success Rate:** Fraction of candidate rewrites that pass validation

## Trajectory

| Version | Program Size | Tool Consonance | Memory Compression | Similarity | Rewrite Rate |
|---------|-------------|----------------|-------------------|------------|-------------|
| v0 (genesis) | — | — | — | — | — |
| v1 (bootstrap compile, 2026-02-17) | 4 Lisp files / 4537 bytes | 18/18 compiled crates | schema-v0 active | High (uniform C-ABI health/version) | N/A (no rewrite loop yet) |
