#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[1/3] build communication/search/voice tools"
cargo build --release \
  -p harmonia-whatsapp \
  -p harmonia-telegram \
  -p harmonia-slack \
  -p harmonia-mattermost \
  -p harmonia-nostr \
  -p harmonia-email-client \
  -p harmonia-search-exa \
  -p harmonia-search-brave \
  -p harmonia-whisper \
  -p harmonia-elevenlabs \
  -p harmonia-parallel-agents \
  -p harmonia-harmonic-matrix \
  -p harmonia-browser

echo "[2/3] cffi healthcheck + vault-smoke"
sbcl --disable-debugger \
  --eval '(load #P"~/quicklisp/setup.lisp")' \
  --eval '(funcall (find-symbol "QUICKLOAD" (find-package :ql)) :cffi)' \
  --eval '(cffi:load-foreign-library #P"/Users/george/harmoniis/projects/agent/harmonia/target/release/libharmonia_whatsapp.dylib")' \
  --eval '(cffi:load-foreign-library #P"/Users/george/harmoniis/projects/agent/harmonia/target/release/libharmonia_telegram.dylib")' \
  --eval '(cffi:load-foreign-library #P"/Users/george/harmoniis/projects/agent/harmonia/target/release/libharmonia_slack.dylib")' \
  --eval '(cffi:load-foreign-library #P"/Users/george/harmoniis/projects/agent/harmonia/target/release/libharmonia_mattermost.dylib")' \
  --eval '(cffi:load-foreign-library #P"/Users/george/harmoniis/projects/agent/harmonia/target/release/libharmonia_nostr.dylib")' \
  --eval '(cffi:load-foreign-library #P"/Users/george/harmoniis/projects/agent/harmonia/target/release/libharmonia_email_client.dylib")' \
  --eval '(cffi:load-foreign-library #P"/Users/george/harmoniis/projects/agent/harmonia/target/release/libharmonia_search_exa.dylib")' \
  --eval '(cffi:load-foreign-library #P"/Users/george/harmoniis/projects/agent/harmonia/target/release/libharmonia_search_brave.dylib")' \
  --eval '(cffi:load-foreign-library #P"/Users/george/harmoniis/projects/agent/harmonia/target/release/libharmonia_whisper.dylib")' \
  --eval '(cffi:load-foreign-library #P"/Users/george/harmoniis/projects/agent/harmonia/target/release/libharmonia_elevenlabs.dylib")' \
  --eval '(cffi:load-foreign-library #P"/Users/george/harmoniis/projects/agent/harmonia/target/release/libharmonia_parallel_agents.dylib")' \
  --eval '(cffi:load-foreign-library #P"/Users/george/harmoniis/projects/agent/harmonia/target/release/libharmonia_harmonic_matrix.dylib")' \
  --eval '(cffi:load-foreign-library #P"/Users/george/harmoniis/projects/agent/harmonia/target/release/libharmonia_browser.dylib")' \
  --eval '(cffi:defcfun ("harmonia_whatsapp_healthcheck" wa-hc) :int)' \
  --eval '(cffi:defcfun ("harmonia_telegram_healthcheck" tg-hc) :int)' \
  --eval '(cffi:defcfun ("harmonia_slack_healthcheck" sl-hc) :int)' \
  --eval '(cffi:defcfun ("harmonia_mattermost_healthcheck" mm-hc) :int)' \
  --eval '(cffi:defcfun ("harmonia_nostr_healthcheck" ns-hc) :int)' \
  --eval '(cffi:defcfun ("harmonia_email_client_healthcheck" em-hc) :int)' \
  --eval '(cffi:defcfun ("harmonia_search_exa_healthcheck" exa-hc) :int)' \
  --eval '(cffi:defcfun ("harmonia_search_brave_healthcheck" br-hc) :int)' \
  --eval '(cffi:defcfun ("harmonia_whisper_healthcheck" wh-hc) :int)' \
  --eval '(cffi:defcfun ("harmonia_elevenlabs_healthcheck" el-hc) :int)' \
  --eval '(cffi:defcfun ("harmonia_parallel_agents_healthcheck" pa-hc) :int)' \
  --eval '(cffi:defcfun ("harmonia_harmonic_matrix_healthcheck" hm-hc) :int)' \
  --eval '(cffi:defcfun ("harmonia_browser_healthcheck" bw-hc) :int)' \
  --eval '(cffi:defcfun ("harmonia_whatsapp_store_linked_device" wa-store) :int (id :string) (cred :string))' \
  --eval '(format t "~&COMMS_HC=~S~%" (list (wa-hc) (tg-hc) (sl-hc) (mm-hc) (ns-hc) (em-hc) (exa-hc) (br-hc) (wh-hc) (el-hc) (pa-hc) (hm-hc) (bw-hc)))' \
  --eval '(format t "~&WA_STORE=~D~%" (wa-store "device-test" "cred-test"))' \
  --quit

echo "[3/3] verify whatsapp device creds persisted in vault store"
rg -n "whatsapp_device_id|whatsapp_device_creds" /tmp/harmonia/vault.secrets >/dev/null
echo "Communication tools smoke complete."
