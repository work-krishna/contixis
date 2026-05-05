import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// ── Commands ────────────────────────────────────────────────────────────────

export function startServer(addr: string): Promise<void> {
  return invoke("start_server", { addr });
}

export function stopServer(): Promise<void> {
  return invoke("stop_server");
}

export interface HostCell { row: number; col: number; name: string; hostname: string; }

export function getHostInfo(): Promise<{ hostId: string; addr: string; hostCells: HostCell[] }> {
  return invoke("get_host_info");
}

export function updateGridLayout(
  cells: Array<{ row: number; col: number; deviceId?: string; screenId?: string }>
): Promise<void> {
  return invoke("update_grid_layout", { cells });
}

export function disconnectDevice(deviceId: string): Promise<void> {
  return invoke("disconnect_device", { deviceId });
}

// ── Events ───────────────────────────────────────────────────────────────────

export interface GridCellUpdate { row: number; col: number; deviceId: string; screenId: string; }

export type DeviceEvent =
  | { type: "connected";    deviceId: string; displayName: string; osType: string }
  | { type: "established";  deviceId: string }
  | { type: "pairing";      deviceId: string; pin: string }
  | { type: "disconnected"; deviceId: string }
  | { type: "gridUpdate";   cells: GridCellUpdate[] };

export function onDeviceEvent(cb: (e: DeviceEvent) => void) {
  return listen<DeviceEvent>("device-event", (event) => cb(event.payload));
}
