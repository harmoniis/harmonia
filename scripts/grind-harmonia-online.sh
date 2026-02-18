#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

: "${OPENROUTER_API_KEY:?OPENROUTER_API_KEY must be set}"

ENV_FILE="${ENV_FILE:-/Users/george/harmoniis/projects/.env}"
if [ -f "$ENV_FILE" ]; then
  set -a
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  set +a
fi

: "${AWS_ACCESS_KEY_ID:?AWS_ACCESS_KEY_ID must be set (via .env or env)}"
: "${AWS_SECRET_ACCESS_KEY:?AWS_SECRET_ACCESS_KEY must be set (via .env or env)}"
: "${AWS_DEFAULT_REGION:?AWS_DEFAULT_REGION must be set (via .env or env)}"

S3_BUCKET="${S3_BUCKET:-harmoniis-wallet-state-339712728485}"
S3_PREFIX="${S3_PREFIX:-harmonia-test}"

build_vault_import_map() {
  local parts=()
  if [ -n "${OPENROUTER_API_KEY:-}" ]; then
    parts+=("OPENROUTER_API_KEY=openrouter")
  fi
  if [ -n "${EXA_API_KEY:-}" ]; then
    parts+=("EXA_API_KEY=exa_api_key|exa")
  fi
  if [ -n "${BRAVE_SEARCH_API_KEY:-}" ]; then
    parts+=("BRAVE_SEARCH_API_KEY=brave_api_key|brave")
  fi
  if [ -n "${BRAVE_API_KEY:-}" ]; then
    parts+=("BRAVE_API_KEY=brave_api_key|brave")
  fi
  local map
  map="$(IFS=,; echo "${parts[*]}")"
  if [ -n "$map" ]; then
    export HARMONIA_VAULT_IMPORT="$map"
  fi
}

build_vault_import_map

echo "[1/4] Live OpenRouter path (Lisp -> Backend; backend reads key from vault)"
HARMONIA_ENV=test \
HARMONIA_OPENROUTER_ALLOW_OFFLINE=0 \
sbcl --disable-debugger \
  --load src/core/boot.lisp \
  --eval '(harmonia:start :run-loop nil)' \
  --eval '(format t "~&ONLINE_PROMPT=~S~%" (harmonia:run-prompt "Reply exactly with: ONLINE_OK" :max-cycles 2))' \
  --quit

echo "[2/4] AWS identity"
aws sts get-caller-identity

echo "[3/4] Live S3 upload via libharmonia_s3_sync.dylib"
TMPF="$(mktemp /tmp/harmonia-s3-upload-XXXXXX)"
echo "harmonia online upload $(date -u +%FT%TZ)" > "$TMPF"
S3_KEY="online-$(date -u +%Y%m%dT%H%M%SZ).txt"

HARMONIA_S3_MODE=aws sbcl --disable-debugger \
  --eval '(load #P"~/quicklisp/setup.lisp")' \
  --eval '(funcall (find-symbol "QUICKLOAD" (find-package :ql)) :cffi)' \
  --eval '(cffi:load-foreign-library #P"/Users/george/harmoniis/projects/agent/harmonia/target/release/libharmonia_s3_sync.dylib")' \
  --eval '(cffi:defcfun ("harmonia_s3_sync_upload_file" up) :int (src :string) (bucket :string) (prefix :string) (key :string))' \
  --eval "(format t \"~&S3_RC=~D~%\" (up \"$TMPF\" \"$S3_BUCKET\" \"$S3_PREFIX\" \"$S3_KEY\"))" \
  --quit

echo "[4/4] Verify object exists"
aws s3 ls "s3://$S3_BUCKET/$S3_PREFIX/$S3_KEY"

echo "Online grind complete."
