#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

PYTHON_BIN="${PYTHON_BIN:-python3}"

echo "[1/3] build release frontends + gateway"
cargo build --release \
  -p harmonia-gateway \
  -p harmonia-tui \
  -p harmonia-mqtt-client \
  -p harmonia-telegram \
  -p harmonia-slack \
  -p harmonia-discord \
  -p harmonia-signal \
  -p harmonia-whatsapp \
  -p harmonia-imessage \
  -p harmonia-mattermost \
  -p harmonia-nostr \
  -p harmonia-email-client \
  -p harmonia-tailscale-frontend

echo
echo "[2/3] exported symbol audit (gateway contract)"
for lib in \
  libharmonia_tui.dylib \
  libharmonia_mqtt_client.dylib \
  libharmonia_telegram.dylib \
  libharmonia_slack.dylib \
  libharmonia_discord.dylib \
  libharmonia_signal.dylib \
  libharmonia_whatsapp.dylib \
  libharmonia_imessage.dylib \
  libharmonia_tailscale_frontend.dylib \
  libharmonia_mattermost.dylib \
  libharmonia_nostr.dylib \
  libharmonia_email_client.dylib
do
  echo "### ${lib}"
  nm -gU "target/release/${lib}" 2>/dev/null | rg "harmonia_frontend_(version|healthcheck|init|poll|send|last_error|shutdown|free_string)$|harmonia_(mattermost|nostr|email_client)_(version|healthcheck|send|publish_text)$" || true
done

echo
echo "[3/3] gateway register audit (actual runtime load)"
"${PYTHON_BIN}" - <<'PY'
import ctypes
from ctypes import c_char_p, c_int
from pathlib import Path

root = Path("target/release")
gw = ctypes.CDLL(str(root / "libharmonia_gateway.dylib"))

gw.harmonia_gateway_init.restype = c_int
gw.harmonia_gateway_register.argtypes = [c_char_p, c_char_p, c_char_p, c_char_p]
gw.harmonia_gateway_register.restype = c_int
gw.harmonia_gateway_last_error.restype = ctypes.c_void_p
gw.harmonia_gateway_free_string.argtypes = [ctypes.c_void_p]
gw.harmonia_gateway_shutdown.restype = c_int

def last_error() -> str:
    ptr = gw.harmonia_gateway_last_error()
    if not ptr:
        return ""
    s = ctypes.string_at(ptr).decode("utf-8", errors="replace")
    gw.harmonia_gateway_free_string(ptr)
    return s

print(f"init rc={gw.harmonia_gateway_init()}")

frontends = [
    ("tui", "libharmonia_tui.dylib"),
    ("mqtt", "libharmonia_mqtt_client.dylib"),
    ("telegram", "libharmonia_telegram.dylib"),
    ("slack", "libharmonia_slack.dylib"),
    ("discord", "libharmonia_discord.dylib"),
    ("signal", "libharmonia_signal.dylib"),
    ("whatsapp", "libharmonia_whatsapp.dylib"),
    ("imessage", "libharmonia_imessage.dylib"),
    ("tailscale", "libharmonia_tailscale_frontend.dylib"),
    ("mattermost", "libharmonia_mattermost.dylib"),
    ("nostr", "libharmonia_nostr.dylib"),
    ("email", "libharmonia_email_client.dylib"),
]

for name, so in frontends:
    rc = gw.harmonia_gateway_register(
        name.encode("utf-8"),
        str(root / so).encode("utf-8"),
        f'(:name "{name}")'.encode("utf-8"),
        b"authenticated",
    )
    err = last_error()
    print(f"{name:10} rc={rc:>2} err={err}")

print(f"shutdown rc={gw.harmonia_gateway_shutdown()}")
PY

echo
echo "Audit complete."
