import { CSSProperties } from "react";
import { useStore } from "../store";

type DragStyle = CSSProperties & { WebkitAppRegion?: string };

export function Titlebar() {
  const { listenAddr, devices } = useStore();
  const connected = Object.values(devices).filter(
    (d) => d.status === "established"
  ).length;

  return (
    <header
      style={{
        height: 40,
        borderBottom: "1px solid var(--border)",
        display: "flex",
        alignItems: "center",
        padding: "0 16px",
        gap: 16,
        background: "var(--surface)",
        flexShrink: 0,
        WebkitAppRegion: "drag",
      } as DragStyle}
    >
      <span style={{ fontWeight: 700, fontSize: 14, letterSpacing: 0.5 }}>
        Contixis
      </span>
      <span style={{ color: "var(--text-dim)", fontSize: 12 }}>
        {listenAddr}
      </span>
      <div style={{ flex: 1 }} />
      <span
        className={`badge ${connected > 0 ? "online" : "offline"}`}
        style={{ WebkitAppRegion: "no-drag" } as DragStyle}
      >
        {connected} connected
      </span>
    </header>
  );
}
