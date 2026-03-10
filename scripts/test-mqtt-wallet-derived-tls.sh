#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WALLET_ROOT="$(cd "$ROOT_DIR/../../marketplace/harmoniis-wallet" && pwd)"
HRMW_BIN="${HRMW_BIN:-$WALLET_ROOT/target/debug/hrmw}"
TOPIC="${HARMONIA_TEST_MQTT_TOPIC:-harmonia/test/alice-bob-$(date +%s)-$$}"

find_mosquitto_bin() {
  if command -v mosquitto >/dev/null 2>&1; then
    command -v mosquitto
    return
  fi
  if [[ -x "/opt/homebrew/sbin/mosquitto" ]]; then
    echo "/opt/homebrew/sbin/mosquitto"
    return
  fi
  if [[ -x "/usr/local/opt/mosquitto/sbin/mosquitto" ]]; then
    echo "/usr/local/opt/mosquitto/sbin/mosquitto"
    return
  fi
  return 1
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

TMPDIR_TEST="$(mktemp -d /tmp/harmonia-mqtt-wallet-derived-XXXXXX)"
cleanup() {
  if [[ -n "${MOSQ_PID:-}" ]]; then
    kill "$MOSQ_PID" >/dev/null 2>&1 || true
    wait "$MOSQ_PID" >/dev/null 2>&1 || true
  fi
  if [[ "${KEEP_TMPDIR:-0}" != "1" ]]; then
    rm -rf "$TMPDIR_TEST" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

require_cmd openssl
require_cmd python3
MOSQ_BIN="$(find_mosquitto_bin)"

echo "[1/9] build hrmw if needed"
if [[ ! -x "$HRMW_BIN" ]]; then
  (
    cd "$WALLET_ROOT"
    cargo build --bin hrmw
  )
fi

echo "[2/9] build Harmonia MQTT/gateway release dylibs"
(
  cd "$ROOT_DIR"
  cargo build --release \
    -p harmonia-gateway \
    -p harmonia-mqtt-client \
    -p harmonia-vault \
    -p harmonia-config-store
)

ALICE_DIR="$TMPDIR_TEST/alice-wallet"
BOB_DIR="$TMPDIR_TEST/bob-wallet"
mkdir -p "$ALICE_DIR" "$BOB_DIR"
ALICE_WALLET="$ALICE_DIR/master.db"
BOB_WALLET="$BOB_DIR/master.db"

echo "[3/9] create dedicated test wallets"
"$HRMW_BIN" setup --wallet "$ALICE_WALLET" --password-manager off >/tmp/hrmw-alice-setup.out 2>&1
"$HRMW_BIN" setup --wallet "$BOB_WALLET" --password-manager off >/tmp/hrmw-bob-setup.out 2>&1
ALICE_INFO="$("$HRMW_BIN" info --wallet "$ALICE_WALLET")"
BOB_INFO="$("$HRMW_BIN" info --wallet "$BOB_WALLET")"
ALICE_RGB_FP="$(printf '%s\n' "$ALICE_INFO" | awk -F': ' '/^RGB fingerprint:/ {print $2; exit}')"
BOB_RGB_FP="$(printf '%s\n' "$BOB_INFO" | awk -F': ' '/^RGB fingerprint:/ {print $2; exit}')"
if [[ -z "$ALICE_RGB_FP" || -z "$BOB_RGB_FP" ]]; then
  echo "failed to resolve wallet fingerprints" >&2
  exit 1
fi

echo "[4/9] derive labeled vault MQTT identities with hrmw"
ALICE_VAULT_OUT="$("$HRMW_BIN" key vault-new --wallet "$ALICE_WALLET" --label harmonia-agent-bob)"
BOB_VAULT_OUT="$("$HRMW_BIN" key vault-new --wallet "$BOB_WALLET" --label mqtt-client-alice)"
ALICE_MQTT_FP="$(printf '%s\n' "$ALICE_VAULT_OUT" | awk -F': ' '/^Vault public key:/ {print $2; exit}')"
BOB_MQTT_FP="$(printf '%s\n' "$BOB_VAULT_OUT" | awk -F': ' '/^Vault public key:/ {print $2; exit}')"
ALICE_SLOT_IDX="$(printf '%s\n' "$ALICE_VAULT_OUT" | awk -F': ' '/^Vault index:/ {print $2; exit}')"
BOB_SLOT_IDX="$(printf '%s\n' "$BOB_VAULT_OUT" | awk -F': ' '/^Vault index:/ {print $2; exit}')"
if [[ -z "$ALICE_MQTT_FP" || -z "$BOB_MQTT_FP" ]]; then
  echo "failed to resolve vault-derived mqtt fingerprints" >&2
  exit 1
fi
"$HRMW_BIN" key vault-export --wallet "$ALICE_WALLET" --label harmonia-agent-bob --output "$TMPDIR_TEST/alice.key.pem" >/tmp/hrmw-alice-vault-export.out
"$HRMW_BIN" key vault-export --wallet "$BOB_WALLET" --label mqtt-client-alice --output "$TMPDIR_TEST/bob.key.pem" >/tmp/hrmw-bob-vault-export.out

echo "[5/9] mint Bob/Alice TLS certificates from wallet-derived keys"
openssl req -new -x509 \
  -key "$TMPDIR_TEST/bob.key.pem" \
  -out "$TMPDIR_TEST/bob-ca.crt" \
  -days 1 \
  -subj "/CN=bob-agent-mqtt-ca" \
  -addext "basicConstraints=critical,CA:TRUE" \
  -addext "keyUsage=critical,digitalSignature,keyCertSign" \
  >/dev/null 2>&1
openssl req -new \
  -key "$TMPDIR_TEST/bob.key.pem" \
  -out "$TMPDIR_TEST/bob.csr" \
  -subj "/CN=bob-agent-mqtt" >/dev/null 2>&1
cat > "$TMPDIR_TEST/bob-server.ext" <<EOF
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature
extendedKeyUsage=serverAuth,clientAuth
subjectAltName=DNS:localhost,IP:127.0.0.1
EOF
openssl x509 -req \
  -in "$TMPDIR_TEST/bob.csr" \
  -CA "$TMPDIR_TEST/bob-ca.crt" \
  -CAkey "$TMPDIR_TEST/bob.key.pem" \
  -CAcreateserial \
  -out "$TMPDIR_TEST/bob.crt" \
  -days 1 \
  -extfile "$TMPDIR_TEST/bob-server.ext" >/dev/null 2>&1
openssl req -new \
  -key "$TMPDIR_TEST/alice.key.pem" \
  -out "$TMPDIR_TEST/alice.csr" \
  -subj "/CN=alice-mobile-mqtt" >/dev/null 2>&1
cat > "$TMPDIR_TEST/alice-client.ext" <<EOF
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature
extendedKeyUsage=clientAuth
EOF
openssl x509 -req \
  -in "$TMPDIR_TEST/alice.csr" \
  -CA "$TMPDIR_TEST/bob-ca.crt" \
  -CAkey "$TMPDIR_TEST/bob.key.pem" \
  -CAcreateserial \
  -out "$TMPDIR_TEST/alice.crt" \
  -days 1 \
  -extfile "$TMPDIR_TEST/alice-client.ext" >/dev/null 2>&1

echo "[6/9] start Bob-owned local mTLS broker"
cat > "$TMPDIR_TEST/mosquitto.conf" <<EOF
listener 8883 localhost
persistence false
allow_anonymous false
cafile $TMPDIR_TEST/bob-ca.crt
certfile $TMPDIR_TEST/bob.crt
keyfile $TMPDIR_TEST/bob.key.pem
require_certificate true
use_identity_as_username true
EOF
"$MOSQ_BIN" -c "$TMPDIR_TEST/mosquitto.conf" -v > "$TMPDIR_TEST/mosquitto.log" 2>&1 &
MOSQ_PID=$!
sleep 1

echo "[7/9] prove Alice -> Bob typed ingress through Harmonia gateway"
export HARMONIA_ENV=test
export HARMONIA_STATE_ROOT="$TMPDIR_TEST/bob-state"
export HARMONIA_VAULT_WALLET_DB="$BOB_WALLET"
export HARMONIA_LIB_DIR="$ROOT_DIR/target/release"
export HARMONIA_MQTT_BROKER="localhost:8883"
export HARMONIA_MQTT_TLS=1
export HARMONIA_MQTT_CA_CERT="$TMPDIR_TEST/bob-ca.crt"
export HARMONIA_MQTT_CLIENT_CERT="$TMPDIR_TEST/bob.crt"
export HARMONIA_MQTT_CLIENT_KEY="$TMPDIR_TEST/bob.key.pem"
export HARMONIA_TEST_TOPIC="$TOPIC"
export HARMONIA_ALICE_CERT="$TMPDIR_TEST/alice.crt"
export HARMONIA_ALICE_KEY="$TMPDIR_TEST/alice.key.pem"
export HARMONIA_BOB_CA="$TMPDIR_TEST/bob-ca.crt"
export HARMONIA_BOB_CERT="$TMPDIR_TEST/bob.crt"
export HARMONIA_BOB_KEY="$TMPDIR_TEST/bob.key.pem"
export HARMONIA_BOB_MQTT_FP="$BOB_MQTT_FP"
export HARMONIA_ALICE_MQTT_FP="$ALICE_MQTT_FP"
python3 <<'PY'
import ctypes
import json
import os
import subprocess
import sys
import time
from pathlib import Path

root = Path(os.environ["HARMONIA_LIB_DIR"])
topic = os.environ["HARMONIA_TEST_TOPIC"]
bob_fp = os.environ["HARMONIA_BOB_MQTT_FP"]
alice_fp = os.environ["HARMONIA_ALICE_MQTT_FP"]

vault = ctypes.CDLL(str(root / "libharmonia_vault.dylib"))
vault.harmonia_vault_init.restype = ctypes.c_int
vault.harmonia_vault_set_secret.argtypes = [ctypes.c_char_p, ctypes.c_char_p]
vault.harmonia_vault_set_secret.restype = ctypes.c_int
vault.harmonia_vault_last_error.restype = ctypes.c_void_p
vault.harmonia_vault_free_string.argtypes = [ctypes.c_void_p]

gateway = ctypes.CDLL(str(root / "libharmonia_gateway.dylib"))
gateway.harmonia_gateway_init.restype = ctypes.c_int
gateway.harmonia_gateway_register.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p]
gateway.harmonia_gateway_register.restype = ctypes.c_int
gateway.harmonia_gateway_poll.restype = ctypes.c_void_p
gateway.harmonia_gateway_send.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p]
gateway.harmonia_gateway_send.restype = ctypes.c_int
gateway.harmonia_gateway_last_error.restype = ctypes.c_void_p
gateway.harmonia_gateway_shutdown.restype = ctypes.c_int
gateway.harmonia_gateway_free_string.argtypes = [ctypes.c_void_p]

mqtt = ctypes.CDLL(str(root / "libharmonia_mqtt_client.dylib"))
mqtt.harmonia_mqtt_client_make_envelope.argtypes = [
    ctypes.c_char_p,
    ctypes.c_char_p,
    ctypes.c_char_p,
    ctypes.c_char_p,
    ctypes.c_char_p,
]
mqtt.harmonia_mqtt_client_make_envelope.restype = ctypes.c_void_p
mqtt.harmonia_mqtt_client_publish.argtypes = [ctypes.c_char_p, ctypes.c_char_p]
mqtt.harmonia_mqtt_client_publish.restype = ctypes.c_int
mqtt.harmonia_mqtt_client_poll.argtypes = [ctypes.c_char_p]
mqtt.harmonia_mqtt_client_poll.restype = ctypes.c_void_p
mqtt.harmonia_mqtt_client_free_string.argtypes = [ctypes.c_void_p]
mqtt.harmonia_mqtt_client_last_error.restype = ctypes.c_void_p

def read_ptr(lib, ptr):
    if not ptr:
        return ""
    text = ctypes.string_at(ptr).decode("utf-8", errors="replace")
    if lib is vault:
        vault.harmonia_vault_free_string(ptr)
    elif lib is gateway:
        gateway.harmonia_gateway_free_string(ptr)
    else:
        mqtt.harmonia_mqtt_client_free_string(ptr)
    return text

def expect_rc(rc, name, err_fn):
    if rc != 0:
        raise SystemExit(f"{name} failed rc={rc} err={err_fn()}")

def vault_error():
    return read_ptr(vault, vault.harmonia_vault_last_error())

def gateway_error():
    return read_ptr(gateway, gateway.harmonia_gateway_last_error())

def mqtt_error():
    return read_ptr(mqtt, mqtt.harmonia_mqtt_client_last_error())

expect_rc(vault.harmonia_vault_init(), "vault init", vault_error)
expect_rc(
    vault.harmonia_vault_set_secret(b"mqtt_agent_fp", bob_fp.encode("utf-8")),
    "vault set mqtt_agent_fp",
    vault_error,
)
expect_rc(gateway.harmonia_gateway_init(), "gateway init", gateway_error)
config = f'(:name "mqtt" :topics ("{topic}") :capabilities (:a2ui "1.0"))'
expect_rc(
    gateway.harmonia_gateway_register(
        b"mqtt",
        str(root / "libharmonia_mqtt_client.dylib").encode("utf-8"),
        config.encode("utf-8"),
        b"authenticated",
    ),
    "gateway register mqtt",
    gateway_error,
)
time.sleep(1.5)
body = json.dumps({"text": "hello from alice mobile mqtt"})
env_ptr = mqtt.harmonia_mqtt_client_make_envelope(
    b"message",
    b"message.text",
    bob_fp.encode("utf-8"),
    alice_fp.encode("utf-8"),
    body.encode("utf-8"),
)
if not env_ptr:
    raise SystemExit(f"mqtt make envelope failed: {mqtt_error()}")
alice_payload = read_ptr(mqtt, env_ptr)
os.environ["HARMONIA_MQTT_CA_CERT"] = os.environ["HARMONIA_BOB_CA"]
os.environ["HARMONIA_MQTT_CLIENT_CERT"] = os.environ["HARMONIA_ALICE_CERT"]
os.environ["HARMONIA_MQTT_CLIENT_KEY"] = os.environ["HARMONIA_ALICE_KEY"]
expect_rc(
    mqtt.harmonia_mqtt_client_publish(topic.encode("utf-8"), alice_payload.encode("utf-8")),
    "alice mqtt publish",
    mqtt_error,
)
os.environ["HARMONIA_MQTT_CLIENT_CERT"] = os.environ["HARMONIA_BOB_CERT"]
os.environ["HARMONIA_MQTT_CLIENT_KEY"] = os.environ["HARMONIA_BOB_KEY"]

poll_text = ""
deadline = time.time() + 12
while time.time() < deadline:
    ptr = gateway.harmonia_gateway_poll()
    poll_text = read_ptr(gateway, ptr)
    if poll_text and poll_text != "nil":
        break
    time.sleep(0.5)
if not poll_text or poll_text == "nil":
    raise SystemExit("gateway poll timed out waiting for Alice->Bob ingress")

required = [
    ':type-name "message.text"',
    f':origin-fp "{alice_fp}"',
    f':agent-fp "{bob_fp}"',
    ':label "authenticated"',
    ':fingerprint-valid t',
    ':kind "mqtt"',
    topic,
]
missing = [item for item in required if item not in poll_text]
if missing:
    raise SystemExit(f"gateway poll missing expected typed fields: {missing}\n{poll_text}")
print("GATEWAY_POLL_OK=1")
print(f"GATEWAY_POLL={poll_text}")

reply_body = json.dumps({"text": "bob received alice over mqtt"})
reply_ptr = mqtt.harmonia_mqtt_client_make_envelope(
    b"message",
    b"message.text",
    bob_fp.encode("utf-8"),
    alice_fp.encode("utf-8"),
    reply_body.encode("utf-8"),
)
if not reply_ptr:
    raise SystemExit(f"mqtt make reply envelope failed: {mqtt_error()}")
reply_payload = read_ptr(mqtt, reply_ptr)
expect_rc(
    gateway.harmonia_gateway_send(b"mqtt", topic.encode("utf-8"), reply_payload.encode("utf-8")),
    "gateway send mqtt reply",
    gateway_error,
)
Path(os.environ["HARMONIA_STATE_ROOT"]).mkdir(parents=True, exist_ok=True)
Path(os.environ["HARMONIA_STATE_ROOT"], "gateway-poll.sexp").write_text(poll_text, encoding="utf-8")
Path(os.environ["HARMONIA_STATE_ROOT"], "bob-reply.json").write_text(reply_payload, encoding="utf-8")
os.environ["HARMONIA_MQTT_CLIENT_CERT"] = os.environ["HARMONIA_ALICE_CERT"]
os.environ["HARMONIA_MQTT_CLIENT_KEY"] = os.environ["HARMONIA_ALICE_KEY"]
reply_ptr = mqtt.harmonia_mqtt_client_poll(topic.encode("utf-8"))
if not reply_ptr:
    raise SystemExit(f"alice mqtt poll failed: {mqtt_error()}")
alice_reply = read_ptr(mqtt, reply_ptr)
if bob_fp not in alice_reply or alice_fp not in alice_reply:
    raise SystemExit(f"alice reply missing expected vault fingerprints: {alice_reply}")
Path(os.environ["HARMONIA_STATE_ROOT"], "alice-recv.json").write_text(alice_reply, encoding="utf-8")
gateway.harmonia_gateway_shutdown()
PY

echo "[8/9] verify Bob -> Alice reply over the same broker channel"
ALICE_REPLY="$TMPDIR_TEST/bob-state/alice-recv.json"
if [[ ! -s "$ALICE_REPLY" ]]; then
  echo "alice reply file missing: $ALICE_REPLY" >&2
  exit 1
fi

echo "[9/9] done"
echo "TOPIC=$TOPIC"
echo "WORKDIR=$TMPDIR_TEST"
echo "ALICE_RGB_FP=$ALICE_RGB_FP"
echo "BOB_RGB_FP=$BOB_RGB_FP"
echo "ALICE_MQTT_LABEL=harmonia-agent-bob"
echo "ALICE_MQTT_SLOT=$ALICE_SLOT_IDX"
echo "ALICE_MQTT_FP=$ALICE_MQTT_FP"
echo "BOB_MQTT_LABEL=mqtt-client-alice"
echo "BOB_MQTT_SLOT=$BOB_SLOT_IDX"
echo "BOB_MQTT_FP=$BOB_MQTT_FP"
echo "GATEWAY_POLL_FILE=$TMPDIR_TEST/bob-state/gateway-poll.sexp"
echo "ALICE_REPLY_FILE=$ALICE_REPLY"
