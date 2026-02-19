#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

: "${OPENROUTER_API_KEY:?OPENROUTER_API_KEY must be set}"

if [ -n "${OPENROUTER_API_KEY:-}" ]; then
  export HARMONIA_VAULT_IMPORT="${HARMONIA_VAULT_IMPORT:-OPENROUTER_API_KEY=openrouter}"
fi

echo "[1/2] harmonic genesis loop (live self-feedback)"
HARMONIA_ENV=test \
HARMONIA_OPENROUTER_ALLOW_OFFLINE=0 \
HARMONIA_OPENROUTER_FALLBACK_MODELS="${HARMONIA_OPENROUTER_FALLBACK_MODELS:-google/gemma-3-4b-it:free,qwen/qwen3-coder:free}" \
HARMONIA_GENESIS_MAX_SECONDS="${HARMONIA_GENESIS_MAX_SECONDS:-240}" \
HARMONIA_MEMORY_IDLE_SECONDS=0 \
HARMONIA_MEMORY_HEARTBEAT_SECONDS=0 \
HARMONIA_MEMORY_NIGHT_START=0 \
HARMONIA_MEMORY_NIGHT_END=24 \
sbcl --disable-debugger --script scripts/harmonic-genesis-loop.lisp

echo "[2/2] harmonic genesis loop complete"
