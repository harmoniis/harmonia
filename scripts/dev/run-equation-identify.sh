#!/usr/bin/env bash
# Run the Phase 8 self-rewrite-loop tests (equation-identify) against the REAL sexp-eval.
# No LLM, no IPC, no Rust runtime — pure Lisp (same offline pattern as run-closed-loop-tests.sh).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
sbcl --noinform --non-interactive --disable-debugger \
  --eval "(load #P\"src/core/boot.lisp\")" \
  --eval "(load #P\"tests/test-equation-identify.lisp\")" \
  --eval "(sb-ext:exit :code (harmonia::run-equation-identify-tests))"
