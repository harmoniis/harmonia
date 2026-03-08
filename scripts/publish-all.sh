#!/bin/bash
# Publish all Harmonia crates to crates.io in dependency order.
# Usage: ./scripts/publish-all.sh [--dry-run]
set -eu

DRY_RUN=""
if [ "${1:-}" = "--dry-run" ]; then
    DRY_RUN="--dry-run"
    echo "==> Dry-run mode"
fi

SLEEP=75

publish() {
    local crate="$1"
    echo "--- Publishing $crate ---"
    local output
    if output=$(cargo publish -p "$crate" $DRY_RUN 2>&1); then
        echo "$output"
    else
        if echo "$output" | grep -q "already exists"; then
            echo "    $crate already published, skipping"
        else
            echo "$output" >&2
            echo "    ERROR: failed to publish $crate"
            return 1
        fi
    fi
    if [ -z "$DRY_RUN" ]; then
        echo "    Waiting ${SLEEP}s for crates.io index..."
        sleep "$SLEEP"
    fi
}

echo "=== Tier 0: No harmonia-* dependencies ==="
publish harmonia-vault
publish harmonia-phoenix
publish harmonia-ouroboros
publish harmonia-memory
publish harmonia-fs
publish harmonia-http
publish harmonia-s3
publish harmonia-git-ops
publish harmonia-rust-forge
publish harmonia-cron-scheduler
publish harmonia-recovery
publish harmonia-parallel-agents
publish harmonia-harmonic-matrix
publish harmonia-config-store
publish harmonia-gateway

echo "=== Tier 0b: Frontends with no vault deps ==="
publish harmonia-tui
publish harmonia-push
publish harmonia-mqtt-client
publish harmonia-mattermost
publish harmonia-nostr
publish harmonia-email-client
publish harmonia-whisper
publish harmonia-social

echo "=== Tier 1: Depends on vault ==="
publish harmonia-openrouter-backend
publish harmonia-browser
publish harmonia-search-exa
publish harmonia-search-brave
publish harmonia-elevenlabs
publish harmonia-telegram
publish harmonia-slack
publish harmonia-whatsapp
publish harmonia-imessage
publish harmonia-tailnet

echo "=== Tier 2: Depends on tailnet ==="
publish harmonia-tailscale-frontend

echo "=== Tier 3: Root binary ==="
publish harmonia

echo "=== Done ==="
