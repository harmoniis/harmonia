#!/bin/sh
# Harmonia installer — https://harmoniis.com/harmonia/install
# Usage: curl -sSf https://harmoniis.com/harmonia/install | sh
set -eu

BOLD='\033[1m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
CYAN='\033[0;36m'
DIM='\033[2m'
RESET='\033[0m'

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
INSTALL_PROFILE="${HARMONIA_INSTALL_PROFILE:-full-agent}"

info()  { printf "${CYAN}→${RESET} %s\n" "$1"; }
ok()    { printf "${GREEN}✓${RESET} %s\n" "$1"; }
warn()  { printf "${YELLOW}!${RESET} %s\n" "$1"; }
err()   { printf "${RED}✗${RESET} %s\n" "$1"; }

validate_install_profile() {
    case "$INSTALL_PROFILE" in
        full-agent|tui-client) ;;
        *)
            err "Invalid install profile: $INSTALL_PROFILE"
            printf "  use: full-agent or tui-client\n"
            exit 1
            ;;
    esac
}

parse_args() {
    while [ "$#" -gt 0 ]; do
        case "$1" in
            --profile)
                [ "$#" -ge 2 ] || { err "--profile requires a value"; exit 1; }
                INSTALL_PROFILE="$2"
                shift 2
                ;;
            --profile=*)
                INSTALL_PROFILE="${1#*=}"
                shift
                ;;
            --version)
                [ "$#" -ge 2 ] || { err "--version requires a value"; exit 1; }
                HARMONIA_VERSION="$2"
                export HARMONIA_VERSION
                shift 2
                ;;
            --version=*)
                HARMONIA_VERSION="${1#*=}"
                export HARMONIA_VERSION
                shift
                ;;
            *)
                err "Unknown option: $1"
                exit 1
                ;;
        esac
    done
    export HARMONIA_INSTALL_PROFILE="$INSTALL_PROFILE"
    validate_install_profile
}

main() {
    parse_args "$@"
    printf "\n"
    printf "${BOLD}${CYAN}"
    printf "  _   _                                  _       \n"
    printf " | | | | __ _ _ __ _ __ ___   ___  _ __ (_) __ _ \n"
    printf " | |_| |/ _\` | '__| '_ \` _ \\ / _ \\| '_ \\| |/ _\` |\n"
    printf " |  _  | (_| | |  | | | | | | (_) | | | | | (_| |\n"
    printf " |_| |_|\\__,_|_|  |_| |_| |_|\\___/|_| |_|_|\\__,_|\n"
    printf "${RESET}\n"
    printf "  ${DIM}Distributed evolutionary homoiconic self-improving agent${RESET}\n\n"

    detect_platform
    check_rust
    if [ "$INSTALL_PROFILE" = "full-agent" ]; then
        check_sbcl
        check_quicklisp
    else
        info "Install profile: tui-client (skipping local SBCL/Quicklisp bootstrap)"
    fi
    install_harmonia
    print_next_steps
}

detect_platform() {
    OS="$(uname -s)"
    ARCH="$(uname -m)"

    case "$OS" in
        Linux*)   PLATFORM="linux" ;;
        Darwin*)  PLATFORM="macos" ;;
        FreeBSD*) PLATFORM="freebsd" ;;
        MINGW*|MSYS*|CYGWIN*)
            err "Windows detected. Use install.ps1 instead:"
            printf "  iwr https://harmoniis.com/harmonia/install.ps1 -UseB | iex\n"
            exit 1
            ;;
        *)
            err "Unsupported OS: $OS"
            exit 1
            ;;
    esac

    case "$ARCH" in
        x86_64|amd64) ARCH_TAG="x86_64" ;;
        arm64|aarch64) ARCH_TAG="aarch64" ;;
        *) ARCH_TAG="$ARCH" ;;
    esac
    PLATFORM_TAG="${PLATFORM}-${ARCH_TAG}"

    info "Platform: $PLATFORM ($ARCH)"
}

check_rust() {
    if command -v cargo > /dev/null 2>&1; then
        ok "Rust $(rustc --version | cut -d' ' -f2)"
    else
        info "Installing Rust via rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        . "$HOME/.cargo/env"
        ok "Rust installed: $(rustc --version | cut -d' ' -f2)"
    fi
}

check_sbcl() {
    if command -v sbcl > /dev/null 2>&1; then
        ok "SBCL $(sbcl --version 2>/dev/null | head -1 | sed 's/SBCL //')"
        return
    fi

    info "SBCL not found. Installing..."

    case "$PLATFORM" in
        macos)
            if command -v brew > /dev/null 2>&1; then
                brew install sbcl
            else
                err "Install Homebrew first: https://brew.sh"
                err "Then run: brew install sbcl"
                exit 1
            fi
            ;;
        linux)
            if command -v apt > /dev/null 2>&1; then
                sudo apt update && sudo apt install -y sbcl
            elif command -v dnf > /dev/null 2>&1; then
                sudo dnf install -y sbcl
            elif command -v pacman > /dev/null 2>&1; then
                sudo pacman -S --noconfirm sbcl
            elif command -v apk > /dev/null 2>&1; then
                sudo apk add sbcl
            else
                err "Cannot auto-install SBCL. Install manually:"
                printf "  https://www.sbcl.org/getting.html\n"
                exit 1
            fi
            ;;
        freebsd)
            sudo pkg install -y sbcl
            ;;
    esac

    if command -v sbcl > /dev/null 2>&1; then
        ok "SBCL installed"
    else
        err "SBCL installation failed. Install manually and re-run."
        exit 1
    fi
}

check_quicklisp() {
    if [ -f "$HOME/quicklisp/setup.lisp" ]; then
        ok "Quicklisp"
        return
    fi

    info "Installing Quicklisp..."

    QLTEMP="$(mktemp /tmp/quicklisp-XXXXXX.lisp)"
    curl -sS -o "$QLTEMP" https://beta.quicklisp.org/quicklisp.lisp

    sbcl --non-interactive \
         --load "$QLTEMP" \
         --eval '(quicklisp-quickstart:install)' \
         --eval '(ql:add-to-init-file)' \
         > /dev/null 2>&1

    rm -f "$QLTEMP"

    if [ -f "$HOME/quicklisp/setup.lisp" ]; then
        ok "Quicklisp installed"
    else
        err "Quicklisp installation failed."
        exit 1
    fi
}

share_root() {
    case "$PLATFORM" in
        linux)
            if [ -n "${XDG_DATA_HOME:-}" ]; then
                printf '%s\n' "${XDG_DATA_HOME}/harmonia"
            else
                printf '%s\n' "$HOME/.local/share/harmonia"
            fi
            ;;
        macos|freebsd)
            printf '%s\n' "$HOME/.local/share/harmonia"
            ;;
    esac
}

run_local_repo_installer() {
    INSTALLER="$1"
    if [ -n "${HARMONIA_VERSION:-}" ]; then
        HARMONIA_VERSION="$HARMONIA_VERSION" HARMONIA_INSTALL_PROFILE="$INSTALL_PROFILE" bash "$INSTALLER"
    else
        HARMONIA_INSTALL_PROFILE="$INSTALL_PROFILE" bash "$INSTALLER"
    fi
}

install_harmonia_binary() {
    INSTALL_URL="${HARMONIA_BINARY_INSTALL_URL:-https://github.com/harmoniis/harmonia/releases/latest/download/install.sh}"
    TMP_INSTALL="$(mktemp /tmp/harmonia-release-install-XXXXXX.sh)"

    info "Installing Harmonia binary release (${PLATFORM_TAG})..."
    if ! curl -fsSL "$INSTALL_URL" -o "$TMP_INSTALL"; then
        rm -f "$TMP_INSTALL"
        return 1
    fi
    chmod +x "$TMP_INSTALL"

    if ! run_local_repo_installer "$TMP_INSTALL"; then
        rm -f "$TMP_INSTALL"
        return 1
    fi
    rm -f "$TMP_INSTALL"

    ok "Harmonia installed"
    return 0
}

install_harmonia_source() {
    HARMONIA_REPO="${HARMONIA_REPO:-https://github.com/harmoniis/harmonia.git}"
    HARMONIA_SOURCE_ROOT="${HARMONIA_SOURCE_ROOT:-$(share_root)/source-checkout}"

    if [ -d "$HARMONIA_SOURCE_ROOT/.git" ]; then
        info "Updating source checkout at $HARMONIA_SOURCE_ROOT..."
        (
            cd "$HARMONIA_SOURCE_ROOT"
            git pull --ff-only || warn "git pull failed — continuing with existing source"
        )
    else
        info "Cloning Harmonia source from $HARMONIA_REPO..."
        mkdir -p "$(dirname "$HARMONIA_SOURCE_ROOT")"
        git clone "$HARMONIA_REPO" "$HARMONIA_SOURCE_ROOT"
    fi

    info "Installing from source checkout..."
    (
        cd "$HARMONIA_SOURCE_ROOT"
        sh scripts/install.sh
    )
}

install_optional_source_checkout() {
    WITH_SOURCE="${HARMONIA_WITH_SOURCE:-0}"
    [ "$WITH_SOURCE" = "1" ] || return 0

    HARMONIA_REPO="${HARMONIA_REPO:-https://github.com/harmoniis/harmonia.git}"
    SOURCE_ROOT="${HARMONIA_SOURCE_REWRITE_ROOT:-$(share_root)/source-rewrite}"
    info "Installing optional source checkout for source-rewrite mode..."
    if [ -d "$SOURCE_ROOT/.git" ]; then
        (
            cd "$SOURCE_ROOT"
            git pull --ff-only || warn "source checkout update failed (continuing)"
        )
    else
        mkdir -p "$(dirname "$SOURCE_ROOT")"
        git clone "$HARMONIA_REPO" "$SOURCE_ROOT"
    fi
    ok "Source checkout ready: $SOURCE_ROOT"
}

install_harmonia() {
    if [ -f "$SCRIPT_DIR/Cargo.toml" ] && [ -d "$SCRIPT_DIR/src" ] && [ -f "$SCRIPT_DIR/scripts/install.sh" ]; then
        info "Installing from local repository checkout..."
        run_local_repo_installer "$SCRIPT_DIR/scripts/install.sh"
        return 0
    fi

    INSTALL_MODE="${HARMONIA_INSTALL_MODE:-binary}"
    case "$INSTALL_MODE" in
        binary)
            if install_harmonia_binary; then
                install_optional_source_checkout
                return 0
            fi
            warn "Binary install failed; falling back to source build."
            install_harmonia_source
            ;;
        source)
            install_harmonia_source
            ;;
        *)
            err "Invalid HARMONIA_INSTALL_MODE=$INSTALL_MODE (use binary or source)"
            exit 1
            ;;
    esac
}

print_next_steps() {
    printf "\n"
    printf "${BOLD}  What's next${RESET}\n\n"
    if [ "$INSTALL_PROFILE" = "tui-client" ]; then
        printf "  On the remote agent node, print a pairing code:\n\n"
        printf "    ${CYAN}${BOLD}harmonia pairing invite${RESET}\n\n"
        printf "  Then open the local Harmonia session client and paste that code:\n\n"
        printf "    ${CYAN}${BOLD}harmonia${RESET}\n\n"
        printf "  Node/session state will be stored under:\n\n"
        printf "    ${CYAN}${BOLD}%s/nodes/${RESET}<node-label>\n\n" "${HARMONIA_DATA_DIR:-$HOME/.harmoniis/harmonia}"
    else
        printf "  Configure your agent (API keys, frontends, evolution mode):\n\n"
        printf "    ${CYAN}${BOLD}harmonia setup${RESET}\n\n"
        printf "  Then start the agent:\n\n"
        printf "    ${CYAN}${BOLD}harmonia start${RESET}\n\n"
    fi
}

main "$@"
