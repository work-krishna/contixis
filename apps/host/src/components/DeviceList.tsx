import React from "react";
import { useStore, type Device } from "../store";
import { disconnectDevice } from "../lib/tauri";

export function DeviceList() {
  const { devices, pairingPin, pairingDeviceId, setPairingPin, removeDevice } = useStore();
  const deviceArr = Object.values(devices);
  const connectedCount = deviceArr.filter(d => d.status === "established").length;

  return (
    <aside
      style={{
        width: 240,
        borderLeft: "1px solid var(--border)",
        display: "flex",
        flexDirection: "column",
        background: "var(--surface)",
      }}
    >
      <div
        style={{
          padding: "12px 16px",
          borderBottom: "1px solid var(--border)",
          fontWeight: 600,
          fontSize: 13,
        }}
      >
        Devices
        <span style={{ fontWeight: 400, color: "var(--text-dim)", fontSize: 12, marginLeft: 6 }}>
          {connectedCount} connected
        </span>
      </div>

      {/* Pairing PIN banner */}
      {pairingPin && (
        <div
          style={{
            margin: 12,
            padding: 12,
            background: "#2a2a1a",
            border: "1px solid var(--warning)",
            borderRadius: "var(--radius)",
          }}
        >
          <div style={{ fontSize: 11, color: "var(--warning)", marginBottom: 4 }}>
            Pairing PIN
          </div>
          <div
            style={{
              fontFamily: "monospace",
              fontSize: 28,
              fontWeight: 700,
              letterSpacing: 6,
            }}
          >
            {pairingPin}
          </div>
          <div style={{ fontSize: 11, color: "var(--text-dim)", marginTop: 4 }}>
            Enter on {pairingDeviceId?.slice(0, 8) ?? "device"}
          </div>
          <button
            className="ghost"
            style={{ marginTop: 8, width: "100%", fontSize: 11 }}
            onClick={() => setPairingPin(null, null)}
          >
            Dismiss
          </button>
        </div>
      )}

      <div style={{ flex: 1, overflowY: "auto", padding: "8px 0" }}>
        {deviceArr.length === 0 ? (
          <p
            style={{
              padding: "24px 16px",
              color: "var(--text-dim)",
              fontSize: 12,
              textAlign: "center",
            }}
          >
            No devices connected.
            <br />
            Install Contixis Agent on other computers.
          </p>
        ) : (
          deviceArr.map((d) => (
            <DeviceRow
              key={d.deviceId}
              device={d}
              onDisconnect={() =>
                disconnectDevice(d.deviceId).catch(console.error)
              }
              onRemove={() => removeDevice(d.deviceId)}
            />
          ))
        )}
      </div>
    </aside>
  );
}

function DeviceRow({
  device,
  onDisconnect,
  onRemove,
}: {
  device: Device;
  onDisconnect: () => void;
  onRemove: () => void;
}) {
  const [hover, setHover] = React.useState(false);
  const screenCount = useStore(
    (s) => s.spatialScreens.filter((sc) => sc.deviceId === device.deviceId).length
  );

  const isDisconnected = device.status === "disconnected";
  const isEstablished  = device.status === "established";

  return (
    <div
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        padding: "10px 16px",
        borderBottom: "1px solid var(--border)",
        background: hover ? "color-mix(in srgb, var(--surface) 80%, var(--border))" : "transparent",
        transition: "background 0.1s",
        opacity: isDisconnected ? 0.6 : 1,
      }}
    >
      {/* Name + status badge */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 8 }}>
        <span style={{ fontWeight: 500, fontSize: 13, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {device.displayName || device.deviceId.slice(0, 12)}
        </span>
        <span
          className={`badge ${
            isEstablished           ? "online"
            : device.status === "pairing" ? "pairing"
            : "offline"
          }`}
          style={{ flexShrink: 0 }}
        >
          {device.status}
        </span>
      </div>

      {/* Sub-info */}
      <div style={{ fontSize: 11, color: "var(--text-dim)", marginTop: 2 }}>
        {device.osType ? `${device.osType} · ` : ""}
        {screenCount} screen{screenCount !== 1 ? "s" : ""}
      </div>

      {/* Action row */}
      {isDisconnected ? (
        <div style={{ marginTop: 6, display: "flex", alignItems: "center", gap: 8 }}>
          <button
            className="ghost"
            style={{ fontSize: 11, padding: "3px 8px" }}
            onClick={onRemove}
          >
            Remove
          </button>
          <span style={{ fontSize: 10, color: "var(--text-dim)", fontStyle: "italic" }}>
            Reconnects automatically
          </span>
        </div>
      ) : hover && isEstablished ? (
        <button
          className="danger"
          style={{ marginTop: 6, fontSize: 11, padding: "3px 8px" }}
          onClick={onDisconnect}
        >
          Disconnect
        </button>
      ) : null}
    </div>
  );
}
