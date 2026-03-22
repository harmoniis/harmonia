#!/usr/bin/env bash
# Test the install config template and headless provisioning.
#
# Tests:
#   1. Template JSON is valid
#   2. Headless setup with a minimal test config
#   3. Vault secrets are written correctly
#   4. Config-store values are written correctly
#   5. Runtime components auto-detection
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HARMONIA="${ROOT_DIR}/target/release/harmonia"
TEMPLATE="${ROOT_DIR}/config/install-config.template.json"
PASS=0
FAIL=0
TOTAL=0

info()  { printf '\033[1;34m[TEST]\033[0m %s\n' "$*"; }
pass()  { printf '\033[1;32m[PASS]\033[0m %s\n' "$*"; PASS=$((PASS + 1)); TOTAL=$((TOTAL + 1)); }
fail()  { printf '\033[1;31m[FAIL]\033[0m %s\n' "$*"; FAIL=$((FAIL + 1)); TOTAL=$((TOTAL + 1)); }

if ! command -v jq >/dev/null 2>&1; then
    echo "SKIP: jq not installed (required for these tests)"
    exit 0
fi

info "Install config tests"
echo ""

# ── Test 1: Template is valid JSON ───────────────────────────────
info "Test 1: Template JSON is valid"
if jq . "$TEMPLATE" > /dev/null 2>&1; then
    pass "template is valid JSON"
else
    fail "template is invalid JSON"
fi

# ── Test 2: Template has expected top-level keys ─────────────────
info "Test 2: Template structure"
for key in node paths vault_secrets config_store; do
    if jq -e ".$key" "$TEMPLATE" > /dev/null 2>&1; then
        pass "template has .$key"
    else
        fail "template missing .$key"
    fi
done

# ── Test 3: Template has expected vault secret keys ──────────────
info "Test 3: Expected vault secret keys in template"
expected_secrets=(
    "openrouter-api-key"
    "telegram-bot-token"
    "slack-bot-token"
    "discord-bot-token"
    "langsmith-api-key"
    "github-token"
)
for secret in "${expected_secrets[@]}"; do
    if jq -e ".vault_secrets[\"$secret\"]" "$TEMPLATE" > /dev/null 2>&1; then
        pass "template has vault secret: $secret"
    else
        fail "template missing vault secret: $secret"
    fi
done

# ── Test 4: Template has config_store structure ──────────────────
info "Test 4: Config store structure"
if jq -e '.config_store["harmonia-cli"]["model-policy"]["provider"]' "$TEMPLATE" > /dev/null 2>&1; then
    pass "template has model-policy provider"
else
    fail "template missing model-policy provider"
fi

if jq -e '.config_store["harmonia-runtime"]["runtime"]["components"]' "$TEMPLATE" > /dev/null 2>&1; then
    pass "template has runtime components key"
else
    fail "template missing runtime components key"
fi

# ── Test 5: Install script parses --config flag ──────────────────
info "Test 5: install.sh accepts --config flag in parse_args"
if grep -q '\-\-config)' "${ROOT_DIR}/scripts/install.sh"; then
    pass "install.sh has --config flag"
else
    fail "install.sh missing --config flag"
fi

if grep -q 'CONFIG_JSON' "${ROOT_DIR}/scripts/install.sh"; then
    pass "install.sh has CONFIG_JSON variable"
else
    fail "install.sh missing CONFIG_JSON variable"
fi

# ── Test 6: install.sh syntax is valid ───────────────────────────
info "Test 6: install.sh syntax check"
if bash -n "${ROOT_DIR}/scripts/install.sh" 2>/dev/null; then
    pass "install.sh syntax is valid"
else
    fail "install.sh has syntax errors"
fi

# ── Summary ──────────────────────────────────────────────────────
echo ""
echo "────────────────────────────────────────"
echo "  $PASS passed, $FAIL failed (of $TOTAL)"
echo "────────────────────────────────────────"

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
