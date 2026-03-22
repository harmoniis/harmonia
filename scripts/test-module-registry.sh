#!/usr/bin/env bash
# Test the module registry system end-to-end.
#
# Prerequisites:
#   - Harmonia must be built (cargo build --workspace)
#   - Harmonia daemon must be running (harmonia start)
#
# Tests:
#   1. Module list via IPC
#   2. Unload a non-core module
#   3. Load a non-core module
#   4. Reload a module
#   5. Refuse to unload core module
#   6. Load unconfigured module (should fail on config check)
#   7. Unknown module name
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HARMONIA="${ROOT_DIR}/target/release/harmonia"
PASS=0
FAIL=0
TOTAL=0

info()  { printf '\033[1;34m[TEST]\033[0m %s\n' "$*"; }
pass()  { printf '\033[1;32m[PASS]\033[0m %s\n' "$*"; PASS=$((PASS + 1)); TOTAL=$((TOTAL + 1)); }
fail()  { printf '\033[1;31m[FAIL]\033[0m %s\n' "$*"; FAIL=$((FAIL + 1)); TOTAL=$((TOTAL + 1)); }

if [ ! -x "$HARMONIA" ]; then
    HARMONIA="$(command -v harmonia 2>/dev/null || true)"
    if [ -z "$HARMONIA" ]; then
        echo "ERROR: harmonia binary not found. Build first: cargo build --workspace --release"
        exit 1
    fi
fi

# Check daemon is running
if ! "$HARMONIA" status 2>/dev/null | grep -q 'running'; then
    echo "ERROR: Harmonia daemon is not running. Start it: harmonia start"
    exit 1
fi

info "Module registry tests"
echo ""

# ── Test 1: List modules ──────────────────────────────────────────
info "Test 1: List all modules"
output=$("$HARMONIA" modules list 2>&1) || true
if echo "$output" | grep -q 'Harmonia Modules'; then
    pass "modules list shows header"
else
    fail "modules list missing header: $output"
fi

if echo "$output" | grep -q 'loaded'; then
    pass "modules list shows loaded modules"
else
    fail "modules list has no loaded modules: $output"
fi

if echo "$output" | grep -q 'config-store'; then
    pass "modules list includes config-store"
else
    fail "modules list missing config-store: $output"
fi

if echo "$output" | grep -q '(core)'; then
    pass "modules list shows core markers"
else
    fail "modules list missing core markers: $output"
fi

# ── Test 2: Unload non-core module ────────────────────────────────
info "Test 2: Unload signalograd (non-core)"
output=$("$HARMONIA" modules unload signalograd 2>&1) || true
if echo "$output" | grep -qi 'ok\|unloaded'; then
    pass "unload signalograd succeeded"
else
    fail "unload signalograd unexpected: $output"
fi

# ── Test 3: Load non-core module ─────────────────────────────────
info "Test 3: Load signalograd back"
output=$("$HARMONIA" modules load signalograd 2>&1) || true
if echo "$output" | grep -qi 'ok\|loaded'; then
    pass "load signalograd succeeded"
else
    fail "load signalograd unexpected: $output"
fi

# ── Test 4: Reload module ────────────────────────────────────────
info "Test 4: Reload signalograd"
output=$("$HARMONIA" modules reload signalograd 2>&1) || true
if echo "$output" | grep -qi 'ok\|loaded'; then
    pass "reload signalograd succeeded"
else
    fail "reload signalograd unexpected: $output"
fi

# ── Test 5: Refuse core unload ───────────────────────────────────
info "Test 5: Refuse to unload core module (config-store)"
output=$("$HARMONIA" modules unload config-store 2>&1) || true
if echo "$output" | grep -qi 'error\|cannot unload core'; then
    pass "core unload refused correctly"
else
    fail "core unload should have been refused: $output"
fi

# ── Test 6: Load unconfigured module (missing config) ────────────
info "Test 6: Load telegram (likely missing bot token)"
# First unload if loaded
"$HARMONIA" modules unload telegram 2>/dev/null || true
output=$("$HARMONIA" modules load telegram 2>&1) || true
if echo "$output" | grep -qi 'error\|missing config\|already loaded'; then
    pass "unconfigured module load handled correctly"
else
    fail "unconfigured module load unexpected: $output"
fi

# ── Test 7: Unknown module name ──────────────────────────────────
info "Test 7: Unknown module name"
output=$("$HARMONIA" modules load nonexistent-module 2>&1) || true
if echo "$output" | grep -qi 'error\|unknown'; then
    pass "unknown module rejected correctly"
else
    fail "unknown module should have been rejected: $output"
fi

# ── Test 8: Modules list (default subcommand) ────────────────────
info "Test 8: 'harmonia modules' with no subcommand defaults to list"
output=$("$HARMONIA" modules 2>&1) || true
if echo "$output" | grep -q 'Harmonia Modules'; then
    pass "default subcommand shows list"
else
    fail "default subcommand missing list output: $output"
fi

# ── Summary ──────────────────────────────────────────────────────
echo ""
echo "────────────────────────────────────────"
echo "  $PASS passed, $FAIL failed (of $TOTAL)"
echo "────────────────────────────────────────"

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
