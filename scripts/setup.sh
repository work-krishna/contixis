#!/usr/bin/env bash
set -euo pipefail

# ── Contixis development environment setup ───────────────────────────────────
# Idempotent: safe to run multiple times.

BOLD='\033[1m'; GREEN='\033[0;32m'; YELLOW='\033[0;33m'; RED='\033[0;31m'; NC='\033[0m'

info()    { echo -e "${GREEN}[setup]${NC} $*"; }
warn()    { echo -e "${YELLOW}[setup]${NC} $*"; }
section() { echo -e "\n${BOLD}── $* ──────────────────────────────────────────${NC}"; }

# ── 1. Detect OS ─────────────────────────────────────────────────────────────

section "Detecting OS"
OS="$(uname -s)"
DISTRO=""
if [[ "$OS" == "Linux" ]]; then
    if command -v lsb_release &>/dev/null; then
        DISTRO="$(lsb_release -si)"
    elif [[ -f /etc/os-release ]]; then
        DISTRO="$(. /etc/os-release && echo "$ID")"
    fi
    info "Linux / $DISTRO"
elif [[ "$OS" == "Darwin" ]]; then
    info "macOS"
else
    warn "Windows detected — run this script in WSL2 or Git Bash"
fi

# ── 2. Linux system dependencies (Tauri + build tools) ───────────────────────

if [[ "$OS" == "Linux" ]]; then
    section "Linux system dependencies"

    PKGS=(
        # Build essentials
        build-essential
        pkg-config
        libssl-dev
        curl

        # Tauri WebView (required for the host GUI)
        libwebkit2gtk-4.1-dev
        libsoup-3.0-dev
        libjavascriptcoregtk-4.1-dev

        # Tauri system tray / icons
        libgtk-3-dev
        libayatana-appindicator3-dev
        librsvg2-dev

        # protobuf compiler (used by contixis-proto build.rs fallback)
        protobuf-compiler

        # X11 for input injection stubs
        libx11-dev
        libxtst-dev
    )

    MISSING=()
    for pkg in "${PKGS[@]}"; do
        dpkg -s "$pkg" &>/dev/null || MISSING+=("$pkg")
    done

    if [[ ${#MISSING[@]} -eq 0 ]]; then
        info "All system packages already installed."
    else
        info "Installing missing packages: ${MISSING[*]}"
        sudo apt-get update -qq
        sudo apt-get install -y "${MISSING[@]}"
        info "System packages installed."
    fi
fi

# ── 3. macOS system dependencies ─────────────────────────────────────────────

if [[ "$OS" == "Darwin" ]]; then
    section "macOS dependencies"
    if ! command -v brew &>/dev/null; then
        warn "Homebrew not found — install it from https://brew.sh then re-run this script."
        exit 1
    fi
    brew install pkg-config protobuf
    info "macOS dependencies installed."
fi

# ── 4. Rust toolchain ────────────────────────────────────────────────────────

section "Rust toolchain"
if command -v rustc &>/dev/null; then
    RUST_VER="$(rustc --version)"
    info "Rust already installed: $RUST_VER"
else
    info "Installing Rust via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --default-toolchain stable
    info "Rust installed."
fi

# Ensure cargo is on PATH for the rest of this script.
# shellcheck disable=SC1091
source "${HOME}/.cargo/env" 2>/dev/null || true

# ── 5. Node.js check ─────────────────────────────────────────────────────────

section "Node.js"
if command -v node &>/dev/null; then
    NODE_VER="$(node --version)"
    info "Node.js $NODE_VER already installed."
    MAJOR="${NODE_VER#v}"; MAJOR="${MAJOR%%.*}"
    if [[ $MAJOR -lt 18 ]]; then
        warn "Node.js 18+ required (found $NODE_VER). Please upgrade."
    fi
else
    warn "Node.js not found. Install v18+ from https://nodejs.org or via nvm."
fi

# ── 6. Build the Rust workspace ──────────────────────────────────────────────

section "Rust workspace build"
cd "$(dirname "$0")/.."

# Try building the full workspace first; if WebKit headers are missing on this
# machine, fall back to excluding the host GUI app.
if cargo check --workspace -q 2>/dev/null; then
    info "Running: cargo build --workspace"
    cargo build --workspace
    info "Rust workspace (all crates including host) built successfully."
else
    warn "WebKit/GTK headers not available — building without the host GUI app."
    warn "Install libwebkit2gtk-4.1-dev and re-run to enable the host app build."
    cargo build --workspace --exclude contixis-host
    info "Rust workspace (agent + libs) built successfully."
fi

# ── 7. Host app frontend deps ────────────────────────────────────────────────

section "Host app Node dependencies"
HOST_DIR="apps/host"
if [[ -f "$HOST_DIR/package.json" ]]; then
    if command -v node &>/dev/null; then
        info "Running: npm install in $HOST_DIR"
        npm --prefix "$HOST_DIR" install
        info "Host frontend dependencies installed."
    else
        warn "Skipping npm install — Node.js not available."
    fi
fi

# ── Done ─────────────────────────────────────────────────────────────────────

echo
echo -e "${GREEN}${BOLD}Setup complete!${NC}"
echo
echo "  Run the agent:  cargo run -p contixis-agent"
echo "  Dev host app:   cd apps/host && npm run tauri dev"
echo
