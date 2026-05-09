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
  lastSeen: number;
}

export interface SpatialScreen {
  deviceId: string;
  x: number;
  y: number;
  widthPx: number;
  heightPx: number;
  isHost: boolean;
  displayName: string;
}

export interface AppState {
  // Devices
  devices: Record<string, Device>;
  setDevice: (d: Device) => void;
  removeDevice: (id: string) => void;
  setDeviceStatus: (id: string, status: Device["status"]) => void;

  // Spatial layout
  spatialScreens: SpatialScreen[];
  setSpatialScreens: (screens: SpatialScreen[]) => void;
  updateScreenPosition: (deviceId: string, x: number, y: number) => void;

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

  // Theme
  theme: "dark" | "light";
  setTheme: (t: "dark" | "light") => void;

  // Settings panel visibility
  showSettings: boolean;
  setShowSettings: (v: boolean) => void;
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

  spatialScreens: [],
  setSpatialScreens: (screens) => set({ spatialScreens: screens }),
  updateScreenPosition: (deviceId, x, y) =>
    set((s) => ({
      spatialScreens: s.spatialScreens.map((sc) =>
        sc.deviceId === deviceId ? { ...sc, x, y } : sc
      ),
    })),

  pairingPin: null,
  pairingDeviceId: null,
  setPairingPin: (pin, deviceId) => set({ pairingPin: pin, pairingDeviceId: deviceId }),

  focusedDeviceId: null,
  setFocusedDevice: (id) => set({ focusedDeviceId: id }),

  hostId: "local",
  listenAddr: "0.0.0.0:7443",

  theme: "dark",
  setTheme: (t) => {
    document.documentElement.setAttribute("data-theme", t);
    set({ theme: t });
  },

  showSettings: false,
  setShowSettings: (v) => set({ showSettings: v }),
}));
