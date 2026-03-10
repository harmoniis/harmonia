#!/usr/bin/env bash
# Harmonia installer.
#
# Modes:
#   1. Local repo install: run from a checked-out repo (`scripts/install.sh`)
#   2. Local artifact install: run from an extracted release tarball (`./install.sh`)
#   3. Remote artifact install: fetch latest or requested GitHub release
#
# Layout:
#   - User data:    ~/.harmoniis/harmonia/          (databases, config, state, frontends)
#   - Binaries:     platform-specific user bin dir
#   - Libraries:    platform-specific user lib dir
#   - Shared data:  platform-specific user share dir
set -euo pipefail

REPO="harmoniis/harmonia"
VERSION="${HARMONIA_VERSION:-}"

info()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
warn()  { printf '\033[1;33m==>\033[0m %s\n' "$*"; }
error() { printf '\033[1;31m==>\033[0m %s\n' "$*" >&2; exit 1; }

detect_platform() {
    local os arch
    os="$(uname -s)"
    arch="$(uname -m)"
    case "$os" in
        Linux)   os="linux" ;;
        Darwin)  os="macos" ;;
        FreeBSD) os="freebsd" ;;
        *)       error "Unsupported OS: $os" ;;
    esac
    case "$arch" in
        x86_64|amd64) arch="x86_64" ;;
        aarch64|arm64) arch="aarch64" ;;
        *) error "Unsupported architecture: $arch" ;;
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
    else
        error "Neither curl nor wget found"
    fi
}

data_dir() {
    echo "${HARMONIA_DATA_DIR:-$HOME/.harmoniis/harmonia}"
}

bin_dir() {
    case "$(uname -s)" in
        Darwin|Linux|FreeBSD)
            echo "${HARMONIA_BIN_DIR:-$HOME/.local/bin}"
            ;;
        *)
            error "Unsupported OS for shell installer: $(uname -s)"
            ;;
    esac
}

lib_dir() {
    case "$(uname -s)" in
        Darwin|Linux|FreeBSD)
            echo "${HARMONIA_LIB_DIR:-$HOME/.local/lib/harmonia}"
            ;;
        *)
            error "Unsupported OS for shell installer: $(uname -s)"
            ;;
    esac
}

share_dir() {
    case "$(uname -s)" in
        Darwin|FreeBSD)
            echo "${HARMONIA_SHARE_DIR:-$HOME/.local/share/harmonia}"
            ;;
        Linux)
            if [ -n "${HARMONIA_SHARE_DIR:-}" ]; then
                echo "${HARMONIA_SHARE_DIR}"
            elif [ -n "${XDG_DATA_HOME:-}" ]; then
                echo "${XDG_DATA_HOME}/harmonia"
            else
                echo "$HOME/.local/share/harmonia"
            fi
            ;;
        *)
            error "Unsupported OS for shell installer: $(uname -s)"
            ;;
    esac
}

shell_rc() {
    case "${SHELL:-}" in
        */zsh)  echo "$HOME/.zshrc" ;;
        */bash) echo "$HOME/.bashrc" ;;
        *)      echo "" ;;
    esac
}

repo_version() {
    sed -n 's/^version = "\(.*\)"/\1/p' "$1/Cargo.toml" | head -1
}

shared_lib_ext() {
    case "$(uname -s)" in
        Darwin) echo "dylib" ;;
        *)      echo "so" ;;
    esac
}

copy_tree_contents() {
    local src="$1" dest="$2"
    [ -d "$src" ] || return 0
    mkdir -p "$dest"
    cp -R "$src/." "$dest/"
}

install_shell_integration() {
    local rc="$1" bindir="$2"
    [ -n "$rc" ] || return 0
    mkdir -p "$(dirname "$rc")"
    if ! grep -qF "$bindir" "$rc" 2>/dev/null; then
        info "Adding ${bindir} to ${rc}..."
        {
            echo ""
            echo "# Harmonia agent"
            echo "export PATH=\"${bindir}:\$PATH\""
        } >> "$rc"
    fi
}

install_bins() {
    local src="$1" bindir="$2"
    mkdir -p "$bindir"
    rm -f "$bindir/harmonia" "$bindir/harmonia-phoenix"
    for bin in harmonia harmonia-phoenix phoenix; do
        if [ -f "$src/$bin" ]; then
            local dest="$bindir/$bin"
            [ "$bin" = "phoenix" ] && dest="$bindir/harmonia-phoenix"
            cp "$src/$bin" "$dest"
            chmod +x "$dest" 2>/dev/null || true
        fi
    done
    [ -x "$bindir/harmonia" ] || error "harmonia binary missing after install"
}

install_libs() {
    local src="$1" libdir="$2" ext="$3"
    mkdir -p "$libdir"
    find "$libdir" -maxdepth 1 -type f -name "libharmonia_*.${ext}" -delete 2>/dev/null || true
    local copied=0
    while IFS= read -r lib; do
        cp "$lib" "$libdir/"
        copied=1
    done < <(find "$src" -maxdepth 1 -type f -name "libharmonia_*.${ext}" | sort)
    [ "$copied" -eq 1 ] || error "no runtime libraries found in ${src}"
}

install_share_tree() {
    local src_root="$1" sharedir="$2"
    mkdir -p "$sharedir"
    rm -rf "$sharedir/src" "$sharedir/config" "$sharedir/doc"
    copy_tree_contents "$src_root/src" "$sharedir/src"
    copy_tree_contents "$src_root/config" "$sharedir/config"
    copy_tree_contents "$src_root/doc" "$sharedir/doc"
}

ensure_data_dirs() {
    local datadir="$1"
    mkdir -p "$datadir" "$datadir/state" "$datadir/frontends" "$datadir/config"
}

install_from_artifact_root() {
    local artifact_root="$1" version="$2"
    local bindir libdir sharedir datadir rc ext
    bindir="$(bin_dir)"
    libdir="$(lib_dir)"
    sharedir="$(share_dir)"
    datadir="$(data_dir)"
    rc="$(shell_rc)"
    ext="$(shared_lib_ext)"

    info "Installing Harmonia v${version}"
    info "  user data: ${datadir}"
    info "  binaries:  ${bindir}"
    info "  libraries: ${libdir}"
    info "  shared:    ${sharedir}"

    install_bins "$artifact_root/bin" "$bindir"
    install_libs "$artifact_root/lib" "$libdir" "$ext"
    install_share_tree "$artifact_root" "$sharedir"
    ensure_data_dirs "$datadir"
    install_shell_integration "$rc" "$bindir"

    info "Installation complete"
    echo ""
    echo "  Version:   $("$bindir/harmonia" --version 2>/dev/null || echo unknown)"
    echo "  Binary:    ${bindir}/harmonia"
    echo "  Libraries: ${libdir}"
    echo "  Shared:    ${sharedir}"
    echo "  User data: ${datadir}"
    echo ""
    if [ -n "$rc" ]; then
        case ":$PATH:" in
            *":$bindir:"*) ;;
            *)
                info "Restart your shell or run:"
                echo ""
                echo "    source ${rc}"
                echo ""
                ;;
        esac
    fi
}

install_from_local_repo() {
    local repo_root="$1"
    local version ext stage
    version="$(repo_version "$repo_root")"
    [ -n "$version" ] || error "could not determine version from ${repo_root}/Cargo.toml"
    ext="$(shared_lib_ext)"

    info "Building Harmonia v${version} from local repo..."
    (cd "$repo_root" && cargo build --workspace --release)

    stage="$(mktemp -d)"
    trap "rm -rf -- \"$stage\"" EXIT
    mkdir -p "$stage/bin" "$stage/lib" "$stage/src" "$stage/config" "$stage/doc"

    install_bins "$repo_root/target/release" "$stage/bin"
    install_libs "$repo_root/target/release" "$stage/lib" "$ext"
    copy_tree_contents "$repo_root/src" "$stage/src"
    copy_tree_contents "$repo_root/config" "$stage/config"
    copy_tree_contents "$repo_root/doc" "$stage/doc"

    install_from_artifact_root "$stage" "$version"
}

download_release_artifact() {
    local platform version tmpdir tarball url checksum_url extracted
    platform="$(detect_platform)"
    version="$1"
    tmpdir="$(mktemp -d)"
    trap "rm -rf -- \"$tmpdir\"" EXIT
    tarball="harmonia-${version}-${platform}.tar.gz"
    url="https://github.com/${REPO}/releases/download/harmonia-v${version}/${tarball}"

    info "Downloading ${url}..."
    fetch "$url" "$tmpdir/$tarball"
    checksum_url="${url}.sha256"
    if fetch "$checksum_url" "$tmpdir/${tarball}.sha256" 2>/dev/null; then
        info "Verifying checksum..."
        (
            cd "$tmpdir"
            if command -v sha256sum >/dev/null 2>&1; then
                sha256sum -c "${tarball}.sha256"
            elif command -v shasum >/dev/null 2>&1; then
                shasum -a 256 -c "${tarball}.sha256"
            else
                warn "No sha256sum or shasum found, skipping checksum verification"
            fi
        )
    else
        warn "Checksum file not available, skipping verification"
    fi

    info "Extracting..."
    tar xzf "$tmpdir/$tarball" -C "$tmpdir"
    extracted="$(find "$tmpdir" -maxdepth 1 -type d -name 'harmonia-*' | head -1)"
    [ -d "$extracted" ] || error "extraction failed"
    printf '%s\n' "$extracted"
}

main() {
    local script_dir repo_root extracted
    script_dir="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)"
    repo_root="$(cd "$script_dir/.." && pwd)"

    if [ -f "$repo_root/Cargo.toml" ] && [ -d "$repo_root/src" ] && [ -d "$repo_root/scripts" ]; then
        install_from_local_repo "$repo_root"
        return 0
    fi

    if [ -d "$script_dir/bin" ] && [ -d "$script_dir/lib" ]; then
        VERSION="${VERSION:-$(basename "$script_dir" | sed 's/^harmonia-//')}"
        install_from_artifact_root "$script_dir" "${VERSION:-unknown}"
        return 0
    fi

    if [ -z "$VERSION" ]; then
        info "Detecting latest release..."
        VERSION="$(latest_version)"
        [ -n "$VERSION" ] || error "Could not determine latest version"
    fi
    extracted="$(download_release_artifact "$VERSION")"
    install_from_artifact_root "$extracted" "$VERSION"
}

main "$@"
