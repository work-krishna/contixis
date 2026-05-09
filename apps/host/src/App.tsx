import { useState, useEffect } from "react";
import { Titlebar } from "./components/Titlebar";
import { TabBar } from "./components/TabBar";
import { SpatialCanvas } from "./components/SpatialCanvas";
import { DeviceList } from "./components/DeviceList";
import { HelpTab } from "./components/HelpTab";
import { AboutTab } from "./components/AboutTab";
import { SettingsPanel } from "./components/SettingsPanel";
import { useStore } from "./store";
import { onDeviceEvent, startServer, getSpatialLayout } from "./lib/tauri";

type Tab = "view" | "help" | "about";

export default function App() {
  const [activeTab, setActiveTab] = useState<Tab>("view");
  const {
    setDevice, setDeviceStatus, setPairingPin,
    setSpatialScreens,
    listenAddr,
  } = useStore();

  useEffect(() => {
    let cancelled = false;

    startServer(listenAddr).catch(console.error);

    // Load all current screens (host + any already-connected agents).
    // Also synthesise Device entries for agents so Titlebar/DeviceList aren't empty
    // after a WebView reload (Rust process keeps running, no events re-fire).
    getSpatialLayout()
      .then((screens) => {
        if (cancelled) return;
        setSpatialScreens(screens);
        screens.filter(s => !s.isHost).forEach(s => setDevice({
          deviceId:    s.deviceId,
          displayName: s.displayName,
          osType:      "",
          status:      "established",
          screens:     [],
          lastSeen:    Date.now(),
        }));
      })
      .catch(console.error);

    const unsubPromise = onDeviceEvent((event) => {
      if (cancelled) return;
      console.debug("[contixis] device-event:", event.type, event);
      switch (event.type) {
        case "connected":
          setDevice({
            deviceId:    event.deviceId,
            displayName: event.displayName,
            osType:      event.osType,
            status:      "connected",
            screens:     [],
            lastSeen:    Date.now(),
          });
          break;
        case "pairing":
          setDevice({
            ...useStore.getState().devices[event.deviceId] ?? {
              deviceId:    event.deviceId,
              displayName: event.deviceId.slice(0, 8),
              osType:      "unknown",
              screens:     [],
              lastSeen:    Date.now(),
            },
            status: "pairing",
          });
          setPairingPin(event.pin, event.deviceId);
          break;
        case "established":
          if (!useStore.getState().devices[event.deviceId]) {
            setDevice({
              deviceId:    event.deviceId,
              displayName: event.deviceId.slice(0, 8),
              osType:      "unknown",
              status:      "established",
              screens:     [],
              lastSeen:    Date.now(),
            });
          } else {
            setDeviceStatus(event.deviceId, "established");
          }
          setPairingPin(null, null);
          break;
        case "disconnected":
          setDeviceStatus(event.deviceId, "disconnected");
          // Remove agent's screen from the canvas immediately.
          // The layoutUpdate that follows confirms the Rust-side removal.
          setSpatialScreens(
            useStore.getState().spatialScreens.filter(s => s.deviceId !== event.deviceId)
          );
          break;
        case "layoutUpdate":
          setSpatialScreens(event.screens);
          break;
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
      <TabBar active={activeTab} onChange={setActiveTab} />
      <div style={{ flex: 1, display: "flex", overflow: "hidden" }}>
        {activeTab === "view" && (
          <>
            <SpatialCanvas />
            <DeviceList />
          </>
        )}
        {activeTab === "help"  && <HelpTab />}
        {activeTab === "about" && <AboutTab />}
      </div>
      <SettingsPanel />
    </div>
  );
}
