import React from "react";
import { useStore, type Device } from "../store";
import { disconnectDevice } from "../lib/tauri";

export function DeviceList() {
  const { devices, pairingPin, pairingDeviceId, setPairingPin } = useStore();
  const deviceArr = Object.values(devices);

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
        Devices ({deviceArr.length})
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
              onDisconnect={() => disconnectDevice(d.deviceId).catch(console.error)}
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
}: {
  device: Device;
  onDisconnect: () => void;
}) {
  const [hover, setHover] = React.useState(false);

  return (
    <div
      draggable
      onDragStart={(e) => e.dataTransfer.setData("deviceId", device.deviceId)}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        padding: "10px 16px",
        borderBottom: "1px solid var(--border)",
        cursor: "grab",
        background: hover ? "#1e2030" : "transparent",
        transition: "background 0.1s",
      }}
    >
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
        }}
      >
        <span style={{ fontWeight: 500, fontSize: 13 }}>
          {device.displayName || device.deviceId.slice(0, 12)}
        </span>
        <span
          className={`badge ${
            device.status === "established"
              ? "online"
              : device.status === "pairing"
              ? "pairing"
              : "offline"
          }`}
        >
          {device.status}
        </span>
      </div>
      <div style={{ fontSize: 11, color: "var(--text-dim)", marginTop: 2 }}>
        {device.osType} · {device.screens.length} screen
        {device.screens.length !== 1 ? "s" : ""}
      </div>
      {hover && (
        <button
          className="danger"
          style={{ marginTop: 6, fontSize: 11, padding: "3px 8px" }}
          onClick={onDisconnect}
        >
          Disconnect
        </button>
      )}
    </div>
  );
}
