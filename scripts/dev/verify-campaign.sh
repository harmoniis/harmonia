#!/usr/bin/env bash
# verify-campaign.sh — the continuous test/verify engine for the Harmonia<->DYSCO
# scientific-verification campaign. Runs every check, reports GREEN/RED/BLOCKED, and
# never aborts on a single failure. Pure orchestration (Lisp+Rust only); the logic it
# drives is Rust (cargo) and Lisp (sbcl probes).
#
#   bash scripts/dev/verify-campaign.sh
#
# Exit 0 iff every runnable check is GREEN (BLOCKED checks do not fail the run).
set -uo pipefail
cd "$(dirname "$0")/../.."

PASS=0; FAIL=0; BLOCK=0
run() { local name="$1"; shift
  printf '\n=== %s ===\n' "$name"
  if "$@"; then echo "[GREEN] $name"; PASS=$((PASS+1)); else echo "[RED]   $name"; FAIL=$((FAIL+1)); fi; }
block() { echo "[BLOCKED] $1"; BLOCK=$((BLOCK+1)); }

# ── Phase 2 + 4 — chaotic-substrate invariants + D1 symbolic recovery (Rust) ──
run "Phase2+4  dynamics-verify (F1 saturation / F2 Euler / D1 recovery)" \
    cargo test --release -p harmonia-dynamics-verify

# ── Phase 6 (offline) — core Rust crate tests touched by the audit ──
run "Phase6    core Rust tests (memory-field, signalograd, mempalace, chronicle)" \
    cargo test -p harmonia-memory-field -p harmonia-signalograd \
               -p harmonia-mempalace -p harmonia-chronicle

# ── Phase 6 (live agent) — operator-run only (safety + environment) ──
# The live circuits / memory-field probes need the FULL phoenix stack up (Rust runtime +
# sbcl agent) so IPC is reachable. We do NOT start it from here: a dev runtime binds the
# shared TUI socket and could collide with a live agent. Bring it up safely with the
# canonical, pid-tracked harness (never touches the live state root):
#   scripts/dev/harness.sh bringup && scripts/dev/harness.sh repl-test && scripts/dev/harness.sh teardown
# And: this sandbox has no reachable LLM provider (orchestration -> "IPC unreachable"), so
# the LLM-dependent assertions are environment-blocked regardless of plumbing.
block "Phase6 live-agent circuits/memory-field probes (operator: harness.sh bringup; sandbox LLM provider unreachable)"
block "resonance-probe (live LLM) — operator-run with full stack + reachable LLM provider"

printf '\n========== CAMPAIGN VERIFY: %d GREEN, %d RED, %d BLOCKED ==========\n' "$PASS" "$FAIL" "$BLOCK"
[ "$FAIL" -eq 0 ]
