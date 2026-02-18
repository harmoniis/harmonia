# Harmonia — The Lisp Agent

Recursive self-improving Common Lisp agent. Lisp orchestration only; all I/O via Rust `.so` tools loaded through CFFI.

See `doc/agent/HARMONIA.md` for full architecture.

## Structure

```
harmonia/
├── README.md               # This file
├── src/                    # Lisp source
│   ├── core/               # Bootstrap, loop, eval, state, rewrite
│   ├── memory/             # Store, recall, compress
│   ├── harmony/            # Detector, scorer, forbidden
│   ├── orchestrator/       # Conductor, planner, cost, stream
│   ├── tools/              # Registry, CFFI bridge, tool protocol
│   ├── backends/           # Backend loader, model selector
│   └── dna/                # Snapshot, journal, git-sync, merge
├── lib/                    # Decoupled Rust crates — each buildable separately
│   ├── core/               # Essential (phoenix, ouroboros, vault, memory, mqtt, http, s3, git, forge, cron, sns, recovery, browser, fs)
│   ├── backends/           # LLM providers (openrouter-backend)
│   └── tools/              # Optional plugins (pgp-identity, webcash-wallet, social)
├── config/                 # agent.sexp, backends.sexp, tools.sexp
├── tests/
└── scripts/                # bootstrap.sh, run.sh, snapshot.sh
```

## Build

```bash
# Lisp: sbcl --load src/core/boot.lisp --eval '(harmonia:start)'

# Rust — build single component (fast incremental):
cargo build -p harmonia-mqtt-client
cargo build -p harmonia-phoenix
cargo build -p harmonia-vault

# Bazel (from agent root): bazel build //harmonia/lib/...
```
