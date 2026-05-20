# Contixis Development Setup

## Prerequisites

### Rust
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
source "$HOME/.cargo/env"
```

### Linux — Tauri host app system dependencies
```bash
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev \
  build-essential \
  libssl-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev
```

### Node.js (for host frontend)
Node 18+ required. Install via [nvm](https://github.com/nvm-sh/nvm) or system package.

> **Note:** On Ubuntu/Debian, `npm` may not be found even if Node.js is installed via `snap` or a minimal package. If you see `Command 'npm' not found`, install it explicitly:
> ```bash
> sudo apt install npm
> ```

## Build

```bash
# Build all Rust library crates + agent binary
cargo build --workspace --exclude contixis-host

# Build host Tauri app (needs system deps above)
cd apps/host
npm install
npm run tauri dev     # dev mode
npm run tauri build   # production bundle
```

> **VS Code snap terminal — libpthread error:** If you launch the terminal from VS Code installed as a snap, its GTK environment variables (`GTK_PATH`, `GTK_IM_MODULE_FILE`, etc.) point into `/snap/core20`, causing a glibc `libpthread` version mismatch at runtime:
> ```
> symbol lookup error: /snap/core20/current/lib/x86_64-linux-gnu/libpthread.so.0:
>     undefined symbol: __libc_pthread_init, version GLIBC_PRIVATE
> ```
> Fix: unset the snap GTK overrides before running, or open a regular (non-snap) terminal:
> ```bash
> unset GTK_PATH GTK_EXE_PREFIX GTK_IM_MODULE_FILE GDK_PIXBUF_MODULE_FILE \
>       GDK_PIXBUF_MODULEDIR GSETTINGS_SCHEMA_DIR GIO_MODULE_DIR LOCPATH
> npm run tauri dev
> ```

## Project Layout

```
contixis/
├── crates/
│   ├── contixis-proto   # Protobuf definitions (prost)
│   ├── contixis-crypto  # PKI: CA, device identity, pairing, TLS verifiers
│   ├── contixis-core    # VirtualGrid, SessionFSM, InputRouter, ClipboardManager
│   ├── contixis-net     # QUIC transport (quinn), mDNS discovery, session handshake
│   ├── contixis-input   # Platform input capture & injection stubs
│   └── contixis-codec   # Video frame / compression stubs
├── apps/
│   ├── agent/           # Agent daemon binary (Rust)
│   └── host/            # Host application (Tauri 2 + React + TypeScript)
│       ├── src/         # React frontend
│       └── src-tauri/   # Tauri Rust backend
└── Cargo.toml           # Workspace root
```

## Architecture

- **Transport**: QUIC (quinn) with TLS 1.3 (rustls), ECDSA P-256 certificates
- **PKI**: Self-hosted CA (rcgen), PIN-based pairing, mutual TLS
- **Protocol**: Protobuf 3 (prost), wire framing `[1B type][3B BE length][N bytes payload]`
- **Grid**: NxM virtual display grid, normalized [0,1] coordinates
- **Input**: USB HID usage codes, platform stubs for Linux/Windows/macOS
- **Discovery**: mDNS (mdns-sd), manual IP fallback
