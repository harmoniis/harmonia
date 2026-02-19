#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

: "${OPENROUTER_API_KEY:?OPENROUTER_API_KEY must be set}"
export HARMONIA_ENV=test
export HARMONIA_VAULT_IMPORT="${HARMONIA_VAULT_IMPORT:-OPENROUTER_API_KEY=openrouter}"

TMPDIR_TEST="$(mktemp -d /tmp/harmonia-grind-XXXXXX)"
cleanup() {
  if [[ -n "${MOSQ_PID:-}" ]]; then
    kill "$MOSQ_PID" >/dev/null 2>&1 || true
    wait "$MOSQ_PID" >/dev/null 2>&1 || true
  fi
  rm -rf "$TMPDIR_TEST" >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo "[1/7] cargo test --workspace"
cargo test --workspace

echo "[2/7] cargo build --release --workspace"
cargo build --release --workspace

echo "[3/7] verify prod genesis is blocked"
if HARMONIA_ENV=prod sbcl --disable-debugger --load src/core/boot.lisp --eval '(harmonia:start :run-loop nil)' --quit >/tmp/harmonia-prod-block.out 2>&1; then
  echo "ERROR: prod genesis was not blocked"
  exit 1
fi
echo "prod genesis block: OK"

echo "[4/7] phoenix supervised restart test lane"
PHX_SENTINEL="$TMPDIR_TEST/phoenix-first-fail"
PHX_TRAUMA="$TMPDIR_TEST/trauma.log"
export PHOENIX_MAX_RESTARTS=2
export PHOENIX_TRAUMA_LOG="$PHX_TRAUMA"
export PHOENIX_CHILD_CMD="if [ ! -f '$PHX_SENTINEL' ]; then touch '$PHX_SENTINEL'; echo first-fail >&2; exit 1; else exit 0; fi"
cargo run -p harmonia-phoenix --quiet -- 1
test -f "$PHX_TRAUMA"
echo "phoenix restart+trauma: OK"

echo "[5/7] lisp boot, orchestrate, rewrite, reset"
sbcl \
  --load src/core/boot.lisp \
  --eval '(harmonia:start :run-loop nil)' \
  --eval '(format t "~&RUN1=~S~%" (harmonia:run-prompt "rewrite loop planner" :max-cycles 2))' \
  --eval '(format t "~&REWRITE_BEFORE_RESET=~D~%" (harmonia::runtime-state-rewrite-count harmonia:*runtime*))' \
  --eval '(harmonia:reset-test-genesis)' \
  --eval '(format t "~&REWRITE_AFTER_RESET=~D~%" (harmonia::runtime-state-rewrite-count harmonia:*runtime*))' \
  --quit

echo "[6/7] create local git sandbox for self-push test"
TEST_REPO="$TMPDIR_TEST/test-agent-repo"
TEST_REMOTE="$TMPDIR_TEST/test-agent-remote.git"
mkdir -p "$TEST_REPO"
git init --bare "$TEST_REMOTE" >/dev/null
git -C "$TEST_REPO" init >/dev/null
git -C "$TEST_REPO" checkout -b main >/dev/null
git -C "$TEST_REPO" remote add origin "$TEST_REMOTE"
echo "(genesis . test)" > "$TEST_REPO/dna.sexp"

echo "[7/7] CFFI tool grind (memory/mqtt/git-push/s3/ouroboros)"
MEM_FILE="$TMPDIR_TEST/memory.db"
S3_LOCAL="$TMPDIR_TEST/s3-local"
S3_SRC="$TMPDIR_TEST/artifact.bin"
echo "artifact" > "$S3_SRC"

if [[ -z "${HARMONIA_MQTT_BROKER:-}" ]]; then
  MOSQ_BIN="${HARMONIA_MOSQUITTO_BIN:-}"
  if [[ -z "$MOSQ_BIN" ]]; then
    if command -v mosquitto >/dev/null 2>&1; then
      MOSQ_BIN="$(command -v mosquitto)"
    elif [[ -x "/opt/homebrew/sbin/mosquitto" ]]; then
      MOSQ_BIN="/opt/homebrew/sbin/mosquitto"
    elif [[ -x "/usr/local/opt/mosquitto/sbin/mosquitto" ]]; then
      MOSQ_BIN="/usr/local/opt/mosquitto/sbin/mosquitto"
    fi
  fi
  if [[ -z "$MOSQ_BIN" || ! -x "$MOSQ_BIN" ]]; then
    echo "ERROR: mosquitto broker binary not found. Set HARMONIA_MQTT_BROKER or HARMONIA_MOSQUITTO_BIN."
    exit 1
  fi
  MQTT_PORT="${HARMONIA_MQTT_LOCAL_PORT:-18883}"
  MQTT_HOST="${HARMONIA_MQTT_LOCAL_HOST:-127.0.0.1}"
  MQTT_CONF="$TMPDIR_TEST/mosquitto.conf"
  MQTT_LOG="$TMPDIR_TEST/mosquitto.log"
  cat > "$MQTT_CONF" <<EOF
listener $MQTT_PORT $MQTT_HOST
persistence false
allow_anonymous true
EOF
  "$MOSQ_BIN" -c "$MQTT_CONF" -v >"$MQTT_LOG" 2>&1 &
  MOSQ_PID=$!
  sleep 1
  export HARMONIA_MQTT_BROKER="${MQTT_HOST}:${MQTT_PORT}"
  export HARMONIA_MQTT_TLS=0
fi

LISP_TEST="$TMPDIR_TEST/grind.lisp"
cat > "$LISP_TEST" <<EOF
(load #P"~/quicklisp/setup.lisp")
(funcall (find-symbol "QUICKLOAD" (find-package :ql)) :cffi)

(cffi:load-foreign-library #P"$ROOT_DIR/target/release/libharmonia_memory.dylib")
(cffi:load-foreign-library #P"$ROOT_DIR/target/release/libharmonia_mqtt_client.dylib")
(cffi:load-foreign-library #P"$ROOT_DIR/target/release/libharmonia_git_ops.dylib")
(cffi:load-foreign-library #P"$ROOT_DIR/target/release/libharmonia_s3_sync.dylib")
(cffi:load-foreign-library #P"$ROOT_DIR/target/release/libharmonia_ouroboros.dylib")

(cffi:defcfun ("harmonia_memory_init" mem-init) :int (path :string))
(cffi:defcfun ("harmonia_memory_put" mem-put) :int (k :string) (v :string))
(cffi:defcfun ("harmonia_memory_get" mem-get) :pointer (k :string))
(cffi:defcfun ("harmonia_memory_free_string" mem-free) :void (p :pointer))

(cffi:defcfun ("harmonia_mqtt_client_reset" mqtt-reset) :int)
(cffi:defcfun ("harmonia_mqtt_client_publish" mqtt-publish) :int (topic :string) (payload :string))
(cffi:defcfun ("harmonia_mqtt_client_poll" mqtt-poll) :pointer (topic :string))
(cffi:defcfun ("harmonia_mqtt_client_free_string" mqtt-free) :void (p :pointer))
(cffi:defcfun ("harmonia_mqtt_client_last_error" mqtt-last-error) :pointer)

(cffi:defcfun ("harmonia_git_ops_commit_all" git-commit-all) :int
  (repo :string) (msg :string) (name :string) (email :string))
(cffi:defcfun ("harmonia_git_ops_push" git-push) :int (repo :string) (remote :string) (branch :string))

(cffi:defcfun ("harmonia_s3_sync_upload_file" s3-upload) :int
  (source :string) (bucket :string) (prefix :string) (key :string))

(cffi:defcfun ("harmonia_ouroboros_record_crash" ouro-record) :int (component :string) (detail :string))
(cffi:defcfun ("harmonia_ouroboros_last_crash" ouro-last) :pointer)
(cffi:defcfun ("harmonia_ouroboros_free_string" ouro-free) :void (p :pointer))

(unless (zerop (mem-init "$MEM_FILE")) (error "memory init failed"))
(unless (zerop (mem-put "dna" "(cycle . 1)")) (error "memory put failed"))
(let ((p (mem-get "dna")))
  (when (cffi:null-pointer-p p) (error "memory get null"))
  (format t "~&MEMORY=~A~%" (cffi:foreign-string-to-lisp p))
  (mem-free p))

(unless (zerop (mqtt-reset)) (error "mqtt reset failed"))
(unless (zerop (mqtt-publish "harmonia/test/grind-001" "(event . ok)"))
  (let ((ep (mqtt-last-error)))
    (unwind-protect
         (error "mqtt publish failed: ~A"
                (if (cffi:null-pointer-p ep) "<unknown>" (cffi:foreign-string-to-lisp ep)))
      (unless (cffi:null-pointer-p ep) (mqtt-free ep)))))
(let ((p (mqtt-poll "harmonia/test/grind-001")))
  (when (cffi:null-pointer-p p) (error "mqtt poll null"))
  (format t "~&MQTT=~A~%" (cffi:foreign-string-to-lisp p))
  (mqtt-free p))

(unless (zerop (git-commit-all "$TEST_REPO" "self update (test)" "Harmonia Test" "harmonia@test.local"))
  (error "git commit failed"))
(unless (zerop (git-push "$TEST_REPO" "origin" "main"))
  (error "git push failed"))
(format t "~&GIT_PUSH=OK~%")

(unless (zerop (s3-upload "$S3_SRC" "test-bucket" "v-test" "artifact.bin"))
  (error "s3 upload failed"))
(format t "~&S3_UPLOAD=OK~%")

(unless (zerop (ouro-record "openrouter-backend" "simulated timeout"))
  (error "ouroboros record failed"))
(let ((p (ouro-last)))
  (when (cffi:null-pointer-p p) (error "ouroboros last crash null"))
  (format t "~&OUROBOROS=~A~%" (cffi:foreign-string-to-lisp p))
  (ouro-free p))

(sb-ext:exit :code 0)
EOF

HARMONIA_S3_MODE=local HARMONIA_S3_LOCAL_ROOT="$S3_LOCAL" sbcl --disable-debugger --load "$LISP_TEST" --quit

git --git-dir "$TEST_REMOTE" rev-parse refs/heads/main >/dev/null
test -f "$S3_LOCAL/test-bucket/v-test/artifact.bin"

echo "Grind test complete: all core test-lane systems validated."
