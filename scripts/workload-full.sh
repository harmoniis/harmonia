#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

: "${OPENROUTER_API_KEY:?OPENROUTER_API_KEY must be set}"

echo "[A] workspace tests"
cargo test --workspace

echo "[B] live OpenRouter + AWS + S3"
set -a
# shellcheck disable=SC1091
source /Users/george/harmoniis/projects/.env
set +a
OPENROUTER_API_KEY="$OPENROUTER_API_KEY" ./scripts/workload-cloud.sh

echo "[C] local PGP+TLS MQTT"
./scripts/test-mqtt-tls.sh

echo "[D] core ffi live checks"
./scripts/test-ffi-live.sh

echo "[E] Harmonia-loop self-push + cleanup"
TMPDIR_TEST="$(mktemp -d /tmp/harmonia-hardproof-loop-XXXXXX)"
REPO_DIR="$TMPDIR_TEST/harmonia"
git clone git@github.com:harmoniis/harmonia.git "$REPO_DIR" >/tmp/hhp_clone.log 2>/tmp/hhp_clone.err
BRANCH="test/harmonia-hardproof-$(date -u +%Y%m%dT%H%M%SZ)"
git -C "$REPO_DIR" checkout -b "$BRANCH" >/tmp/hhp_co.log 2>/tmp/hhp_co.err
OPENROUTER_API_KEY="$OPENROUTER_API_KEY" HARMONIA_ENV=test sbcl --disable-debugger \
  --load src/core/boot.lisp \
  --eval '(harmonia:start :run-loop nil)' \
  --eval "(format t \"~&LOOP_PUSH=~S~%\" (harmonia:run-self-push-test \"$REPO_DIR/\" \"$BRANCH\"))" \
  --quit
git -C "$REPO_DIR" ls-remote --heads origin "$BRANCH"
git -C "$REPO_DIR" push origin --delete "$BRANCH" >/tmp/hhp_del.log 2>/tmp/hhp_del.err

echo "[F] harmonic genesis state-machine feedback loop"
OPENROUTER_API_KEY="$OPENROUTER_API_KEY" ./scripts/test-genesis-loop.sh

echo "[G] communication/search/voice tools smoke"
./scripts/test-frontends.sh

echo "Full workload complete."
