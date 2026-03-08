# Harmonia Boot Knowledge

This directory is the **agent-facing canonical knowledge base** for Harmonia.

It is split into two stable knowledge domains:

- `genesis/`: foundational identity, architecture, and constraints.
- `evolution/`: current operating state, scored changes, and rewrite direction.

Design intent:

- Keep the most important long-lived concepts close to runtime source.
- Keep naming clean, stable, and predictable.
- Make this corpus usable both by humans and by automated prompt/bootstrap loaders.

Reference docs for developers live in `harmonia/doc/`, and they point back to this boot corpus.
