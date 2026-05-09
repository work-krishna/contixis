import { CSSProperties } from "react";
import { useStore } from "../store";

type DragStyle = CSSProperties & { WebkitAppRegion?: string };

function GearIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
    </svg>
  );
}

export function Titlebar() {
  const { listenAddr, devices, showSettings, setShowSettings } = useStore();
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
      <button
        onClick={() => setShowSettings(!showSettings)}
        title="Settings"
        style={{
          WebkitAppRegion: "no-drag",
          background: showSettings ? "var(--border)" : "transparent",
          border: "1px solid transparent",
          borderRadius: 6,
          padding: "4px 6px",
          color: showSettings ? "var(--accent)" : "var(--text-dim)",
          cursor: "pointer",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          transition: "background 0.15s, color 0.15s",
        } as DragStyle}
      >
        <GearIcon />
      </button>
    </header>
  );
}
