import { useEffect } from "react";
import { Titlebar } from "./components/Titlebar";
import { GridCanvas } from "./components/GridCanvas";
import { DeviceList } from "./components/DeviceList";
import { useStore } from "./store";
import { onDeviceEvent, startServer, getHostInfo } from "./lib/tauri";

export default function App() {
  const { setDevice, setDeviceStatus, setPairingPin, setHostCells, placeDevice, removeFromGrid, listenAddr } = useStore();

  useEffect(() => {
    let cancelled = false;

    // Register the event listener first, THEN start the server.
    // onDeviceEvent returns a Promise — we must await it before starting the
    // server so no events can fire before the listener is registered.
    // Start server and load host cells immediately — these don't depend on
    // the device-event listener being registered first.
    startServer(listenAddr).catch(console.error);
    getHostInfo()
      .then((info) => { if (!cancelled) setHostCells(info.hostCells); })
      .catch(console.error);

    // Register device-event listener separately.
    const unsubPromise = onDeviceEvent((event) => {
      if (cancelled) return;
      console.debug("[contixis] device-event:", event.type, event);
      switch (event.type) {
        case "connected":
          setDevice({
            deviceId: event.deviceId,
            displayName: event.displayName,
            osType: event.osType,
            status: "connected",
            screens: [],
            lastSeen: Date.now(),
          });
          break;
        case "pairing":
          setDevice({
            ...useStore.getState().devices[event.deviceId] ?? {
              deviceId: event.deviceId,
              displayName: event.deviceId.slice(0, 8),
              osType: "unknown",
              screens: [],
              lastSeen: Date.now(),
            },
            status: "pairing",
          });
          setPairingPin(event.pin, event.deviceId);
          break;
        case "established":
          // Upsert: create device if Connected event was missed
          if (!useStore.getState().devices[event.deviceId]) {
            setDevice({
              deviceId: event.deviceId,
              displayName: event.deviceId.slice(0, 8),
              osType: "unknown",
              status: "established",
              screens: [],
              lastSeen: Date.now(),
            });
          } else {
            setDeviceStatus(event.deviceId, "established");
          }
          setPairingPin(null, null);
          break;
        case "disconnected":
          setDeviceStatus(event.deviceId, "disconnected");
          break;
        case "gridUpdate": {
          // Rebuild grid from authoritative Rust state
          const currentGrid = useStore.getState().grid;
          // Clear cells that no longer have a device
          currentGrid.forEach((c) => {
            if (c.deviceId && !event.cells.find((nc) => nc.deviceId === c.deviceId)) {
              removeFromGrid(c.deviceId);
            }
          });
          // Place/update cells from Rust
          event.cells.forEach((c) => placeDevice(c.deviceId, c.row, c.col, c.screenId));
          break;
        }
      }
    });

    unsubPromise.catch((e) => console.error("[contixis] listen failed:", e));

    return () => {
      cancelled = true;
      unsubPromise.then((fn) => fn()).catch(() => {});
    };
  }, []);

  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column" }}>
      <Titlebar />
      <div style={{ flex: 1, display: "flex", overflow: "hidden" }}>
        <GridCanvas />
        <DeviceList />
      </div>
    </div>
  );
}
