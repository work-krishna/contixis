import { create } from "zustand";

export type ConnStatus = "idle" | "connecting" | "pairing" | "connected";

export interface PairedHost {
  hostId: string;
  displayName?: string;
  lastAddr?: string;
  isOnline: boolean;
}

export interface DiscoveredHost {
  hostId: string;
  addr: string;
  isPaired: boolean;
}

export interface AppState {
  deviceId: string;
  connStatus: ConnStatus;
  connHostId: string | null;
  connAddr: string | null;

  pairedHosts: PairedHost[];
  discoveredHosts: DiscoveredHost[];

  showPinDialog: boolean;

  setAgentInfo: (deviceId: string, status: ConnStatus, hostId: string | null, addr: string | null) => void;
  setPairedHosts: (hosts: PairedHost[]) => void;
  setDiscoveredHosts: (hosts: DiscoveredHost[]) => void;

  addDiscovered: (hostId: string, addr: string, isPaired: boolean) => void;
  removeDiscovered: (hostId: string) => void;

  setConnStatus: (status: ConnStatus, hostId?: string | null, addr?: string | null) => void;
  setShowPinDialog: (v: boolean) => void;

  markPairedOnline: (hostId: string, online: boolean) => void;
}

export const useStore = create<AppState>((set) => ({
  deviceId:       "",
  connStatus:     "idle",
  connHostId:     null,
  connAddr:       null,
  pairedHosts:    [],
  discoveredHosts: [],
  showPinDialog:  false,

  setAgentInfo: (deviceId, connStatus, connHostId, connAddr) =>
    set({ deviceId, connStatus, connHostId, connAddr }),

  setPairedHosts: (pairedHosts) => set({ pairedHosts }),
  setDiscoveredHosts: (discoveredHosts) => set({ discoveredHosts }),

  addDiscovered: (hostId, addr, isPaired) =>
    set((s) => {
      const exists = s.discoveredHosts.some((h) => h.hostId === hostId);
      if (exists) return {};
      return { discoveredHosts: [...s.discoveredHosts, { hostId, addr, isPaired }] };
    }),

  removeDiscovered: (hostId) =>
    set((s) => ({
      discoveredHosts: s.discoveredHosts.filter((h) => h.hostId !== hostId),
    })),

  setConnStatus: (connStatus, connHostId = null, connAddr = null) =>
    set({ connStatus, connHostId, connAddr }),

  setShowPinDialog: (showPinDialog) => set({ showPinDialog }),

  markPairedOnline: (hostId, online) =>
    set((s) => ({
      pairedHosts: s.pairedHosts.map((h) =>
        h.hostId === hostId ? { ...h, isOnline: online } : h
      ),
    })),
}));
