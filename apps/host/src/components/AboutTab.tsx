const INFO_ROWS = [
  { label: "Version",   value: "0.1.0" },
  { label: "License",   value: "MIT" },
  { label: "Protocol",  value: "QUIC / TLS 1.3" },
  { label: "Transport", value: "contixis-net (QUIC)" },
  { label: "Platform",  value: "Linux · Windows · macOS" },
];

export function AboutTab() {
  return (
    <div style={{ flex: 1, overflowY: "auto", padding: "20px 24px" }}>
      <div style={{ maxWidth: 540 }}>
        {/* Logo / name */}
        <div style={{ display: "flex", alignItems: "center", gap: 14, marginBottom: 24 }}>
          <div style={{
            width: 52, height: 52, borderRadius: 12,
            background: "var(--accent-dim)",
            border: "2px solid var(--accent)",
            display: "flex", alignItems: "center", justifyContent: "center",
            fontSize: 26, fontWeight: 900, color: "var(--accent)",
          }}>C</div>
          <div>
            <div style={{ fontSize: 22, fontWeight: 800, color: "var(--text)" }}>Contixis</div>
            <div style={{ fontSize: 13, color: "var(--text-dim)", marginTop: 2 }}>
              KVM-over-network — share one keyboard &amp; mouse across machines
            </div>
          </div>
        </div>

        {/* Info table */}
        <div style={{ background: "var(--surface)", borderRadius: 8, border: "1px solid var(--border)", overflow: "hidden", marginBottom: 24 }}>
          {INFO_ROWS.map((r, i) => (
            <div
              key={r.label}
              style={{
                display: "flex",
                justifyContent: "space-between",
                padding: "10px 16px",
                borderBottom: i < INFO_ROWS.length - 1 ? "1px solid var(--border)" : "none",
              }}
            >
              <span style={{ fontSize: 13, color: "var(--text-dim)" }}>{r.label}</span>
              <span style={{ fontSize: 13, color: "var(--text)", fontWeight: 500 }}>{r.value}</span>
            </div>
          ))}
        </div>

        {/* Description */}
        <p style={{ fontSize: 13, color: "var(--text-dim)", lineHeight: 1.7, marginBottom: 16 }}>
          Contixis lets you control multiple computers with a single keyboard and mouse.
          Move your cursor to the edge of any screen to seamlessly switch to the adjacent machine.
          Clipboard, keyboard input, and mouse movement are all forwarded securely over an encrypted QUIC connection.
        </p>

        <p style={{ fontSize: 12, color: "var(--text-dim)", lineHeight: 1.6 }}>
          Built with Rust · Tauri · React
        </p>
      </div>
    </div>
  );
}
