#!/usr/bin/env bash
# Build release tarball for a given version.
# Usage: bash scripts/build_release.sh <VERSION> [PLATFORM]
# If PLATFORM is omitted, auto-detect from the current host.
set -euo pipefail

VERSION="${1:?Usage: build_release.sh <VERSION> [PLATFORM]}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# Detect or accept platform
if [ -n "${2:-}" ]; then
    PLATFORM="$2"
    case "$PLATFORM" in
        *windows*) LIB_EXT="dll" ; BIN_EXT=".exe" ;;
        *macos*)   LIB_EXT="dylib" ; BIN_EXT="" ;;
        *)         LIB_EXT="so" ; BIN_EXT="" ;;
    esac
else
    case "$(uname -s)-$(uname -m)" in
        Linux-x86_64)    PLATFORM="linux-x86_64"   ; LIB_EXT="so"    ; BIN_EXT="" ;;
        Linux-aarch64)   PLATFORM="linux-aarch64"   ; LIB_EXT="so"    ; BIN_EXT="" ;;
        Darwin-arm64)    PLATFORM="macos-aarch64"   ; LIB_EXT="dylib" ; BIN_EXT="" ;;
        Darwin-x86_64)   PLATFORM="macos-x86_64"   ; LIB_EXT="dylib" ; BIN_EXT="" ;;
        MINGW*|MSYS*|CYGWIN*) PLATFORM="windows-x86_64" ; LIB_EXT="dll" ; BIN_EXT=".exe" ;;
        *)               echo "Unsupported platform: $(uname -s)-$(uname -m)"; exit 1 ;;
    esac
fi

TARBALL="harmonia-${VERSION}-${PLATFORM}.tar.gz"
STAGING="harmonia-${VERSION}"

echo "==> Building release tarball: ${TARBALL}"

# Ensure release build exists
if [ ! -d target/release ]; then
    echo "Error: target/release not found. Run 'cargo build --workspace --release' first."
    exit 1
fi

# Create staging directory
rm -rf "$STAGING"
mkdir -p "$STAGING/bin" "$STAGING/lib" "$STAGING/config" "$STAGING/src"

# CLI binary
if [ -f "target/release/harmonia${BIN_EXT}" ]; then
    cp "target/release/harmonia${BIN_EXT}" "$STAGING/bin/"
fi

# Phoenix supervisor binary (name can vary by build profile)
if [ -f "target/release/harmonia-phoenix${BIN_EXT}" ]; then
    cp "target/release/harmonia-phoenix${BIN_EXT}" "$STAGING/bin/"
elif [ -f "target/release/phoenix${BIN_EXT}" ]; then
    cp "target/release/phoenix${BIN_EXT}" "$STAGING/bin/harmonia-phoenix${BIN_EXT}"
fi

# Collect all shared libraries
echo "Collecting shared libraries..."
for lib in target/release/*."${LIB_EXT}"; do
    [ -f "$lib" ] || continue
    name="$(basename "$lib")"
    # Skip build artifacts that aren't our libs
    case "$name" in
        libharmonia_*|harmonia_*) ;;
        *) continue ;;
    esac
    # iMessage (BlueBubbles) only works on macOS — skip on other platforms
    case "$PLATFORM" in
        macos*) ;;
        *)
            case "$name" in
                *imessage*) echo "  [skip] $name (macOS only)"; continue ;;
            esac
            ;;
    esac
    cp "$lib" "$STAGING/lib/"
done

# Copy config files
cp -r config/ "$STAGING/config/"

# Copy Lisp source (needed for SBCL runtime)
cp -r src/ "$STAGING/src/"

# Copy install script
if [ -f scripts/install.sh ]; then
    cp scripts/install.sh "$STAGING/"
    chmod +x "$STAGING/install.sh"
fi

# Copy doc reference for offline use
if [ -d doc/reference ]; then
    mkdir -p "$STAGING/doc/reference"
    cp -r doc/reference/ "$STAGING/doc/reference/"
fi

# List contents
echo "Release contents:"
find "$STAGING" -type f | sort | while read -r f; do
    echo "  $f"
done

# Create tarball
tar czf "$TARBALL" "$STAGING"
rm -rf "$STAGING"

# Generate SHA256 checksum
CHECKSUM_FILE="${TARBALL}.sha256"
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$TARBALL" > "$CHECKSUM_FILE"
else
    shasum -a 256 "$TARBALL" > "$CHECKSUM_FILE"
fi

echo "==> Created ${TARBALL} ($(du -h "$TARBALL" | cut -f1))"
echo "==> Checksum: $(cat "$CHECKSUM_FILE")"
