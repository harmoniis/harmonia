#!/usr/bin/env bash
# Build release tarball for a given version.
# Usage: bash scripts/build_release.sh <VERSION>
set -euo pipefail

VERSION="${1:?Usage: build_release.sh <VERSION>}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# Detect platform
case "$(uname -s)-$(uname -m)" in
    Linux-x86_64)   PLATFORM="linux-x86_64"   ; LIB_EXT="so"    ;;
    Linux-aarch64)   PLATFORM="linux-aarch64"  ; LIB_EXT="so"    ;;
    Darwin-arm64)    PLATFORM="macos-aarch64"  ; LIB_EXT="dylib" ;;
    Darwin-x86_64)   PLATFORM="macos-x86_64"  ; LIB_EXT="dylib" ;;
    *)               echo "Unsupported platform: $(uname -s)-$(uname -m)"; exit 1 ;;
esac

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
if [ -f "target/release/harmonia" ]; then
    cp target/release/harmonia "$STAGING/bin/"
fi

# Phoenix supervisor binary (name can vary by build profile)
if [ -f "target/release/harmonia-phoenix" ]; then
    cp target/release/harmonia-phoenix "$STAGING/bin/"
elif [ -f "target/release/phoenix" ]; then
    cp target/release/phoenix "$STAGING/bin/harmonia-phoenix"
fi

# Collect all shared libraries
echo "Collecting shared libraries..."
for lib in target/release/*."${LIB_EXT}"; do
    [ -f "$lib" ] || continue
    name="$(basename "$lib")"
    # Skip build artifacts that aren't our libs
    case "$name" in
        libharmonia_*) cp "$lib" "$STAGING/lib/" ;;
    esac
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
