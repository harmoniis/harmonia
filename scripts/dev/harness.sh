#!/usr/bin/env bash
# harness.sh — Harmonia closed-loop verification harness (isolated dev node).
#
# Subcommands:
#   bringup    Start runtime + sbcl-agent on the dev node; wait until TUI socket is ready.
#   smoke      Drive a prompt through the live TUI socket; assert a coherent, non-error reply.
#   repl-test  Headless: init ports (no run-loop) and call %orchestrate-repl on several prompts.
#              Fast gate for Lisp REPL/harness fixes — no TUI, no actor loop.
#   teardown   Stop the dev runtime + sbcl processes.
#   status     Show what's running.
#
# Never touches the live state root (~/.harmoniis/harmonia). Never builds (cargo is a
# separate, strictly-serialized gate — see build-gate.sh).
set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DEV="${HARMONIA_DEV_ROOT:-$HOME/.harmoniis/harmonia-dev}"
RUNTIME_BIN="$REPO/target/release/harmonia-runtime"
RUNTIME_LOG="/tmp/hdev-runtime.log"
SBCL_LOG="/tmp/hdev-sbcl.log"
TUI_SOCK="${TMPDIR:-/tmp}/harmonia/harmonia.sock"
RUN_DIR="$DEV/run"
RUNTIME_PID_FILE="$RUN_DIR/runtime.pid"
SBCL_PID_FILE="$RUN_DIR/sbcl.pid"

export HARMONIA_STATE_ROOT="$DEV" HARMONIA_SYSTEM_DIR="$DEV" \
       HARMONIA_VAULT_DB="$DEV/vault.db" HARMONIA_SOURCE_DIR="$REPO" \
       HARMONIA_ENV=dev HARMONIA_NODE_LABEL=harmonia-dev HARMONIA_LOG_LEVEL="${HARMONIA_LOG_LEVEL:-info}"

log() { printf '\033[36m[harness]\033[0m %s\n' "$*"; }

wait_for() { # wait_for <file> <grep-pattern> <max-iters>
  local f="$1" pat="$2" max="${3:-400}" i=0
  while [ "$i" -lt "$max" ]; do
    grep -q "$pat" "$f" 2>/dev/null && return 0
    i=$((i+1))
    sleep 0.05
  done
  return 1
}

stop_tracked() { # stop_tracked <pid-file> <label> <expected-command-fragment>
  local pid_file="$1" label="$2" expected="$3" pid command i
  [ -f "$pid_file" ] || return 1
  read -r pid < "$pid_file" || pid=""
  if [[ ! "$pid" =~ ^[0-9]+$ ]] || ! kill -0 "$pid" 2>/dev/null; then
    rm -f "$pid_file"
    return 1
  fi
  command="$(ps -p "$pid" -o command= 2>/dev/null || true)"
  if [[ "$command" != *"$expected"* ]]; then
    log "REFUSING to stop $label pid=$pid: command does not match $expected"
    return 2
  fi
  kill -TERM "$pid" 2>/dev/null || true
  for ((i=0; i<100; i++)); do
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.1
  done
  kill -0 "$pid" 2>/dev/null && kill -KILL "$pid" 2>/dev/null || true
  rm -f "$pid_file"
  log "stopped $label pid=$pid"
  return 0
}

cmd_teardown() {
  local runtime_stopped=0
  stop_tracked "$SBCL_PID_FILE" "sbcl-agent" "$REPO/src/core/boot.lisp" || true
  if stop_tracked "$RUNTIME_PID_FILE" "runtime" "$RUNTIME_BIN"; then
    runtime_stopped=1
  fi
  [ "$runtime_stopped" -eq 1 ] && rm -f "$TUI_SOCK" 2>/dev/null || true
}

cmd_bringup() {
  mkdir -p "$DEV" "$RUN_DIR"
  [ -x "$RUNTIME_BIN" ] || { log "FATAL: $RUNTIME_BIN missing — run build-gate.sh first"; exit 2; }
  cmd_teardown
  [ ! -S "$TUI_SOCK" ] || {
    log "FATAL: TUI socket already exists and is not owned by this isolated harness: $TUI_SOCK"
    exit 2
  }
  log "starting runtime → $RUNTIME_LOG"
  nohup "$RUNTIME_BIN" >"$RUNTIME_LOG" 2>&1 &
  printf '%s\n' "$!" > "$RUNTIME_PID_FILE"
  wait_for "$RUNTIME_LOG" "IPC listening" 800 || {
    log "FATAL: runtime did not bind"
    tail -20 "$RUNTIME_LOG"
    stop_tracked "$RUNTIME_PID_FILE" "runtime" "$RUNTIME_BIN" || true
    exit 2
  }
  log "starting sbcl-agent → $SBCL_LOG"
  nohup sbcl --noinform --disable-debugger --load "$REPO/src/core/boot.lisp" --eval '(harmonia:start)' >"$SBCL_LOG" 2>&1 &
  printf '%s\n' "$!" > "$SBCL_PID_FILE"
  wait_for "$SBCL_LOG" "Actor system started" 1200 || {
    log "FATAL: sbcl-agent did not reach run loop"
    tail -25 "$SBCL_LOG"
    cmd_teardown
    exit 2
  }
  wait_for "$RUNTIME_LOG" "frontend-tui spawned" 400 || true
  log "bringup complete. TUI socket: $TUI_SOCK"
}

cmd_smoke() {
  local prompt="${1:-What is 2 plus 2? Answer in one short sentence.}"
  [ -S "$TUI_SOCK" ] || { log "FATAL: TUI socket $TUI_SOCK absent — run bringup"; exit 2; }
  python3 - "$TUI_SOCK" "$prompt" <<'PY'
import socket, sys, time
sock, prompt = sys.argv[1], sys.argv[2]
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM); s.settimeout(50)
s.connect(sock); s.sendall((prompt + "\n").encode())
buf = b""; t0 = time.time()
while time.time() - t0 < 45:
    try: d = s.recv(4096)
    except socket.timeout: break
    if not d: break
    buf += d
    if b"\n" in buf and time.time() - t0 > 2: break
resp = buf.decode("utf-8", "replace").strip()
s.close()
print("PROMPT:", prompt)
print("REPLY :", resp)
bad = (not resp) or resp == "NIL" or resp.lstrip().startswith("(:error") or resp.lstrip().startswith("(:unknown")
sys.exit(1 if bad else 0)
PY
  local rc=$?
  [ "$rc" -eq 0 ] && log "SMOKE PASS" || log "SMOKE FAIL (rc=$rc)"
  return $rc
}

cmd_repl_test() {
  mkdir -p "$DEV"
  # Requires a runtime up for healthy providers + memory IPC.
  grep -q "IPC listening" "$RUNTIME_LOG" 2>/dev/null || cmd_bringup >/dev/null 2>&1 || true
  HARMONIA_LOG_LEVEL=warn sbcl --noinform --disable-debugger --load "$REPO/src/core/boot.lisp" \
    --eval '(harmonia:start :run-loop nil)' \
    --eval '(progn
      (let ((fail 0))
        (dolist (p (list "What is 2 plus 2? Answer in one short sentence."
                         "What operating system am I running? Use a shell command."))
          (let ((r (harmonia::%orchestrate-repl p)))
            (format t "~%PROMPT: ~A~%REPLY : [~A]~%" p r)
            (when (or (null r) (harmonia::%error-form-p (princ-to-string r))) (incf fail))))
        (sb-ext:exit :code (if (zerop fail) 0 1))))' 2>&1 \
    | grep -vE "caught WARNING|compilation unit|undefined|See also|ANSI|^; |^;$|in: |file:|\[(INFO|WARN|DEBUG)\]"
}

cmd_status() {
  local pid
  if [ -f "$RUNTIME_PID_FILE" ]; then
    read -r pid < "$RUNTIME_PID_FILE" || pid=""
    kill -0 "$pid" 2>/dev/null && echo "runtime: pid=$pid (tracked)" || echo "runtime: stale pid file"
  else
    echo "runtime: not tracked"
  fi
  if [ -f "$SBCL_PID_FILE" ]; then
    read -r pid < "$SBCL_PID_FILE" || pid=""
    kill -0 "$pid" 2>/dev/null && echo "sbcl-agent: pid=$pid (tracked)" || echo "sbcl-agent: stale pid file"
  else
    echo "sbcl-agent: not tracked"
  fi
  [ -S "$TUI_SOCK" ] && echo "tui socket: $TUI_SOCK (present)" || echo "tui socket: absent"
}

case "${1:-}" in
  bringup)   cmd_bringup ;;
  smoke)     shift; cmd_smoke "$@" ;;
  repl-test) cmd_repl_test ;;
  teardown)  cmd_teardown ;;
  status)    cmd_status ;;
  *) echo "usage: $0 {bringup|smoke [prompt]|repl-test|teardown|status}"; exit 1 ;;
esac
