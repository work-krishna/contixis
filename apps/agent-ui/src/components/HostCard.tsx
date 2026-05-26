import { useEffect, useRef, useState } from "react";
import { connectToHost, disconnect, enterPin, forgetHost } from "../lib/tauri";
import { useStore } from "../store";

interface HostCardProps {
  hostId:      string;
  label:       string;
  sublabel:    string;
  addr?:       string;
  isOnline?:   boolean;
  isPaired?:   boolean;
  canForget?:  boolean;
  onForgot?:   () => void;
}

export function HostCard({
  hostId, label, sublabel, addr, isOnline, isPaired, canForget, onForgot,
}: HostCardProps) {
  const { connStatus, connHostId } = useStore();
  const [hover, setHover]         = useState(false);
  const [awaitingPin, setAwaitingPin] = useState(false);
  const [pin, setPin]             = useState("");
  const [pinBusy, setPinBusy]     = useState(false);
  const [pinError, setPinError]   = useState("");
  const pinRef = useRef<HTMLInputElement>(null);

  const isConnected  = connStatus === "connected" && connHostId === hostId;
  const busy         = connStatus !== "idle";

  // Reset PIN panel when connection ends or succeeds
  useEffect(() => {
    if (connStatus === "idle" || connStatus === "connected") {
      setAwaitingPin(false);
      setPin("");
      setPinError("");
    }
  }, [connStatus]);

  // Auto-focus PIN input when it appears
  useEffect(() => {
    if (awaitingPin) setTimeout(() => pinRef.current?.focus(), 50);
  }, [awaitingPin]);

  const handleConnect = async () => {
    if (!addr) return;
    setPinError("");
    try {
      await connectToHost(addr);
      setAwaitingPin(true); // show PIN input immediately
    } catch (e: any) {
      console.error(e);
    }
  };

  const handlePinSubmit = async () => {
    const trimmed = pin.trim();
    if (!trimmed || pinBusy) return;
    setPinBusy(true);
    setPinError("");
    try {
      await enterPin(trimmed);
      setAwaitingPin(false);
      setPin("");
    } catch (e: any) {
      setPinError(typeof e === "string" ? e : "Failed — check PIN and retry.");
    } finally {
      setPinBusy(false);
    }
  };

  const handleDisconnect = async () => {
    try { await disconnect(); }
    catch (e) { console.error(e); }
  };

  const handleForget = async () => {
    try {
      await forgetHost(hostId);
      onForgot?.();
    } catch (e) { console.error(e); }
  };

  const statusBadge = isConnected
    ? <span className="badge online">connected</span>
    : awaitingPin
    ? <span className="badge connecting">pairing…</span>
    : isOnline
    ? <span className="badge online">online</span>
    : isPaired
    ? <span className="badge offline">offline</span>
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
      {/* Row: name + badge */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 8 }}>
        <span style={{ fontWeight: 500, fontSize: 13, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {label}
        </span>
        {statusBadge}
      </div>

      <div style={{ fontSize: 11, color: "var(--text-dim)", marginTop: 2 }}>
        {sublabel}
      </div>

      {/* PIN input — shown immediately after clicking Connect */}
      {awaitingPin && (
        <div style={{ marginTop: 12 }}>
          <div style={{ fontSize: 11, color: "var(--text-dim)", marginBottom: 6 }}>
            Enter the PIN shown on the host:
          </div>
          <input
            ref={pinRef}
            type="text"
            value={pin}
            onChange={(e) => { setPin(e.target.value); setPinError(""); }}
            onKeyDown={(e) => e.key === "Enter" && handlePinSubmit()}
            placeholder="123456"
            maxLength={8}
            style={{
              width: "100%",
              boxSizing: "border-box",
              background: "var(--bg)",
              border: `1px solid ${pinError ? "var(--danger, #e55)" : "var(--border)"}`,
              borderRadius: "var(--radius)",
              color: "var(--text)",
              padding: "8px 10px",
              fontSize: 22,
              letterSpacing: 8,
              textAlign: "center",
              marginBottom: 8,
            }}
          />
          {pinError && (
            <div style={{ fontSize: 11, color: "var(--danger, #e55)", marginBottom: 6 }}>
              {pinError}
            </div>
          )}
          <div style={{ display: "flex", gap: 6 }}>
            <button
              className="primary"
              style={{ flex: 1 }}
              disabled={pinBusy || !pin.trim()}
              onClick={handlePinSubmit}
            >
              {pinBusy ? "Confirming…" : "Confirm PIN"}
            </button>
            <button
              className="ghost"
              onClick={() => { setAwaitingPin(false); setPin(""); disconnect().catch(() => {}); }}
            >
              Cancel
            </button>
          </div>
        </div>
      )}

      {/* Action buttons — only on hover when not showing PIN input */}
      {!awaitingPin && hover && (
        <div style={{ display: "flex", gap: 6, marginTop: 8 }}>
          {isConnected ? (
            <button className="danger" onClick={handleDisconnect}>Disconnect</button>
          ) : addr ? (
            <button className="primary" onClick={handleConnect} disabled={busy}>
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
