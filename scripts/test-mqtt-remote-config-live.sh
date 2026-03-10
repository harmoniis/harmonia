#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WALLET_ROOT="$(cd "$ROOT_DIR/../../marketplace/harmoniis-wallet" && pwd)"
HRMW_BIN="${HRMW_BIN:-$WALLET_ROOT/target/debug/hrmw}"
HARMONIA_BIN="${HARMONIA_BIN:-$ROOT_DIR/target/debug/harmonia}"
REMOTE_AGENT_URL="${HARMONIA_REMOTE_AGENT_URL:-}"
REMOTE_PUSH_DEVICES_URL="${HARMONIA_REMOTE_PUSH_DEVICES_URL:-}"
REMOTE_PUSH_WEBHOOK_URL="${HARMONIA_REMOTE_PUSH_WEBHOOK_URL:-}"
BROKER_PORT="${HARMONIA_TEST_MQTT_PORT:-18883}"
DEVICE_ID="${HARMONIA_TEST_DEVICE_ID:-alice-ios-1}"
TMPDIR_TEST="$(mktemp -d /tmp/harmonia-mqtt-remote-live-XXXXXX)"

if [[ -z "$REMOTE_AGENT_URL" ]]; then
  echo "HARMONIA_REMOTE_AGENT_URL is required" >&2
  exit 1
fi
if [[ -z "$REMOTE_PUSH_DEVICES_URL" ]]; then
  REMOTE_PUSH_DEVICES_URL="${REMOTE_AGENT_URL%/agent}/push/devices"
fi
if [[ -z "$REMOTE_PUSH_WEBHOOK_URL" ]]; then
  REMOTE_PUSH_WEBHOOK_URL="${REMOTE_AGENT_URL%/agent}/webhooks/push"
fi

find_mosquitto_client() {
  local name="$1"
  if command -v "$name" >/dev/null 2>&1; then
    command -v "$name"
    return
  fi
  for path in \
    "/opt/homebrew/bin/$name" \
    "/usr/local/bin/$name" \
    "/opt/homebrew/sbin/$name" \
    "/usr/local/sbin/$name"; do
    if [[ -x "$path" ]]; then
      echo "$path"
      return
    fi
  done
  return 1
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

cleanup() {
  if [[ -n "${BROKER_PID:-}" ]]; then
    kill "$BROKER_PID" >/dev/null 2>&1 || true
    wait "$BROKER_PID" >/dev/null 2>&1 || true
  fi
  if [[ "${KEEP_TMPDIR:-0}" != "1" ]]; then
    rm -rf "$TMPDIR_TEST" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

require_cmd openssl
require_cmd python3
MOSQ_PUB="$(find_mosquitto_client mosquitto_pub)"
MOSQ_SUB="$(find_mosquitto_client mosquitto_sub)"

echo "[1/8] build wallet and Harmonia binaries"
if [[ ! -x "$HRMW_BIN" ]]; then
  (cd "$WALLET_ROOT" && cargo build --bin hrmw)
fi
(
  cd "$ROOT_DIR"
  cargo build -p harmonia -p harmonia-gateway -p harmonia-mqtt-client -p harmonia-vault -p harmonia-config-store
)

ALICE_DIR="$TMPDIR_TEST/alice-wallet"
BOB_DIR="$TMPDIR_TEST/bob-wallet"
mkdir -p "$ALICE_DIR" "$BOB_DIR"
ALICE_WALLET="$ALICE_DIR/master.db"
BOB_WALLET="$BOB_DIR/master.db"


echo "[2/8] create dedicated Alice/Bob wallets"
"$HRMW_BIN" setup --wallet "$ALICE_WALLET" --password-manager off >/tmp/hrmw-alice-remote-setup.out 2>&1
"$HRMW_BIN" setup --wallet "$BOB_WALLET" --password-manager off >/tmp/hrmw-bob-remote-setup.out 2>&1
ALICE_VAULT_OUT="$($HRMW_BIN key vault-new --wallet "$ALICE_WALLET" --label harmonia-agent-bob)"
BOB_VAULT_OUT="$($HRMW_BIN key vault-new --wallet "$BOB_WALLET" --label mqtt-client-alice)"
ALICE_MQTT_FP="$(printf '%s\n' "$ALICE_VAULT_OUT" | awk -F': ' '/^Vault public key:/ {print $2; exit}')"
BOB_MQTT_FP="$(printf '%s\n' "$BOB_VAULT_OUT" | awk -F': ' '/^Vault public key:/ {print $2; exit}')"
if [[ -z "$ALICE_MQTT_FP" || -z "$BOB_MQTT_FP" ]]; then
  echo "failed to derive wallet-backed MQTT identities" >&2
  exit 1
fi
"$HRMW_BIN" key vault-export --wallet "$ALICE_WALLET" --label harmonia-agent-bob --output "$TMPDIR_TEST/alice.key.pem" >/tmp/hrmw-alice-remote-export.out
"$HRMW_BIN" key vault-export --wallet "$BOB_WALLET" --label mqtt-client-alice --output "$TMPDIR_TEST/bob.key.pem" >/tmp/hrmw-bob-remote-export.out


echo "[3/8] generate wallet-bound mTLS certificates"
openssl genpkey -algorithm Ed25519 -out "$TMPDIR_TEST/bob-ca.key.pem" >/dev/null 2>&1
openssl req -new -x509 \
  -key "$TMPDIR_TEST/bob-ca.key.pem" \
  -out "$TMPDIR_TEST/bob-ca.crt" \
  -days 1 \
  -subj "/CN=harmonia-mqtt-ca" \
  -addext "basicConstraints=critical,CA:TRUE" \
  -addext "keyUsage=critical,digitalSignature,keyCertSign" \
  >/dev/null 2>&1
openssl req -new \
  -key "$TMPDIR_TEST/bob.key.pem" \
  -out "$TMPDIR_TEST/bob.csr" \
  -subj "/CN=$BOB_MQTT_FP" >/dev/null 2>&1
cat > "$TMPDIR_TEST/bob-server.ext" <<EOF2
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature
extendedKeyUsage=serverAuth,clientAuth
subjectAltName=DNS:localhost,IP:127.0.0.1
EOF2
openssl x509 -req \
  -in "$TMPDIR_TEST/bob.csr" \
  -CA "$TMPDIR_TEST/bob-ca.crt" \
  -CAkey "$TMPDIR_TEST/bob-ca.key.pem" \
  -CAcreateserial \
  -out "$TMPDIR_TEST/bob.crt" \
  -days 1 \
  -extfile "$TMPDIR_TEST/bob-server.ext" >/dev/null 2>&1
cat "$TMPDIR_TEST/bob.crt" "$TMPDIR_TEST/bob-ca.crt" > "$TMPDIR_TEST/bob.chain.crt"
openssl req -new \
  -key "$TMPDIR_TEST/alice.key.pem" \
  -out "$TMPDIR_TEST/alice.csr" \
  -subj "/CN=$ALICE_MQTT_FP" >/dev/null 2>&1
cat > "$TMPDIR_TEST/alice-client.ext" <<EOF2
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature
extendedKeyUsage=clientAuth
EOF2
openssl x509 -req \
  -in "$TMPDIR_TEST/alice.csr" \
  -CA "$TMPDIR_TEST/bob-ca.crt" \
  -CAkey "$TMPDIR_TEST/bob-ca.key.pem" \
  -CAcreateserial \
  -out "$TMPDIR_TEST/alice.crt" \
  -days 1 \
  -extfile "$TMPDIR_TEST/alice-client.ext" >/dev/null 2>&1

TOPIC_BASE="harmonia/$BOB_MQTT_FP/device/$DEVICE_ID"
STATE_ROOT="$TMPDIR_TEST/bob-state"
mkdir -p "$STATE_ROOT"
export REMOTE_AGENT_URL REMOTE_PUSH_DEVICES_URL REMOTE_PUSH_WEBHOOK_URL
export ALICE_WALLET BOB_WALLET ALICE_MQTT_FP BOB_MQTT_FP DEVICE_ID BROKER_PORT TMPDIR_TEST STATE_ROOT TOPIC_BASE HRMW_BIN
export HARMONIA_HRMW_BIN="$HRMW_BIN"

echo "[4/8] seed remote agent config and Alice push-device state on deployed backend"
python3 <<'PY'
import hashlib
import json
import os
import ssl
import subprocess
import sys
import urllib.request

import certifi

agent_url = os.environ["REMOTE_AGENT_URL"]
push_devices_url = os.environ["REMOTE_PUSH_DEVICES_URL"]
push_webhook_url = os.environ["REMOTE_PUSH_WEBHOOK_URL"]
alice_wallet = os.environ["ALICE_WALLET"]
bob_wallet = os.environ["BOB_WALLET"]
alice_fp = os.environ["ALICE_MQTT_FP"].upper()
bob_fp = os.environ["BOB_MQTT_FP"].upper()
device_id = os.environ["DEVICE_ID"]
broker_port = int(os.environ["BROKER_PORT"])


def sign(wallet, label, message):
    proc = subprocess.run(
        [
            os.environ.get("HRMW_BIN", "hrmw"),
            "key",
            "vault-sign",
            "--wallet",
            wallet,
            "--label",
            label,
            "--message",
            message,
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    fields = {}
    for line in proc.stdout.splitlines():
        if ":" in line:
            key, value = line.split(":", 1)
            fields[key.strip()] = value.strip()
    return fields["Vault public key"], fields["Signature"]


def canonical_json(value):
    return json.dumps(value, separators=(",", ":"), sort_keys=True)


def post(url, query, variables):
    payload = json.dumps({"query": query, "variables": variables}, separators=(",", ":")).encode()
    req = urllib.request.Request(url, data=payload, headers={"Content-Type": "application/json"})
    context = ssl.create_default_context(cafile=certifi.where())
    with urllib.request.urlopen(req, timeout=20, context=context) as resp:
        data = json.loads(resp.read().decode())
    if data.get("errors"):
        raise SystemExit(f"GraphQL error from {url}: {data['errors']}")
    return data.get("data", {})

agent_hash_payload = {
    "mqtt_domain": "127.0.0.1",
    "mqtt_port": broker_port,
    "mqtt_tls_required": True,
    "broker_mode": "embedded",
    "trusted_client_fingerprints": [alice_fp],
    "push_webhook_url": push_webhook_url,
    "push_webhook_token": None,
    "config_json": json.dumps({"scenario": "remote-config-live-test"}, separators=(",", ":")),
}
agent_hash = hashlib.sha256(
    canonical_json(agent_hash_payload).encode()
).hexdigest()
agent_message = f"harmonia:agent-config:set:{bob_fp}:{agent_hash}"
bob_public_key, bob_signature = sign(bob_wallet, "mqtt-client-alice", agent_message)
agent_graphql_input = {
    "fingerprint": bob_fp,
    "publicKey": bob_public_key,
    "signature": bob_signature,
    "mqttDomain": agent_hash_payload["mqtt_domain"],
    "mqttPort": agent_hash_payload["mqtt_port"],
    "mqttTlsRequired": agent_hash_payload["mqtt_tls_required"],
    "brokerMode": agent_hash_payload["broker_mode"],
    "trustedClientFingerprints": agent_hash_payload["trusted_client_fingerprints"],
    "pushWebhookUrl": agent_hash_payload["push_webhook_url"],
    "pushWebhookToken": agent_hash_payload["push_webhook_token"],
    "configJson": agent_hash_payload["config_json"],
}
post(
    agent_url,
    "mutation UpsertConfig($input: GqlHarmoniaAgentConfigInput!) { upsertConfig(input: $input) { fingerprint mqttDomain mqttPort trustedClientFingerprints pushWebhookUrl trustedDevices { deviceId fingerprint mqttIdentityFingerprint } } }",
    {"input": agent_graphql_input},
)

device_hash_payload = {
    "device_id": device_id,
    "label": "Alice iPhone",
    "platform": "ios",
    "push_token": "dummy-token",
    "sns_target_arn": None,
    "push_data_json": json.dumps({"apns-topic": "com.harmoniis.mobile"}, separators=(",", ":")),
    "mqtt_identity_fingerprint": alice_fp,
    "trusted_agent_fingerprints": [bob_fp],
    "config_json": json.dumps({"scenario": "remote-config-live-test"}, separators=(",", ":")),
}
device_hash = hashlib.sha256(
    canonical_json(device_hash_payload).encode()
).hexdigest()
device_message = f"harmonia:push-device:set:{alice_fp}:{device_id}:{device_hash}"
alice_public_key, alice_signature = sign(alice_wallet, "harmonia-agent-bob", device_message)
device_graphql_input = {
    "fingerprint": alice_fp,
    "publicKey": alice_public_key,
    "signature": alice_signature,
    "deviceId": device_hash_payload["device_id"],
    "label": device_hash_payload["label"],
    "platform": device_hash_payload["platform"],
    "pushToken": device_hash_payload["push_token"],
    "snsTargetArn": device_hash_payload["sns_target_arn"],
    "pushDataJson": device_hash_payload["push_data_json"],
    "mqttIdentityFingerprint": device_hash_payload["mqtt_identity_fingerprint"],
    "trustedAgentFingerprints": device_hash_payload["trusted_agent_fingerprints"],
    "configJson": device_hash_payload["config_json"],
}
post(
    push_devices_url,
    "mutation UpsertDevice($input: GqlHarmoniaPushDeviceInput!) { upsertDevice(input: $input) { fingerprint deviceId mqttIdentityFingerprint trustedAgentFingerprints pushToken } }",
    {"input": device_graphql_input},
)
requested_at = "live-test"
_, bob_read_signature = sign(bob_wallet, "mqtt-client-alice", f"harmonia:agent-config:get:{bob_fp}:{requested_at}")
config_data = post(
    agent_url,
    "query VerifyConfig($fingerprint: String!, $publicKey: String!, $signature: String!, $requestedAt: String!) { config(fingerprint: $fingerprint, publicKey: $publicKey, signature: $signature, requestedAt: $requestedAt) { fingerprint mqttDomain mqttPort trustedClientFingerprints trustedDevices { deviceId fingerprint mqttIdentityFingerprint } } }",
    {
        "fingerprint": bob_fp,
        "publicKey": bob_public_key,
        "signature": bob_read_signature,
        "requestedAt": requested_at,
    },
)
config = config_data["config"]
if config["fingerprint"].upper() != bob_fp:
    raise SystemExit(f"unexpected agent config fingerprint: {config}")
if not any((item.get("mqttIdentityFingerprint") or item.get("fingerprint", "")).upper() == alice_fp for item in config.get("trustedDevices", [])):
    raise SystemExit(f"alice device missing from trustedDevices: {config}")
print("REMOTE_CONFIG_OK=1")
PY

echo "[5/8] seed local Harmonia config-store and start embedded broker"
python3 <<'PY'
import os
import sqlite3
import time
from pathlib import Path

state_root = Path(os.environ["STATE_ROOT"])
config_db = state_root / "config.db"
config_db.parent.mkdir(parents=True, exist_ok=True)
conn = sqlite3.connect(config_db)
conn.executescript(
    """
    PRAGMA journal_mode=WAL;
    PRAGMA synchronous=NORMAL;
    CREATE TABLE IF NOT EXISTS config_kv (
        scope TEXT NOT NULL,
        key TEXT NOT NULL,
        value TEXT NOT NULL,
        updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
        PRIMARY KEY(scope, key)
    );
    CREATE INDEX IF NOT EXISTS idx_config_updated_at ON config_kv(updated_at);
    CREATE TABLE IF NOT EXISTS config_meta (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
    """
)
entries = [
    ("mqtt-broker", "mode", "embedded"),
    ("mqtt-broker", "bind", f"127.0.0.1:{os.environ['BROKER_PORT']}"),
    ("mqtt-broker", "tls", "1"),
    ("mqtt-broker", "ca-cert", str(Path(os.environ["TMPDIR_TEST"]) / "bob-ca.crt")),
    ("mqtt-broker", "server-cert", str(Path(os.environ["TMPDIR_TEST"]) / "bob.chain.crt")),
    ("mqtt-broker", "server-key", str(Path(os.environ["TMPDIR_TEST"]) / "bob.key.pem")),
    ("mqtt-broker", "remote-config-url", os.environ["REMOTE_AGENT_URL"]),
    ("mqtt-broker", "remote-config-identity-label", "mqtt-client-alice"),
    ("mqtt-broker", "remote-config-refresh-seconds", "60"),
    ("mqtt-broker", "identity-public-key", os.environ["BOB_MQTT_FP"]),
    ("mqtt-frontend", "broker", f"127.0.0.1:{os.environ['BROKER_PORT']}"),
    ("mqtt-frontend", "tls", "1"),
    ("mqtt-frontend", "ca-cert", str(Path(os.environ["TMPDIR_TEST"]) / "bob-ca.crt")),
    ("mqtt-frontend", "client-cert", str(Path(os.environ["TMPDIR_TEST"]) / "bob.crt")),
    ("mqtt-frontend", "client-key", str(Path(os.environ["TMPDIR_TEST"]) / "bob.key.pem")),
    ("mqtt-frontend", "push-webhook-url", os.environ["REMOTE_PUSH_WEBHOOK_URL"]),
    ("mqtt-frontend", "trusted-client-fingerprints-json", "[]"),
    ("mqtt-frontend", "trusted-device-registry-json", "[]"),
]
now = int(time.time())
conn.executemany(
    "INSERT INTO config_kv(scope, key, value, updated_at) VALUES (?, ?, ?, ?) ON CONFLICT(scope, key) DO UPDATE SET value=excluded.value, updated_at=excluded.updated_at",
    [(scope, key, value, now) for scope, key, value in entries],
)
conn.commit()
conn.close()
PY

BROKER_LOG="$TMPDIR_TEST/broker.log"
HARMONIA_STATE_ROOT="$STATE_ROOT" \
HARMONIA_VAULT_WALLET_DB="$BOB_WALLET" \
HARMONIA_HRMW_BIN="$HRMW_BIN" \
HARMONIA_LIB_DIR="$ROOT_DIR/target/debug" \
"$HARMONIA_BIN" broker > "$BROKER_LOG" 2>&1 &
BROKER_PID=$!
for _ in $(seq 1 30); do
  if grep -q "MQTT broker listening" "$BROKER_LOG"; then
    break
  fi
  if ! kill -0 "$BROKER_PID" >/dev/null 2>&1; then
    echo "embedded broker exited unexpectedly" >&2
    cat "$BROKER_LOG" >&2 || true
    exit 1
  fi
  sleep 1
done
if ! grep -q "MQTT broker listening" "$BROKER_LOG"; then
  echo "broker did not become ready" >&2
  cat "$BROKER_LOG" >&2 || true
  exit 1
fi

echo "[6/8] prove Alice -> Bob MQTT ingress and Bob -> Alice online reply"
export HARMONIA_LIB_DIR="$ROOT_DIR/target/debug"
export HARMONIA_STATE_ROOT="$STATE_ROOT"
export HARMONIA_VAULT_WALLET_DB="$BOB_WALLET"
export MOSQ_PUB MOSQ_SUB
python3 <<'PY'
import ctypes
import json
import os
import sqlite3
import subprocess
import sys
import time
from pathlib import Path

root = Path(os.environ["HARMONIA_LIB_DIR"])
state_root = Path(os.environ["HARMONIA_STATE_ROOT"])
bob_fp = os.environ["BOB_MQTT_FP"].upper()
alice_fp = os.environ["ALICE_MQTT_FP"].upper()
device_id = os.environ["DEVICE_ID"]
topic_base = os.environ["TOPIC_BASE"]
message_topic = f"{topic_base}/messages"
connect_topic = f"{topic_base}/connect"
disconnect_topic = f"{topic_base}/disconnect"
mosq_pub = os.environ["MOSQ_PUB"]
mosq_sub = os.environ["MOSQ_SUB"]
ca = str(Path(os.environ["TMPDIR_TEST"]) / "bob-ca.crt")
alice_cert = str(Path(os.environ["TMPDIR_TEST"]) / "alice.crt")
alice_key = str(Path(os.environ["TMPDIR_TEST"]) / "alice.key.pem")

ext = ".dylib" if sys.platform == "darwin" else ".so"
vault = ctypes.CDLL(str(root / f"libharmonia_vault{ext}"))
gateway = ctypes.CDLL(str(root / f"libharmonia_gateway{ext}"))
mqtt = ctypes.CDLL(str(root / f"libharmonia_mqtt_client{ext}"))

vault.harmonia_vault_init.restype = ctypes.c_int
vault.harmonia_vault_set_secret.argtypes = [ctypes.c_char_p, ctypes.c_char_p]
vault.harmonia_vault_set_secret.restype = ctypes.c_int
vault.harmonia_vault_last_error.restype = ctypes.c_void_p
vault.harmonia_vault_free_string.argtypes = [ctypes.c_void_p]

gateway.harmonia_gateway_init.restype = ctypes.c_int
gateway.harmonia_gateway_register.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p]
gateway.harmonia_gateway_register.restype = ctypes.c_int
gateway.harmonia_gateway_poll.restype = ctypes.c_void_p
gateway.harmonia_gateway_send.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p]
gateway.harmonia_gateway_send.restype = ctypes.c_int
gateway.harmonia_gateway_last_error.restype = ctypes.c_void_p
gateway.harmonia_gateway_shutdown.restype = ctypes.c_int
gateway.harmonia_gateway_free_string.argtypes = [ctypes.c_void_p]

mqtt.harmonia_mqtt_client_make_envelope.argtypes = [
    ctypes.c_char_p,
    ctypes.c_char_p,
    ctypes.c_char_p,
    ctypes.c_char_p,
    ctypes.c_char_p,
]
mqtt.harmonia_mqtt_client_make_envelope.restype = ctypes.c_void_p
mqtt.harmonia_mqtt_client_last_error.restype = ctypes.c_void_p
mqtt.harmonia_mqtt_client_free_string.argtypes = [ctypes.c_void_p]


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


def mosquitto(*args, capture=False, timeout=15):
    if capture:
        proc = subprocess.run(args, capture_output=True, text=True, timeout=timeout)
        if proc.returncode != 0:
            raise SystemExit(f"command failed: {' '.join(args)}\nstdout={proc.stdout}\nstderr={proc.stderr}")
        return proc.stdout
    proc = subprocess.run(args, timeout=timeout)
    if proc.returncode != 0:
        raise SystemExit(f"command failed: {' '.join(args)}")
    return None


def make_envelope(text):
    payload = json.dumps({"text": text}).encode("utf-8")
    ptr = mqtt.harmonia_mqtt_client_make_envelope(
        b"message",
        b"message.text",
        bob_fp.encode("utf-8"),
        alice_fp.encode("utf-8"),
        payload,
    )
    if not ptr:
        raise SystemExit(f"mqtt make envelope failed: {mqtt_error()}")
    return read_ptr(mqtt, ptr)

expect_rc(vault.harmonia_vault_init(), "vault init", vault_error)
expect_rc(
    vault.harmonia_vault_set_secret(b"mqtt_agent_fp", bob_fp.encode("utf-8")),
    "vault set mqtt_agent_fp",
    vault_error,
)
expect_rc(gateway.harmonia_gateway_init(), "gateway init", gateway_error)
config = f'(:name "mqtt" :topics ("{topic_base}/#") :capabilities (:a2ui "1.0"))'
expect_rc(
    gateway.harmonia_gateway_register(
        b"mqtt",
        str(root / f"libharmonia_mqtt_client{ext}").encode("utf-8"),
        config.encode("utf-8"),
        b"authenticated",
    ),
    "gateway register mqtt",
    gateway_error,
)
time.sleep(2.0)

connect_payload = json.dumps(
    {
        "device_id": device_id,
        "owner_fingerprint": alice_fp,
        "platform": "ios",
        "platform_version": "18.0",
        "app_version": "1.0.0",
        "device_model": "iPhone",
        "capabilities": ["push", "a2ui"],
        "permissions_granted": ["push"],
        "a2ui_version": "1.0",
        "mqtt_identity_fingerprint": alice_fp,
    },
    separators=(",", ":"),
)
mosquitto(
    mosq_pub,
    "-h", "127.0.0.1",
    "-p", os.environ["BROKER_PORT"],
    "--cafile", ca,
    "--cert", alice_cert,
    "--key", alice_key,
    "-q", "1",
    "-t", connect_topic,
    "-m", connect_payload,
)
time.sleep(1.0)

alice_payload = make_envelope("hello from alice over remote-config broker")
mosquitto(
    mosq_pub,
    "-h", "127.0.0.1",
    "-p", os.environ["BROKER_PORT"],
    "--cafile", ca,
    "--cert", alice_cert,
    "--key", alice_key,
    "-q", "1",
    "-t", message_topic,
    "-m", alice_payload,
)

poll_text = ""
deadline = time.time() + 15
while time.time() < deadline:
    ptr = gateway.harmonia_gateway_poll()
    poll_text = read_ptr(gateway, ptr)
    if poll_text and poll_text != "nil" and "message.text" in poll_text:
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
    ':trusted-origin t',
]
missing = [item for item in required if item not in poll_text]
if missing:
    raise SystemExit(f"gateway poll missing expected typed fields: {missing}\n{poll_text}")
(state_root / "gateway-poll.sexp").write_text(poll_text, encoding="utf-8")

online_reply_path = state_root / "alice-online-reply.json"
sub_proc = subprocess.Popen(
    [
        mosq_sub,
        "-h", "127.0.0.1",
        "-p", os.environ["BROKER_PORT"],
        "--cafile", ca,
        "--cert", alice_cert,
        "--key", alice_key,
        "-q", "1",
        "-C", "1",
        "-t", message_topic,
    ],
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
    text=True,
)
time.sleep(1.0)
reply_payload = make_envelope("bob immediate reply over embedded rmqtt")
expect_rc(
    gateway.harmonia_gateway_send(b"mqtt", message_topic.encode("utf-8"), reply_payload.encode("utf-8")),
    "gateway send immediate reply",
    gateway_error,
)
stdout, stderr = sub_proc.communicate(timeout=15)
if sub_proc.returncode != 0:
    raise SystemExit(f"mosquitto_sub failed for online reply: {stderr}")
if bob_fp not in stdout or alice_fp not in stdout:
    raise SystemExit(f"alice did not receive expected online reply: {stdout}")
online_reply_path.write_text(stdout, encoding="utf-8")

mosquitto(
    mosq_pub,
    "-h", "127.0.0.1",
    "-p", os.environ["BROKER_PORT"],
    "--cafile", ca,
    "--cert", alice_cert,
    "--key", alice_key,
    "-q", "1",
    "-t", disconnect_topic,
    "-n",
)
time.sleep(1.0)
queued_payload = make_envelope("bob queued reply while alice is offline")
expect_rc(
    gateway.harmonia_gateway_send(b"mqtt", message_topic.encode("utf-8"), queued_payload.encode("utf-8")),
    "gateway send queued reply",
    gateway_error,
)
time.sleep(1.5)
queue_db = state_root / "mqtt-offline-queue.db"
conn = sqlite3.connect(queue_db)
queued_rows = conn.execute(
    "SELECT COUNT(*) FROM offline_messages WHERE device_id = ?",
    (device_id,),
).fetchone()[0]
if queued_rows < 1:
    raise SystemExit("expected queued offline MQTT message")
conn.close()

queued_reply_path = state_root / "alice-queued-reply.json"
sub_proc = subprocess.Popen(
    [
        mosq_sub,
        "-h", "127.0.0.1",
        "-p", os.environ["BROKER_PORT"],
        "--cafile", ca,
        "--cert", alice_cert,
        "--key", alice_key,
        "-q", "1",
        "-C", "1",
        "-t", message_topic,
    ],
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
    text=True,
)
time.sleep(1.0)
mosquitto(
    mosq_pub,
    "-h", "127.0.0.1",
    "-p", os.environ["BROKER_PORT"],
    "--cafile", ca,
    "--cert", alice_cert,
    "--key", alice_key,
    "-q", "1",
    "-t", connect_topic,
    "-m", connect_payload,
)
stdout, stderr = sub_proc.communicate(timeout=15)
if sub_proc.returncode != 0:
    raise SystemExit(f"mosquitto_sub failed for queued reply flush: {stderr}")
if "bob queued reply while alice is offline" not in stdout:
    raise SystemExit(f"alice did not receive queued reply flush: {stdout}")
queued_reply_path.write_text(stdout, encoding="utf-8")
conn = sqlite3.connect(queue_db)
remaining_rows = conn.execute(
    "SELECT COUNT(*) FROM offline_messages WHERE device_id = ?",
    (device_id,),
).fetchone()[0]
conn.close()
if remaining_rows != 0:
    raise SystemExit(f"offline queue was not flushed, remaining rows={remaining_rows}")

gateway.harmonia_gateway_shutdown()
print("MQTT_REMOTE_CONFIG_LIVE_OK=1")
PY

echo "[7/8] verify broker log and queue artifacts"
if [[ ! -s "$STATE_ROOT/gateway-poll.sexp" ]]; then
  echo "missing gateway poll artifact" >&2
  exit 1
fi
if [[ ! -s "$STATE_ROOT/alice-online-reply.json" ]]; then
  echo "missing immediate reply artifact" >&2
  exit 1
fi
if [[ ! -s "$STATE_ROOT/alice-queued-reply.json" ]]; then
  echo "missing queued reply artifact" >&2
  exit 1
fi

echo "[8/8] done"
echo "WORKDIR=$TMPDIR_TEST"
echo "REMOTE_AGENT_URL=$REMOTE_AGENT_URL"
echo "REMOTE_PUSH_DEVICES_URL=$REMOTE_PUSH_DEVICES_URL"
echo "REMOTE_PUSH_WEBHOOK_URL=$REMOTE_PUSH_WEBHOOK_URL"
echo "BROKER_LOG=$BROKER_LOG"
echo "STATE_ROOT=$STATE_ROOT"
echo "ALICE_MQTT_FP=$ALICE_MQTT_FP"
echo "BOB_MQTT_FP=$BOB_MQTT_FP"
echo "TOPIC_BASE=$TOPIC_BASE"
