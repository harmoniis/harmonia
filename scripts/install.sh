#!/usr/bin/env bash
# Harmonia installer — works from extracted tarball or fetches latest release.
#
# From tarball:  ./install.sh
# From web:      curl -fsSL https://github.com/harmoniis/harmonia/releases/latest/download/install.sh | bash
#
# Environment overrides:
#   HARMONIA_PREFIX  — installation prefix (default: ~/.harmoniis/harmonia)
#   HARMONIA_VERSION — specific version to install (default: latest)
set -euo pipefail

REPO="harmoniis/harmonia"
PREFIX="${HARMONIA_PREFIX:-$HOME/.harmoniis/harmonia}"
VERSION="${HARMONIA_VERSION:-}"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

info()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn()  { printf '\033[1;33m==>\033[0m %s\n' "$*"; }
error() { printf '\033[1;31m==>\033[0m %s\n' "$*" >&2; exit 1; }

detect_platform() {
    local os arch
    os="$(uname -s)"
    arch="$(uname -m)"
    case "$os" in
        Linux)   os="linux"   ;;
        Darwin)  os="macos"   ;;
        FreeBSD) os="freebsd" ;;
        NetBSD)  os="netbsd"  ;;
        *)       error "Unsupported OS: $os" ;;
    esac
    case "$arch" in
        x86_64|amd64)          arch="x86_64"   ;;
        aarch64|arm64)         arch="aarch64"  ;;
        riscv64)               arch="riscv64"  ;;
        powerpc|ppc)           arch="powerpc"  ;;
        sparc64)               arch="sparc64"  ;;
        *)                     error "Unsupported architecture: $arch" ;;
    esac
    echo "${os}-${arch}"
}

latest_version() {
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
            | grep '"tag_name"' | head -1 | sed 's/.*"harmonia-v\(.*\)".*/\1/'
    elif command -v wget >/dev/null 2>&1; then
        wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" \
            | grep '"tag_name"' | head -1 | sed 's/.*"harmonia-v\(.*\)".*/\1/'
    else
        error "Neither curl nor wget found"
    fi
}

fetch() {
    local url="$1" dest="$2"
    if command -v curl >/dev/null 2>&1; then
        curl -fSL --progress-bar -o "$dest" "$url"
    elif command -v wget >/dev/null 2>&1; then
        wget -q --show-progress -O "$dest" "$url"
    fi
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

# Check if we're running from inside an extracted tarball
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ -d "$SCRIPT_DIR/bin" ] && [ -d "$SCRIPT_DIR/lib" ]; then
    # Local install from extracted tarball
    info "Installing from local tarball..."
    TARBALL_DIR="$SCRIPT_DIR"
else
    # Remote install: fetch tarball
    PLATFORM="$(detect_platform)"
    if [ -z "$VERSION" ]; then
        info "Detecting latest release..."
        VERSION="$(latest_version)"
        [ -n "$VERSION" ] || error "Could not determine latest version"
    fi
    info "Installing Harmonia v${VERSION} for ${PLATFORM}"

    TARBALL="harmonia-${VERSION}-${PLATFORM}.tar.gz"
    URL="https://github.com/${REPO}/releases/download/harmonia-v${VERSION}/${TARBALL}"

    TMPDIR="$(mktemp -d)"
    trap 'rm -rf "$TMPDIR"' EXIT

    info "Downloading ${URL}..."
    fetch "$URL" "$TMPDIR/$TARBALL"

    CHECKSUM_URL="${URL}.sha256"
    if fetch "$CHECKSUM_URL" "$TMPDIR/${TARBALL}.sha256" 2>/dev/null; then
        info "Verifying checksum..."
        cd "$TMPDIR"
        if command -v sha256sum >/dev/null 2>&1; then
            sha256sum -c "${TARBALL}.sha256" || error "Checksum verification failed"
        elif command -v shasum >/dev/null 2>&1; then
            shasum -a 256 -c "${TARBALL}.sha256" || error "Checksum verification failed"
        else
            warn "No sha256sum or shasum found, skipping checksum verification"
        fi
        cd - >/dev/null
    else
        warn "Checksum file not available, skipping verification"
    fi

    info "Extracting..."
    tar xzf "$TMPDIR/$TARBALL" -C "$TMPDIR"

    # Find the extracted directory
    TARBALL_DIR="$(find "$TMPDIR" -maxdepth 1 -type d -name 'harmonia-*' | head -1)"
    [ -d "$TARBALL_DIR" ] || error "Extraction failed"
fi

# Install to prefix
info "Installing to ${PREFIX}..."
mkdir -p "$PREFIX"

# Copy all components
for dir in bin lib config src doc; do
    if [ -d "$TARBALL_DIR/$dir" ]; then
        mkdir -p "$PREFIX/$dir"
        cp -r "$TARBALL_DIR/$dir/"* "$PREFIX/$dir/"
    fi
done

# Make binaries executable
chmod +x "$PREFIX/bin/"* 2>/dev/null || true

# Create state directory
mkdir -p "$PREFIX/state"

# ---------------------------------------------------------------------------
# Shell integration
# ---------------------------------------------------------------------------

SHELL_RC=""
case "${SHELL:-}" in
    */zsh)  SHELL_RC="$HOME/.zshrc"  ;;
    */bash) SHELL_RC="$HOME/.bashrc" ;;
esac

PATH_LINE="export PATH=\"${PREFIX}/bin:\$PATH\""
HARMONIA_LINE="export HARMONIA_HOME=\"${PREFIX}\""

if [ -n "$SHELL_RC" ]; then
    if ! grep -qF "HARMONIA_HOME" "$SHELL_RC" 2>/dev/null; then
        info "Adding Harmonia to ${SHELL_RC}..."
        {
            echo ""
            echo "# Harmonia agent"
            echo "$HARMONIA_LINE"
            echo "$PATH_LINE"
        } >> "$SHELL_RC"
    fi
fi

# ---------------------------------------------------------------------------
# Verify
# ---------------------------------------------------------------------------

info "Installation complete!"
echo ""
echo "  HARMONIA_HOME = ${PREFIX}"
echo "  Binaries:       ${PREFIX}/bin/"
echo "  Libraries:      ${PREFIX}/lib/"
echo "  Config:         ${PREFIX}/config/"
echo "  Lisp source:    ${PREFIX}/src/"
echo "  Runtime state:  ${PREFIX}/state/"
echo ""

if [ -x "$PREFIX/bin/harmonia" ]; then
    echo "  Version: $("$PREFIX/bin/harmonia" --version 2>/dev/null || echo 'unknown')"
    echo ""
fi

if [ -n "$SHELL_RC" ]; then
    warn "Restart your shell or run: source ${SHELL_RC}"
fi
