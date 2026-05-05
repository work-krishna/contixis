# Contixis — Host PC Setup

The host PC is the machine whose **keyboard and mouse you want to share** with other computers.
It runs the Contixis desktop application (Tauri + React UI).

---

## Requirements

| | Minimum |
|---|---|
| OS | Ubuntu 22.04 LTS or later (other Debian-based distros work too) |
| RAM | 512 MB free |
| Network | Same LAN as agent PCs (Wi-Fi or Ethernet) |
| Display | X11 session (Wayland not yet supported) |

---

## Step 1 — Install system dependencies

```bash
sudo apt-get update
sudo apt-get install -y \
  build-essential pkg-config libssl-dev \
  libwebkit2gtk-4.1-dev \
  libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libx11-dev \
  libxtst-dev \
  protobuf-compiler
```

---

## Step 2 — Install Rust

Skip if already installed (`rustc --version` succeeds).

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env
```

---

## Step 3 — Install Node.js via nvm

> **Do not use the snap version of Node/npm.** It injects snap library paths that conflict
> with the compiled binary. Use nvm instead.

```bash
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash
source ~/.bashrc
nvm install 20
nvm use 20
```

Verify:
```bash
node --version   # should print v20.x.x
npm --version    # should print 10.x.x
which node       # should print ~/.nvm/versions/...  (NOT /snap/bin/node)
```

---

## Step 4 — Build the host app

```bash
cd /path/to/contixis/apps/host
npm install
npm run tauri build
```

The packaged app will be at:
```
apps/host/src-tauri/target/release/bundle/
  ├── deb/contixis_0.1.0_amd64.deb     ← install this
  └── appimage/contixis_0.1.0_amd64.AppImage
```

Install the `.deb`:
```bash
sudo dpkg -i apps/host/src-tauri/target/release/bundle/deb/contixis_*.deb
```

---

## Step 5 — Run the host app

**After installing the .deb:**
```bash
contixis
```

**Or run directly from the build output (no install):**
```bash
apps/host/src-tauri/target/release/contixis-host
```

**Or run in development mode (hot-reload, from a non-snap terminal):**
```bash
cd apps/host
npm run tauri dev
```

> **Important:** If you use VS Code and it is installed as a **snap**, its integrated terminal
> will cause a library conflict. Always run `npm run tauri dev` from a system terminal
> (GNOME Terminal, Konsole, xterm) opened from the desktop — not from inside VS Code.
> Alternatively, reinstall VS Code from the `.deb` package at https://code.visualstudio.com/download

---

## Step 6 — Start the server

1. Open the Contixis app.
2. Click **Start Server**.
3. The app begins listening on `0.0.0.0:7443` and advertises itself on your LAN via mDNS.

---

## Step 7 — Pair an agent

When an agent connects from another PC:

1. The host UI shows the device name in the **Devices** panel.
2. A **6-digit PIN** appears — read it out to whoever is at the agent machine.
3. Once they enter it, the device status changes to **Established**.
4. **Drag** the device card onto a grid cell (left, right, above, or below the centre square).

---

## Using the grid

| Action | What happens |
|---|---|
| Move cursor to the screen edge toward an agent cell | Focus transfers to that agent |
| Press **Escape** | Focus returns to the host |
| Drag a device to a different grid cell | Rearranges the virtual layout |
| Click **Disconnect** on a device card | Drops the connection |

---

## Troubleshooting

**App shows blank white window**
- You are running in a Wayland session. Log out and choose "Ubuntu on Xorg" at the login screen.

**`symbol lookup error: /snap/core20/...`**
- You are running from inside a snap terminal (VS Code snap). Open a system terminal instead.

**Devices not appearing after agent starts**
- Check that both machines are on the same LAN segment (mDNS does not cross routers).
- Check firewall: `sudo ufw allow 7443/udp` on the host.

**PIN prompt never appears on agent**
- The agent could not reach the host. Try connecting manually:
  ```bash
  CONTIXIS_HOST=192.168.1.x:7443 DISPLAY=:0 contixis-agent run
  ```
  (replace with your host's IP address)
