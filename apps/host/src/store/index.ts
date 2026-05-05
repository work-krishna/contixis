import { create } from "zustand";

export interface ScreenInfo {
  id: string;
  widthPx: number;
  heightPx: number;
  dpiScale: number;
  isPrimary: boolean;
}

export interface Device {
  deviceId: string;
  displayName: string;
  osType: string;
  status: "connected" | "established" | "pairing" | "disconnected";
  screens: ScreenInfo[];
  gridRow?: number;
  gridCol?: number;
  lastSeen: number;
}

export interface GridCell {
  row: number;
  col: number;
  deviceId?: string;
  screenId?: string;
}

export interface AppState {
  // Devices
  devices: Record<string, Device>;
  setDevice: (d: Device) => void;
  removeDevice: (id: string) => void;
  setDeviceStatus: (id: string, status: Device["status"]) => void;

  // Grid layout (3×3)
  gridRows: number;
  gridCols: number;
  grid: GridCell[];
  placeDevice: (deviceId: string, row: number, col: number, screenId?: string) => void;
  removeFromGrid: (deviceId: string) => void;

  // Pairing
  pairingPin: string | null;
  pairingDeviceId: string | null;
  setPairingPin: (pin: string | null, deviceId: string | null) => void;

  // Active focus
  focusedDeviceId: string | null;
  setFocusedDevice: (id: string | null) => void;

  // Host info
  hostId: string;
  listenAddr: string;
  hostCells: Array<{ row: number; col: number; name: string; hostname: string }>;
  setHostCells: (cells: Array<{ row: number; col: number; name: string; hostname: string }>) => void;
}

function buildInitialGrid(rows: number, cols: number): GridCell[] {
  const cells: GridCell[] = [];
  for (let r = 0; r < rows; r++) {
    for (let c = 0; c < cols; c++) {
      cells.push({ row: r, col: c });
    }
  }
  return cells;
}

export const useStore = create<AppState>((set) => ({
  devices: {},
  setDevice: (d) => set((s) => ({ devices: { ...s.devices, [d.deviceId]: d } })),
  removeDevice: (id) =>
    set((s) => {
      const devices = { ...s.devices };
      delete devices[id];
      return { devices };
    }),
  setDeviceStatus: (id, status) =>
    set((s) => ({
      devices: s.devices[id]
        ? { ...s.devices, [id]: { ...s.devices[id], status } }
        : s.devices,
    })),

  gridRows: 3,
  gridCols: 3,
  grid: buildInitialGrid(3, 3),
  placeDevice: (deviceId, row, col, screenId) =>
    set((s) => ({
      grid: s.grid.map((cell) =>
        cell.row === row && cell.col === col
          ? { ...cell, deviceId, screenId }
          : cell.deviceId === deviceId
          ? { ...cell, deviceId: undefined, screenId: undefined }
          : cell
      ),
    })),
  removeFromGrid: (deviceId) =>
    set((s) => ({
      grid: s.grid.map((cell) =>
        cell.deviceId === deviceId
          ? { ...cell, deviceId: undefined, screenId: undefined }
          : cell
      ),
    })),

  pairingPin: null,
  pairingDeviceId: null,
  setPairingPin: (pin, deviceId) => set({ pairingPin: pin, pairingDeviceId: deviceId }),

  focusedDeviceId: null,
  setFocusedDevice: (id) => set({ focusedDeviceId: id }),

  hostId: "local",
  listenAddr: "0.0.0.0:7443",
  hostCells: [{ row: 1, col: 1, name: "Display", hostname: "host" }],
  setHostCells: (cells) => set({ hostCells: cells }),
}));
