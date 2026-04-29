#!/usr/bin/env bash
# Run the closed-loop wiring tests. No LLM, no IPC, no Rust runtime.
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

sbcl --noinform --non-interactive --disable-debugger \
  --eval "(load #P\"src/core/boot.lisp\")" \
  --eval "(load #P\"tests/test-closed-loops.lisp\")" \
  --eval "(multiple-value-bind (pass fail) (harmonia::run-closed-loop-tests) (sb-ext:exit :code (if (zerop fail) 0 1)))"
