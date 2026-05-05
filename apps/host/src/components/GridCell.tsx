import React from "react";
import { type GridCell as Cell, type Device } from "../store";

interface HostCellData { row: number; col: number; name: string; hostname: string; }

interface Props {
  cell: Cell;
  device?: Device;
  hostCell: HostCellData | null;
  onDrop: (deviceId: string, row: number, col: number) => void;
  onClear: (row: number, col: number) => void;
}

export function GridCell({ cell, device, hostCell, onDrop, onClear }: Props) {
  const [dragOver, setDragOver] = React.useState(false);

  const handleDragOver = (e: React.DragEvent) => {
    if (hostCell) return;
    e.preventDefault();
    setDragOver(true);
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    if (hostCell) return;
    const deviceId = e.dataTransfer.getData("deviceId");
    if (deviceId) onDrop(deviceId, cell.row, cell.col);
  };

  return (
    <div
      className={[
        "grid-cell",
        device ? "filled" : "empty",
        hostCell ? "host" : "",
        dragOver ? "drag-over" : "",
      ]
        .filter(Boolean)
        .join(" ")}
      onDragOver={handleDragOver}
      onDragLeave={() => setDragOver(false)}
      onDrop={handleDrop}
      style={{
        background: hostCell
          ? "var(--accent-dim)"
          : device
          ? "var(--cell-filled)"
          : "var(--cell-empty)",
        border: `2px solid ${
          hostCell ? "var(--accent)" : dragOver ? "var(--accent)" : "var(--border)"
        }`,
        borderRadius: "var(--radius)",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        gap: 4,
        padding: 8,
        minHeight: 80,
        position: "relative",
        transition: "border-color 0.15s, background 0.15s",
        cursor: hostCell ? "default" : device ? "default" : "crosshair",
      }}
    >
      {hostCell && (
        <>
          <span style={{ fontSize: 10, color: "var(--accent)", fontWeight: 700, letterSpacing: 1, textTransform: "uppercase" }}>
            HOST
          </span>
          <span style={{ fontSize: 13, fontWeight: 600, color: "var(--text)", textAlign: "center" }}>
            {hostCell.hostname}
          </span>
          <span style={{ fontSize: 11, color: "var(--text-dim)", textAlign: "center" }}>
            {hostCell.name}
          </span>
        </>
      )}
      {!hostCell && device && (
        <>
          <span style={{ fontSize: 12, fontWeight: 600, textAlign: "center" }}>
            {device.displayName || device.deviceId.slice(0, 8)}
          </span>
          <span
            className={`badge ${
              device.status === "established" ? "online" : "pairing"
            }`}
          >
            {device.status}
          </span>
          <button
            className="ghost"
            style={{ fontSize: 10, padding: "2px 6px", position: "absolute", top: 4, right: 4 }}
            onClick={() => onClear(cell.row, cell.col)}
            title="Remove from grid"
          >
            ✕
          </button>
        </>
      )}
      {!hostCell && !device && (
        <span style={{ fontSize: 11, color: "var(--text-dim)" }}>
          {cell.row},{cell.col}
        </span>
      )}
    </div>
  );
}
