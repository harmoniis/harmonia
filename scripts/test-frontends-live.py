#!/usr/bin/env python3
"""
Live frontend integration harness for Harmonia gateway.

What this does:
1) Loads libharmonia_gateway and attempts to register each frontend.
   - Frontends are always attempted.
   - Env vars are treated as optional bootstrap config only.
2) Optionally sends a test message per frontend when --send is enabled and target env vars exist.
3) Runs a poll tick to ensure gateway/frontend interaction is alive.
4) Best-effort QR endpoint probing for WhatsApp/Signal bridge APIs.

This script never prints secret values.
"""

from __future__ import annotations

import argparse
import ctypes
import json
import os
import sqlite3
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Dict, List, Optional, Sequence, Tuple


ROOT = Path(__file__).resolve().parents[1]
LIB_DIR = ROOT / "target" / "release"
GATEWAY_LIB = LIB_DIR / "libharmonia_gateway.dylib"
DEFAULT_STATE_ROOT = Path.home() / ".harmoniis" / "harmonia"


def _quote(value: str) -> str:
    return value.replace("\\", "\\\\").replace('"', '\\"')


def _config_from_env(frontend: str) -> Tuple[Optional[str], List[str]]:
    """Return (config_sexp, warnings). Never blocks registration on env."""
    if frontend == "tui":
        return '(:name "tui")', []
    if frontend == "mqtt":
        return '(:name "mqtt")', []
    if frontend == "telegram":
        tok = os.getenv("HARMONIA_TELEGRAM_BOT_TOKEN", "")
        if tok:
            return f'(:bot-token "{_quote(tok)}")', []
        return '(:name "telegram")', []
    if frontend == "slack":
        bot = os.getenv("HARMONIA_SLACK_BOT_TOKEN", "")
        app = os.getenv("HARMONIA_SLACK_APP_TOKEN", "")
        channels = os.getenv("HARMONIA_SLACK_CHANNELS", "")
        parts: List[str] = []
        if bot:
            parts.append(f'(bot-token "{_quote(bot)}")')
        if app:
            parts.append(f'(app-token "{_quote(app)}")')
        channel_items = " ".join(
            f'"{_quote(ch.strip())}"' for ch in channels.split(",") if ch.strip()
        )
        if channel_items:
            parts.append(f"(channels {channel_items})")
        if not parts:
            return '(:name "slack")', []
        return f"(slack-config {' '.join(parts)})", []
    if frontend == "discord":
        tok = os.getenv("HARMONIA_DISCORD_BOT_TOKEN", "")
        channels = os.getenv("HARMONIA_DISCORD_CHANNELS", "")
        parts: List[str] = []
        if tok:
            parts.append(f'(bot-token "{_quote(tok)}")')
        channel_items = " ".join(
            f'"{_quote(ch.strip())}"' for ch in channels.split(",") if ch.strip()
        )
        if channel_items:
            parts.append(f"(channels {channel_items})")
        if not parts:
            return '(:name "discord")', []
        return f"(discord-config {' '.join(parts)})", []
    if frontend == "signal":
        account = os.getenv("HARMONIA_SIGNAL_ACCOUNT", "")
        rpc = os.getenv("HARMONIA_SIGNAL_RPC_URL", "")
        auth = os.getenv("HARMONIA_SIGNAL_AUTH_TOKEN", "")
        parts = []
        if account:
            parts.append(f'(:account "{_quote(account)}")')
        if rpc:
            parts.append(f'(:rpc-url "{_quote(rpc)}")')
        if auth:
            parts.append(f'(:auth-token "{_quote(auth)}")')
        if not parts:
            return '(:name "signal")', []
        return f"(signal-config {' '.join(parts)})", []
    if frontend == "whatsapp":
        # URL optional because client defaults localhost bridge.
        url = os.getenv("HARMONIA_WHATSAPP_API_URL", "")
        key = os.getenv("HARMONIA_WHATSAPP_API_KEY", "")
        parts = []
        if url:
            parts.append(f'(:api-url "{_quote(url)}")')
        if key:
            parts.append(f'(:api-key "{_quote(key)}")')
        return f"(whatsapp-config {' '.join(parts)})", []
    if frontend == "imessage":
        url = os.getenv("HARMONIA_IMESSAGE_SERVER_URL", "")
        pw = os.getenv("HARMONIA_IMESSAGE_PASSWORD", "")
        parts = []
        if url:
            parts.append(f':server-url "{_quote(url)}"')
        if pw:
            parts.append(f':password "{_quote(pw)}"')
        if not parts:
            return '(:name "imessage")', []
        return f"({' '.join(parts)})", []
    if frontend == "tailscale":
        # Uses tailnet config + listener bind; skip unless user explicitly opts in.
        return '(:name "tailscale")', []
    if frontend == "mattermost":
        return '(:name "mattermost")', []
    if frontend == "nostr":
        return '(:name "nostr")', []
    if frontend == "email":
        return '(:name "email")', []
    return None, [f"unknown frontend: {frontend}"]


def _read_config_store(scope: str, key: str) -> Optional[str]:
    state_root = os.getenv("HARMONIA_STATE_ROOT", "").strip()
    if not state_root:
        state_root = str(Path.home() / ".harmoniis" / "harmonia")
    db_path = Path(os.getenv("HARMONIA_CONFIG_DB", "").strip() or (Path(state_root) / "config.db"))
    if not db_path.exists():
        return None
    try:
        conn = sqlite3.connect(str(db_path))
        try:
            row = conn.execute(
                "SELECT value FROM config_kv WHERE scope=? AND key=? LIMIT 1",
                (scope, key),
            ).fetchone()
            if not row or not row[0]:
                return None
            value = str(row[0]).strip()
            return value or None
        finally:
            conn.close()
    except Exception:
        return None


def _bootstrap_runtime_env() -> None:
    state_root = os.getenv("HARMONIA_STATE_ROOT", "").strip()
    if not state_root and DEFAULT_STATE_ROOT.exists():
        os.environ["HARMONIA_STATE_ROOT"] = str(DEFAULT_STATE_ROOT)
        state_root = str(DEFAULT_STATE_ROOT)

    if state_root:
        root = Path(state_root)
        config_db = root / "config.db"
        vault_db = root / "vault.db"
        if not os.getenv("HARMONIA_CONFIG_DB", "").strip() and config_db.exists():
            os.environ["HARMONIA_CONFIG_DB"] = str(config_db)
        if not os.getenv("HARMONIA_VAULT_DB", "").strip() and vault_db.exists():
            os.environ["HARMONIA_VAULT_DB"] = str(vault_db)


def _send_target_env(frontend: str) -> Optional[str]:
    mapping = {
        "telegram": "HARMONIA_TEST_TELEGRAM_CHAT_ID",
        "slack": "HARMONIA_TEST_SLACK_CHANNEL_ID",
        "discord": "HARMONIA_TEST_DISCORD_CHANNEL_ID",
        "signal": "HARMONIA_TEST_SIGNAL_TARGET",
        "whatsapp": "HARMONIA_TEST_WHATSAPP_TARGET",
        "imessage": "HARMONIA_TEST_IMESSAGE_TARGET",
        "tailscale": "HARMONIA_TEST_TAILSCALE_TARGET",
        "mqtt": "HARMONIA_TEST_MQTT_TOPIC",
        "mattermost": "HARMONIA_TEST_MATTERMOST_CHANNEL_ID",
        "nostr": "HARMONIA_TEST_NOSTR_CHANNEL",
        "email": "HARMONIA_TEST_EMAIL_RECIPIENT",
    }
    env_name = mapping.get(frontend)
    if not env_name:
        return None
    v = os.getenv(env_name, "").strip()
    return v or None


def _probe_http_json(url: str, headers: Optional[Dict[str, str]] = None) -> Tuple[int, str]:
    req = urllib.request.Request(url, headers=headers or {})
    with urllib.request.urlopen(req, timeout=6) as resp:
        body = resp.read().decode("utf-8", errors="replace")
        return int(resp.status), body


def _probe_qr_endpoints() -> None:
    print("\n== QR Probe (best effort) ==")

    wa_url = (
        os.getenv("HARMONIA_WHATSAPP_API_URL", "").strip()
        or _read_config_store("whatsapp-frontend", "api-url")
        or ""
    ).rstrip("/")
    wa_key = os.getenv("HARMONIA_WHATSAPP_API_KEY", "").strip()
    if wa_url:
        headers = {}
        if wa_key:
            headers["Authorization"] = f"Bearer {wa_key}"
        paths = ["/api/qr", "/qr", "/api/session/qr"]
        found = False
        for p in paths:
            url = f"{wa_url}{p}"
            try:
                status, body = _probe_http_json(url, headers=headers)
                sample = body[:180].replace("\n", " ")
                print(f"whatsapp qr {url} -> HTTP {status} body_sample={sample}")
                found = True
                out = Path("/tmp/harmonia_whatsapp_qr_probe.json")
                out.write_text(body, encoding="utf-8")
                print(f"whatsapp qr response saved: {out}")
                break
            except Exception as e:
                print(f"whatsapp qr {url} -> {e}")
        if not found:
            print("whatsapp qr: no known endpoint responded")
    else:
        print("whatsapp qr: skipped (HARMONIA_WHATSAPP_API_URL unset)")

    sig_url = (
        os.getenv("HARMONIA_SIGNAL_RPC_URL", "").strip()
        or _read_config_store("signal-frontend", "rpc-url")
        or ""
    ).rstrip("/")
    sig_token = os.getenv("HARMONIA_SIGNAL_AUTH_TOKEN", "").strip()
    if sig_url:
        headers = {}
        if sig_token:
            headers["Authorization"] = f"Bearer {sig_token}"
        paths = ["/v1/qrcodelink?deviceName=harmonia", "/v1/qrcode", "/v2/qrcode"]
        found = False
        for p in paths:
            url = f"{sig_url}{p}"
            try:
                status, body = _probe_http_json(url, headers=headers)
                sample = body[:180].replace("\n", " ")
                print(f"signal qr {url} -> HTTP {status} body_sample={sample}")
                found = True
                out = Path("/tmp/harmonia_signal_qr_probe.json")
                out.write_text(body, encoding="utf-8")
                print(f"signal qr response saved: {out}")
                break
            except Exception as e:
                print(f"signal qr {url} -> {e}")
        if not found:
            print("signal qr: no known endpoint responded")
    else:
        print("signal qr: skipped (HARMONIA_SIGNAL_RPC_URL unset)")


def main() -> int:
    _bootstrap_runtime_env()
    parser = argparse.ArgumentParser(description="Live Harmonia frontend audit harness")
    parser.add_argument(
        "--send",
        action="store_true",
        help="send a test message on successfully registered frontends when HARMONIA_TEST_* target env vars are set",
    )
    parser.add_argument(
        "--include-tailscale",
        action="store_true",
        help="attempt tailscale frontend registration (may fail in sandbox due bind permissions)",
    )
    args = parser.parse_args()

    if not GATEWAY_LIB.exists():
        print(f"gateway library missing: {GATEWAY_LIB}", file=sys.stderr)
        return 2

    gw = ctypes.CDLL(str(GATEWAY_LIB))
    gw.harmonia_gateway_init.restype = ctypes.c_int
    gw.harmonia_gateway_register.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p]
    gw.harmonia_gateway_register.restype = ctypes.c_int
    gw.harmonia_gateway_send.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p]
    gw.harmonia_gateway_send.restype = ctypes.c_int
    gw.harmonia_gateway_poll.restype = ctypes.c_void_p
    gw.harmonia_gateway_last_error.restype = ctypes.c_void_p
    gw.harmonia_gateway_shutdown.restype = ctypes.c_int
    gw.harmonia_gateway_free_string.argtypes = [ctypes.c_void_p]

    def last_error() -> str:
        ptr = gw.harmonia_gateway_last_error()
        if not ptr:
            return ""
        text = ctypes.string_at(ptr).decode("utf-8", errors="replace")
        gw.harmonia_gateway_free_string(ptr)
        return text

    def poll_once() -> str:
        ptr = gw.harmonia_gateway_poll()
        if not ptr:
            return ""
        text = ctypes.string_at(ptr).decode("utf-8", errors="replace")
        gw.harmonia_gateway_free_string(ptr)
        return text

    frontends: List[str] = [
        "tui",
        "mqtt",
        "telegram",
        "slack",
        "discord",
        "signal",
        "whatsapp",
        "imessage",
        "mattermost",
        "nostr",
        "email",
    ]
    if args.include_tailscale:
        frontends.append("tailscale")

    print(f"gateway init rc={gw.harmonia_gateway_init()}")
    print("\n== Register ==")

    status: Dict[str, str] = {}
    for name in frontends:
        so_map = {
            "tui": "libharmonia_tui.dylib",
            "mqtt": "libharmonia_mqtt_client.dylib",
            "telegram": "libharmonia_telegram.dylib",
            "slack": "libharmonia_slack.dylib",
            "discord": "libharmonia_discord.dylib",
            "signal": "libharmonia_signal.dylib",
            "whatsapp": "libharmonia_whatsapp.dylib",
            "imessage": "libharmonia_imessage.dylib",
            "mattermost": "libharmonia_mattermost.dylib",
            "nostr": "libharmonia_nostr.dylib",
            "email": "libharmonia_email_client.dylib",
            "tailscale": "libharmonia_tailscale_frontend.dylib",
        }
        lib_name = so_map[name]
        lib_path = LIB_DIR / lib_name
        if not lib_path.exists():
            status[name] = "missing-lib"
            print(f"{name:10} skip missing library {lib_path}")
            continue

        config, warnings = _config_from_env(name)
        if warnings:
            status[name] = "config-warning"
            print(f"{name:10} config warning: {', '.join(warnings)}")
            continue

        rc = gw.harmonia_gateway_register(
            name.encode("utf-8"),
            str(lib_path).encode("utf-8"),
            config.encode("utf-8"),
            b"authenticated",
        )
        err = last_error()
        if rc == 0:
            status[name] = "registered"
            print(f"{name:10} ok")
        else:
            status[name] = "register-failed"
            print(f"{name:10} fail rc={rc} err={err}")

    print("\n== Poll ==")
    try:
        result = poll_once()
        print(f"poll ok bytes={len(result.encode('utf-8'))}")
    except Exception as e:
        print(f"poll failed: {e}")

    if args.send:
        print("\n== Send ==")
        for name in frontends:
            if status.get(name) != "registered":
                continue
            target = _send_target_env(name)
            if not target:
                print(f"{name:10} skip missing HARMONIA_TEST_* target")
                continue
            payload = f"[harmonia live test] {name} {int(time.time())}"
            rc = gw.harmonia_gateway_send(
                name.encode("utf-8"),
                target.encode("utf-8"),
                payload.encode("utf-8"),
            )
            err = last_error()
            if rc == 0:
                print(f"{name:10} send ok target={target}")
            else:
                print(f"{name:10} send fail rc={rc} err={err}")

    _probe_qr_endpoints()

    print("\n== Summary ==")
    for name in frontends:
        print(f"{name:10} {status.get(name, 'not-run')}")

    print(f"\ngateway shutdown rc={gw.harmonia_gateway_shutdown()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
