#!/usr/bin/env bash
# Orchestrate a TUI integration test: bring up runtime + agent, run the
# Python TUI client against the live socket, capture results, tear down.
set -uo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

STATE_ROOT="/var/folders/3y/xmp5j1xj3ldcxxm2p_xqwwgc0000gn/T/harmonia"
RUNTIME_LOG="/tmp/harmonia-runtime-tuitest.log"
AGENT_LOG="/tmp/harmonia-agent-tuitest.log"
TEST_LOG="/tmp/harmonia-tui-test.log"

cleanup() {
  echo "[orchestrate] shutting down…"
  pkill -f harmonia-runtime 2>/dev/null || true
  pkill -f tui-test-agent.lisp 2>/dev/null || true
  sleep 1
}
trap cleanup EXIT INT TERM

# Start fresh.
pkill -f harmonia-runtime 2>/dev/null || true
pkill -f tui-test-agent.lisp 2>/dev/null || true
sleep 1
rm -f "$RUNTIME_LOG" "$AGENT_LOG" "$TEST_LOG"

echo "[orchestrate] starting harmonia-runtime…"
target/release/harmonia-runtime > "$RUNTIME_LOG" 2>&1 &

# Wait for runtime ready.
for i in $(seq 1 60); do
  if grep -q "All actors spawned, IPC ready" "$RUNTIME_LOG" 2>/dev/null; then
    break
  fi
  sleep 1
done
if ! grep -q "All actors spawned, IPC ready" "$RUNTIME_LOG" 2>/dev/null; then
  echo "[orchestrate] runtime never became ready"
  tail -30 "$RUNTIME_LOG"
  exit 2
fi
echo "[orchestrate] runtime ready"

echo "[orchestrate] starting SBCL agent…"
HARMONIA_STATE_ROOT="$STATE_ROOT" \
HARMONIA_ENV=test \
HARMONIA_OPENROUTER_ALLOW_OFFLINE=0 \
HARMONIA_OPENROUTER_FALLBACK_MODELS="google/gemma-3-4b-it:free,qwen/qwen3-coder:free" \
sbcl --disable-debugger --script scripts/tui-test-agent.lisp > "$AGENT_LOG" 2>&1 &

# Wait for agent boot.
for i in $(seq 1 60); do
  if grep -q "Bootstrap complete" "$AGENT_LOG" 2>/dev/null; then
    break
  fi
  sleep 1
done
if ! grep -q "Bootstrap complete" "$AGENT_LOG" 2>/dev/null; then
  echo "[orchestrate] agent never finished boot"
  tail -30 "$AGENT_LOG"
  exit 3
fi
echo "[orchestrate] agent ready, sleeping 3s for actor warm-up…"
sleep 3

echo "[orchestrate] running TUI memory test…"
python3 scripts/tui-memory-test.py 2>&1 | tee "$TEST_LOG"
TEST_EXIT=${PIPESTATUS[0]}

echo
echo "── RUNTIME LOG TAIL ────────────────────────────────────────"
tail -20 "$RUNTIME_LOG"
echo
echo "── AGENT LOG TAIL ──────────────────────────────────────────"
tail -30 "$AGENT_LOG"

exit $TEST_EXIT
