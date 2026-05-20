import { useState } from "react";
import { connectToHost, disconnect, forgetHost } from "../lib/tauri";
import { useStore } from "../store";

interface HostCardProps {
  hostId:      string;
  label:       string;
  sublabel:    string;
  addr?:       string;
  isOnline?:   boolean;
  isPaired?:   boolean;
  canForget?:  boolean;
}

export function HostCard({
  hostId, label, sublabel, addr, isOnline, isPaired, canForget,
}: HostCardProps) {
  const { connStatus, connHostId, connAddr } = useStore();
  const [hover, setHover] = useState(false);

  const isConnected  = connStatus === "connected"  && connHostId === hostId;
  const isConnecting = (connStatus === "connecting" || connStatus === "pairing") && connAddr === addr;
  const busy         = connStatus !== "idle";

  const handleConnect = async () => {
    if (!addr) return;
    try { await connectToHost(addr); }
    catch (e) { console.error(e); }
  };

  const handleDisconnect = async () => {
    try { await disconnect(); }
    catch (e) { console.error(e); }
  };

  const handleForget = async () => {
    try { await forgetHost(hostId); }
    catch (e) { console.error(e); }
  };

  const statusBadge = isConnected   ? <span className="badge online">connected</span>
    : isConnecting                   ? <span className="badge connecting">connecting…</span>
    : isOnline                       ? <span className="badge online">online</span>
    : isPaired                       ? <span className="badge offline">offline</span>
    : null;

  return (
    <div
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        padding: "12px 16px",
        borderBottom: "1px solid var(--border)",
        background: hover ? "color-mix(in srgb, var(--surface) 80%, var(--border))" : "transparent",
        transition: "background 0.1s",
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 8 }}>
        <span style={{ fontWeight: 500, fontSize: 13, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {label}
        </span>
        {statusBadge}
      </div>

      <div style={{ fontSize: 11, color: "var(--text-dim)", marginTop: 2 }}>
        {sublabel}
      </div>

      {hover && (
        <div style={{ display: "flex", gap: 6, marginTop: 8 }}>
          {isConnected ? (
            <button className="danger" onClick={handleDisconnect}>Disconnect</button>
          ) : addr ? (
            <button className="primary" onClick={handleConnect} disabled={busy && !isConnecting}>
              Connect
            </button>
          ) : null}
          {canForget && !isConnected && (
            <button className="ghost" onClick={handleForget}>Forget</button>
          )}
        </div>
      )}
    </div>
  );
}
