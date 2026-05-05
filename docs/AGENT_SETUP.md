# Contixis — Agent (Secondary PC) Setup

The agent PC is a machine you want to **control from the host PC**.
It runs a lightweight background daemon — no GUI required.

---

## Requirements

| | Minimum |
|---|---|
| OS | Ubuntu 22.04 LTS or later |
| RAM | 64 MB free |
| Network | Same LAN as the host PC |
| Display | X11 session running (needed for input injection) |

---

## Step 1 — Install system dependencies

```bash
sudo apt-get update
sudo apt-get install -y \
  build-essential \
  pkg-config \
  libssl-dev \
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

## Step 3 — Add your user to the `input` group

The agent reads raw input events from `/dev/input/*`. Your user must be in the `input` group.

```bash
sudo usermod -aG input $USER
```

**Log out and log back in** (or reboot) for the group change to take effect.

Verify:
```bash
groups | grep input    # should show "input" in the list
```

---

## Step 4 — Build the agent binary

Get the project source on the agent machine (via git clone, scp, or USB), then:

```bash
cd /path/to/contixis
cargo build --release -p contixis-agent
```

The binary will be at `target/release/contixis-agent`.

**Optional — install system-wide:**
```bash
sudo cp target/release/contixis-agent /usr/local/bin/contixis-agent
```

---

## Step 5 — Run the agent

```bash
DISPLAY=:0 contixis-agent run
```

- `DISPLAY=:0` tells the agent which X11 display to inject input events into.
  If your display is not `:0`, check with `echo $DISPLAY`.
- The agent will automatically discover the host via **mDNS** on your LAN.
- On first run it will prompt for a pairing PIN (see Step 6).

---

## Step 6 — Pair with the host

1. Make sure the **host app is running** and the server is started.
2. Start the agent — within a few seconds the host UI shows **"Device connecting…"** with a PIN.
3. The agent terminal prompts:
   ```
   Enter pairing PIN:
   ```
4. Type the PIN shown on the host and press Enter.
5. The host UI shows the device as **Established**.

Pairing is permanent — the agent remembers the host and reconnects automatically on future runs without needing a PIN again.

---

## Step 7 — Automatic startup (optional)

To have the agent start automatically when you log in, create a systemd user service:

```bash
mkdir -p ~/.config/systemd/user

cat > ~/.config/systemd/user/contixis-agent.service << 'EOF'
[Unit]
Description=Contixis Agent
After=graphical-session.target

[Service]
ExecStart=/usr/local/bin/contixis-agent run
Environment=DISPLAY=:0
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
EOF

systemctl --user enable contixis-agent
systemctl --user start contixis-agent
```

Check status:
```bash
systemctl --user status contixis-agent
journalctl --user -u contixis-agent -f    # live logs
```

Stop / restart:
```bash
systemctl --user stop contixis-agent
systemctl --user restart contixis-agent
```

---

## What to expect after pairing

| On host | On agent |
|---|---|
| Device card appears in the Devices panel | Agent logs: `session established` |
| Drag device to a grid cell | Agent logs: `grid layout updated` |
| Move cursor to the edge toward the agent | Agent logs: `focus received` |
| Type / move mouse (on host) | Input is injected into agent's X11 session |
| Press **Escape** | Agent logs: `focus dropped`; control returns to host |

---

## Troubleshooting

**`/dev/input/*: permission denied`**
- You are not in the `input` group, or the group change has not taken effect yet.
- Run `sudo usermod -aG input $USER`, then log out and back in.

**Agent starts but host never shows a pairing prompt**
- Both machines must be on the same LAN (mDNS does not cross routers/subnets).
- Check host firewall: `sudo ufw allow 7443/udp`.
- Try connecting to the host by IP directly:
  ```bash
  DISPLAY=:0 contixis-agent --host 192.168.1.x:7443 run
  ```
  (replace with your host's actual IP)

**Mouse / keyboard not injecting on the agent**
- Make sure an X11 session is running (`echo $DISPLAY` should return `:0` or similar).
- Wayland sessions are not yet supported. Log in with "Ubuntu on Xorg".

**Agent crashes immediately**
- Check logs: `journalctl --user -u contixis-agent -n 50`
- Common cause: `DISPLAY` not set. Always pass `DISPLAY=:0` when running manually.

**After reboot, agent does not auto-connect**
- The systemd service may not be enabled. Run:
  ```bash
  systemctl --user enable contixis-agent
  systemctl --user start contixis-agent
  ```
