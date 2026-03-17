#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[1/4] cargo test --workspace"
cargo test --workspace

echo "[2/4] cargo build --release --workspace"
cargo build --release --workspace

echo "[3/4] SBCL CFFI healthcheck sweep"
sbcl \
  --eval '(load #P"~/quicklisp/setup.lisp")' \
  --eval '(funcall (find-symbol "QUICKLOAD" (find-package :ql)) :cffi)' \
  --eval '(defun hc (path sym) (cffi:load-foreign-library path) (let ((ptr (cffi:foreign-symbol-pointer sym))) (if (cffi:null-pointer-p ptr) -1 (cffi:foreign-funcall-pointer ptr () :int))))' \
  --eval "(format t \"~&browser=~D~%\" (hc #P\"$ROOT_DIR/target/release/libharmonia_browser.dylib\" \"harmonia_browser_healthcheck\"))" \
  --eval "(format t \"cron=~D~%\" (hc #P\"$ROOT_DIR/target/release/libharmonia_cron_scheduler.dylib\" \"harmonia_cron_scheduler_healthcheck\"))" \
  --eval "(format t \"fs=~D~%\" (hc #P\"$ROOT_DIR/target/release/libharmonia_fs.dylib\" \"harmonia_fs_healthcheck\"))" \
  --eval "(format t \"git=~D~%\" (hc #P\"$ROOT_DIR/target/release/libharmonia_git_ops.dylib\" \"harmonia_git_ops_healthcheck\"))" \
  --eval "(format t \"http=~D~%\" (hc #P\"$ROOT_DIR/target/release/libharmonia_http.dylib\" \"harmonia_http_healthcheck\"))" \
  --eval "(format t \"http2=~D~%\" (hc #P\"$ROOT_DIR/target/release/libharmonia_http2_mtls.dylib\" \"harmonia_frontend_healthcheck\"))" \
  --eval "(format t \"memory=~D~%\" (hc #P\"$ROOT_DIR/target/release/libharmonia_memory.dylib\" \"harmonia_memory_healthcheck\"))" \
  --eval "(format t \"mqtt=~D~%\" (hc #P\"$ROOT_DIR/target/release/libharmonia_mqtt_client.dylib\" \"harmonia_mqtt_client_healthcheck\"))" \
  --eval "(format t \"openrouter=~D~%\" (hc #P\"$ROOT_DIR/target/release/libharmonia_openrouter.dylib\" \"harmonia_openrouter_healthcheck\"))" \
  --eval "(format t \"ouroboros=~D~%\" (hc #P\"$ROOT_DIR/target/release/libharmonia_ouroboros.dylib\" \"harmonia_ouroboros_healthcheck\"))" \
  --eval "(format t \"push-mqtt=~D~%\" (hc #P\"$ROOT_DIR/target/release/libharmonia_mqtt_client.dylib\" \"harmonia_frontend_healthcheck\"))" \
  --eval "(format t \"recovery=~D~%\" (hc #P\"$ROOT_DIR/target/release/libharmonia_recovery.dylib\" \"harmonia_recovery_healthcheck\"))" \
  --eval "(format t \"forge=~D~%\" (hc #P\"$ROOT_DIR/target/release/libharmonia_rust_forge.dylib\" \"harmonia_rust_forge_healthcheck\"))" \
  --eval "(format t \"s3=~D~%\" (hc #P\"$ROOT_DIR/target/release/libharmonia_s3.dylib\" \"harmonia_s3_healthcheck\"))" \
  --eval "(format t \"social=~D~%\" (hc #P\"$ROOT_DIR/target/release/libharmonia_social.dylib\" \"harmonia_social_healthcheck\"))" \
  --eval "(format t \"vault=~D~%\" (hc #P\"$ROOT_DIR/target/release/libharmonia_vault.dylib\" \"harmonia_vault_healthcheck\"))" \
  --eval "(format t \"tailnet=~D~%\" (hc #P\"$ROOT_DIR/target/release/libharmonia_tailnet.dylib\" \"harmonia_tailnet_healthcheck\"))" \
  --eval "(format t \"gateway=~D~%\" (hc #P\"$ROOT_DIR/target/release/libharmonia_gateway.dylib\" \"harmonia_gateway_healthcheck\"))" \
  --quit

echo "[4/4] SBCL orchestration smoke test"
: "${OPENROUTER_API_KEY:?OPENROUTER_API_KEY must be set}"
export HARMONIA_VAULT_IMPORT="${HARMONIA_VAULT_IMPORT:-OPENROUTER_API_KEY=openrouter}"
sbcl \
    --load src/core/boot.lisp \
    --eval '(harmonia:start :run-loop nil)' \
    --eval '(format t "~&SMOKE=~S~%" (harmonia:run-prompt "rewrite planner for lower token cost" :max-cycles 2))' \
    --eval '(format t "~&REWRITE_COUNT=~D~%" (harmonia::runtime-state-rewrite-count harmonia:*runtime*))' \
    --quit

echo "Harmonia validation complete."
