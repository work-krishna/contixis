import { useState } from "react";
import { enterPin } from "../lib/tauri";

export function PinDialog({ onDismiss }: { onDismiss: () => void }) {
  const [pin, setPin] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (!pin.trim()) return;
    setBusy(true);
    try {
      await enterPin(pin.trim());
      onDismiss();
    } catch (e) {
      console.error(e);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      style={{
        position: "fixed", inset: 0,
        background: "rgba(0,0,0,0.6)",
        display: "flex", alignItems: "center", justifyContent: "center",
        zIndex: 100,
      }}
    >
      <div
        style={{
          background: "var(--surface)",
          border: "1px solid var(--border)",
          borderRadius: "var(--radius)",
          padding: 24,
          width: 300,
        }}
      >
        <div style={{ fontWeight: 600, marginBottom: 4 }}>Pairing Required</div>
        <div style={{ fontSize: 12, color: "var(--text-dim)", marginBottom: 16 }}>
          Enter the PIN shown on the host machine.
        </div>
        <input
          type="text"
          value={pin}
          onChange={(e) => setPin(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && submit()}
          placeholder="e.g. 123456"
          autoFocus
          style={{
            width: "100%",
            background: "var(--bg)",
            border: "1px solid var(--border)",
            borderRadius: "var(--radius)",
            color: "var(--text)",
            padding: "8px 12px",
            fontSize: 20,
            letterSpacing: 6,
            textAlign: "center",
            marginBottom: 16,
          }}
        />
        <div style={{ display: "flex", gap: 8 }}>
          <button
            className="primary"
            style={{ flex: 1 }}
            onClick={submit}
            disabled={busy || !pin.trim()}
          >
            {busy ? "Sending…" : "Confirm"}
          </button>
          <button className="ghost" onClick={onDismiss}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}
