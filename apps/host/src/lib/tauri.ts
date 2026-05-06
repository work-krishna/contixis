import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { SpatialScreen } from "../store";

// ── Commands ────────────────────────────────────────────────────────────────

export function startServer(addr: string): Promise<void> {
  return invoke("start_server", { addr });
}

export function stopServer(): Promise<void> {
  return invoke("stop_server");
}

export interface HostScreenInfo {
  deviceId: string;
  x: number;
  y: number;
  widthPx: number;
  heightPx: number;
  name: string;
  hostname: string;
}

export function getHostInfo(): Promise<{
  hostId: string;
  addr: string;
  hostScreens: HostScreenInfo[];
}> {
  return invoke("get_host_info");
}

export function getSpatialLayout(): Promise<SpatialScreen[]> {
  return invoke<SpatialScreen[]>("get_spatial_layout");
}

export function updateSpatialLayout(
  screens: Array<{ deviceId: string; x: number; y: number }>
): Promise<void> {
  return invoke("update_spatial_layout", { screens });
}

export function disconnectDevice(deviceId: string): Promise<void> {
  return invoke("disconnect_device", { deviceId });
}

// ── Events ───────────────────────────────────────────────────────────────────

export type DeviceEvent =
  | { type: "connected";     deviceId: string; displayName: string; osType: string }
  | { type: "established";   deviceId: string }
  | { type: "pairing";       deviceId: string; pin: string }
  | { type: "disconnected";  deviceId: string }
  | { type: "layoutUpdate";  screens: SpatialScreen[] };

export function onDeviceEvent(cb: (e: DeviceEvent) => void) {
  return listen<DeviceEvent>("device-event", (event) => cb(event.payload));
}
