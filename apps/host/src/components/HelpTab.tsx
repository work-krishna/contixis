const SECTIONS = [
  {
    title: "Getting Started",
    items: [
      "Launch Contixis on the host machine — it starts the server automatically.",
      "Install and run the Contixis agent on each remote machine.",
      "The agent appears in the device list once it connects to the host.",
      "Accept the pairing PIN shown on screen when connecting for the first time.",
    ],
  },
  {
    title: "Screen Layout",
    items: [
      "The View tab shows a spatial canvas of all connected screens.",
      "Drag agent screens to reposition them relative to the host.",
      "Screens always snap to edges — no gaps are allowed between screens.",
      "When you drag a screen out from between others, adjacent screens auto-shift to fill the gap.",
    ],
  },
  {
    title: "Mouse & Keyboard",
    items: [
      "Move your cursor to the edge of any screen to seamlessly switch to the adjacent screen.",
      "All keystrokes and mouse input are forwarded to whichever screen has focus.",
      "Press the release shortcut (default: Ctrl+Alt+H) to return focus to the host. Configurable in Settings.",
      "Clipboard content is shared automatically between host and all agents.",
      "Scroll direction follows each machine's system settings.",
    ],
  },
  {
    title: "Troubleshooting",
    items: [
      "If an agent shows as disconnected, ensure port 7443 is reachable from the agent machine.",
      "Re-pair a device by disconnecting and reconnecting — accept the new pairing PIN.",
      "On Linux agents, the uinput kernel module must be loaded: sudo modprobe uinput",
      "Agent input injection requires the user to be in the 'input' group or run as root.",
    ],
  },
];

export function HelpTab() {
  return (
    <div style={{ flex: 1, overflowY: "auto", padding: "20px 24px" }}>
      <h2 style={{ fontSize: 18, fontWeight: 700, color: "var(--text)", marginBottom: 20 }}>Help &amp; Usage</h2>
      <div style={{ display: "flex", flexDirection: "column", gap: 24, maxWidth: 640 }}>
        {SECTIONS.map(sec => (
          <div key={sec.title}>
            <h3 style={{ fontSize: 13, fontWeight: 700, color: "var(--accent)", textTransform: "uppercase", letterSpacing: 0.5, marginBottom: 10 }}>
              {sec.title}
            </h3>
            <ul style={{ display: "flex", flexDirection: "column", gap: 8, paddingLeft: 0, listStyle: "none" }}>
              {sec.items.map((item, i) => (
                <li key={i} style={{ display: "flex", gap: 10, fontSize: 13, color: "var(--text)", lineHeight: 1.6 }}>
                  <span style={{ color: "var(--accent)", flexShrink: 0, marginTop: 2 }}>›</span>
                  <span>{item}</span>
                </li>
              ))}
            </ul>
          </div>
        ))}
      </div>
    </div>
  );
}
