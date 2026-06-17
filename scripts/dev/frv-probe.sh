#!/usr/bin/env bash
# frv-probe.sh — discharge the runtime contracts (formal-verification.lisp) against the LIVE node.
# Standalone, log-only: it runs the operation-level postconditions and prints SCOPED verdicts. An
# operation-level VIOLATION means that operation is genuinely broken (sound, not a sample). This is
# the honest answer to "is the memory actually working" — it will report the field as broken.
set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DEV="${HARMONIA_DEV_ROOT:-$HOME/.harmoniis/harmonia-dev}"
SOCK="${TMPDIR:-/tmp}/harmonia/harmonia.sock"
[ -S "$SOCK" ] || "$REPO/scripts/dev/harness.sh" bringup >/dev/null 2>&1

ASSERT="$(mktemp -t frv-XXXX).lisp"
cat > "$ASSERT" <<'LISP'
(in-package :harmonia)
(format t "~%════════ FORMAL RUNTIME VERIFICATION (runtime contracts — sound, not sampled) ════════~%")
(format t "  ~A registered contract(s). Operation-level = sound+total; collection-level = spot-check.~%~%"
        (length *frv-contracts*))
(multiple-value-bind (all-sound-hold verdicts) (frv-report)
  (declare (ignore verdicts))
  (format t "~%  VERDICT: ~A~%"
          (if all-sound-hold
              "all OPERATION-LEVEL (sound) contracts hold — those operations satisfy their postconditions"
              "an OPERATION-LEVEL (sound) contract is VIOLATED — that operation is broken (not a flake)"))
  (format t "════════════════════════════════════════════════════════════════════════════════════~%")
  (sb-ext:exit :code (if all-sound-hold 0 1)))
LISP

env HARMONIA_STATE_ROOT="$DEV" HARMONIA_SYSTEM_DIR="$DEV" HARMONIA_VAULT_DB="$DEV/vault.db" \
    HARMONIA_SOURCE_DIR="$REPO" HARMONIA_ENV=dev HARMONIA_LOG_LEVEL=error \
  sbcl --noinform --disable-debugger --load "$REPO/src/core/boot.lisp" \
       --eval '(harmonia:start :run-loop nil)' --load "$ASSERT"
RC=$?
rm -f "$ASSERT"
exit $RC
