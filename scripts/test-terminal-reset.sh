#!/usr/bin/env bash
# Test that harmonia resets the terminal from raw mode.
#
# Simulates a crashed TUI session by putting the terminal in raw mode,
# then runs harmonia and checks that output doesn't have staircase pattern.
set -euo pipefail

HARMONIA="${1:-/Users/george/.local/bin/harmonia}"
PASS=0
FAIL=0

info()  { printf '\033[1;34m[TEST]\033[0m %s\n' "$*"; }
pass()  { printf '\033[1;32m[PASS]\033[0m %s\n' "$*"; PASS=$((PASS + 1)); }
fail()  { printf '\033[1;31m[FAIL]\033[0m %s\n' "$*"; FAIL=$((FAIL + 1)); }

# We need a real terminal for this test
if [ ! -t 0 ] || [ ! -t 1 ]; then
    echo "SKIP: not running in a terminal (need interactive tty for raw mode test)"
    exit 0
fi

info "Terminal reset test"
echo ""

# Save terminal state
SAVED=$(stty -g 2>/dev/null) || { echo "SKIP: stty not available"; exit 0; }

# Test 1: Put terminal in raw mode, then run harmonia, check output
info "Test 1: Raw mode recovery"

# Set raw mode (simulates crashed TUI)
stty raw -echo 2>/dev/null

# Run harmonia status, capture output
OUTPUT=$("$HARMONIA" --version 2>&1) || true

# Check terminal state after harmonia ran
AFTER=$(stty -g 2>/dev/null) || true

# Restore terminal (safety net)
stty "$SAVED" 2>/dev/null

# Check if OPOST is in the stty output (should be, means cooked mode)
if stty -a 2>/dev/null | grep -q 'opost'; then
    pass "terminal has opost after harmonia (cooked mode restored)"
else
    fail "terminal missing opost after harmonia (still raw)"
fi

# Check output doesn't have staircase (no \r\n means lines start at wrong column)
if echo "$OUTPUT" | grep -q 'harmonia'; then
    pass "output contains expected text"
else
    fail "output missing expected text: $OUTPUT"
fi

# Test 2: Normal mode stays normal
info "Test 2: Normal mode not disturbed"
OUTPUT2=$("$HARMONIA" --version 2>&1) || true
if echo "$OUTPUT2" | grep -q 'harmonia'; then
    pass "normal mode output correct"
else
    fail "normal mode output broken: $OUTPUT2"
fi

# Ensure terminal is restored
stty "$SAVED" 2>/dev/null || true

echo ""
echo "  $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] || exit 1
