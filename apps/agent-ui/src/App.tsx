import { useEffect, useState } from "react";
import { useStore } from "./store";
import { HostCard } from "./components/HostCard";
import { PinDialog } from "./components/PinDialog";
import {
  getAgentInfo,
  getPairedHosts,
  getDiscoveredHosts,
  onAgentEvent,
} from "./lib/tauri";

export default function App() {
  const {
    deviceId, connStatus, connHostId,
    pairedHosts, discoveredHosts,
    showPinDialog,
    setAgentInfo, setPairedHosts, setDiscoveredHosts,
    addDiscovered, removeDiscovered,
    setConnStatus, setShowPinDialog,
    markPairedOnline,
  } = useStore();

  const [loading, setLoading] = useState(true);
  const [scanning, setScanning] = useState(false);

  const refreshDiscovered = async () => {
    setScanning(true);
    try {
      const discovered = await getDiscoveredHosts();
      setDiscoveredHosts(discovered);
      discovered.forEach((h) => markPairedOnline(h.hostId, true));
    } catch (e) {
      console.error(e);
    } finally {
      setScanning(false);
    }
  };

  useEffect(() => {
    Promise.all([getAgentInfo(), getPairedHosts(), getDiscoveredHosts()])
      .then(([info, paired, discovered]) => {
        setAgentInfo(info.deviceId, info.connStatus, info.connHostId, info.connAddr);
        setPairedHosts(paired);
        setDiscoveredHosts(discovered);
      })
      .catch(console.error)
      .finally(() => setLoading(false));

    const unsubPromise = onAgentEvent((event) => {
      switch (event.type) {
        case "hostDiscovered": {
          const isPaired = useStore.getState().pairedHosts.some(
            (h) => h.hostId === event.hostId
          );
          addDiscovered(event.hostId, event.addr, isPaired);
          markPairedOnline(event.hostId, true);
          break;
        }
        case "hostLost":
          removeDiscovered(event.hostId);
          markPairedOnline(event.hostId, false);
          break;
        case "statusChange":
          if (event.status === "pairing") {
            setShowPinDialog(true);
          } else {
            setShowPinDialog(false);
          }
          setConnStatus(
            event.status,
            event.hostId ?? null,
            event.addr   ?? null
          );
          break;
      }
    });

    return () => { unsubPromise.then((fn) => fn()).catch(() => {}); };
  }, []);

  const statusColor =
    connStatus === "connected"  ? "var(--success)"
    : connStatus === "connecting" ? "var(--accent)"
    : connStatus === "pairing"    ? "var(--warning)"
    : "var(--text-dim)";

  const statusLabel =
    connStatus === "connected"  ? `Connected · ${connHostId?.slice(0, 8) ?? ""}`
    : connStatus === "connecting" ? "Connecting…"
    : connStatus === "pairing"    ? "Pairing…"
    : "Idle";

  // Hosts shown in Discovered section: mDNS hits not already in Paired.
  const newlyDiscovered = discoveredHosts.filter(
    (d) => !pairedHosts.some((p) => p.hostId === d.hostId)
  );

  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column", background: "var(--bg)" }}>
      {/* Header */}
      <div
        style={{
          padding: "14px 16px",
          borderBottom: "1px solid var(--border)",
          background: "var(--surface)",
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
        }}
      >
        <div>
          <div style={{ fontWeight: 700, fontSize: 15 }}>Contixis Agent</div>
          <div style={{ fontSize: 11, color: "var(--text-dim)", marginTop: 2 }}>
            {deviceId ? deviceId.slice(0, 18) + "…" : "loading…"}
          </div>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <span
            style={{
              width: 8, height: 8, borderRadius: "50%",
              background: statusColor,
              display: "inline-block",
            }}
          />
          <span style={{ fontSize: 12, color: statusColor }}>{statusLabel}</span>
        </div>
      </div>

      {/* Body */}
      <div style={{ flex: 1, overflowY: "auto" }}>
        {loading ? (
          <p style={{ padding: 24, color: "var(--text-dim)", fontSize: 12, textAlign: "center" }}>
            Loading…
          </p>
        ) : (
          <>
            {/* Discovered section */}
            <Section title="Discovered" count={newlyDiscovered.length} onRefresh={refreshDiscovered} refreshing={scanning}>
              {newlyDiscovered.length === 0 ? (
                <Empty>Scanning for hosts on the local network…</Empty>
              ) : (
                newlyDiscovered.map((h) => (
                  <HostCard
                    key={h.hostId}
                    hostId={h.hostId}
                    label={h.hostId.slice(0, 8)}
                    sublabel={h.addr}
                    addr={h.addr}
                    isOnline
                    isPaired={h.isPaired}
                  />
                ))
              )}
            </Section>

            {/* Paired section */}
            <Section title="Paired Hosts" count={pairedHosts.length}>
              {pairedHosts.length === 0 ? (
                <Empty>No paired hosts yet — connect to a host to pair.</Empty>
              ) : (
                pairedHosts.map((h) => {
                  const discovered = discoveredHosts.find((d) => d.hostId === h.hostId);
                  return (
                    <HostCard
                      key={h.hostId}
                      hostId={h.hostId}
                      label={h.displayName ?? h.hostId.slice(0, 8)}
                      sublabel={
                        h.isOnline && discovered
                          ? discovered.addr
                          : h.lastAddr ?? "last address unknown"
                      }
                      addr={discovered?.addr ?? h.lastAddr}
                      isOnline={h.isOnline}
                      isPaired
                      canForget
                    />
                  );
                })
              )}
            </Section>
          </>
        )}
      </div>

      {/* Pairing PIN dialog */}
      {showPinDialog && (
        <PinDialog onDismiss={() => setShowPinDialog(false)} />
      )}
    </div>
  );
}

function Section({
  title, count, children, onRefresh, refreshing,
}: {
  title: string;
  count: number;
  children: React.ReactNode;
  onRefresh?: () => void;
  refreshing?: boolean;
}) {
  return (
    <div>
      <div
        style={{
          padding: "6px 16px",
          fontSize: 11,
          fontWeight: 600,
          color: "var(--text-dim)",
          textTransform: "uppercase",
          letterSpacing: "0.06em",
          borderBottom: "1px solid var(--border)",
          background: "var(--surface)",
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
        }}
      >
        <span>{title} <span style={{ fontWeight: 400 }}>({count})</span></span>
        {onRefresh && (
          <button
            className="ghost"
            onClick={onRefresh}
            disabled={refreshing}
            style={{ fontSize: 11, padding: "2px 10px" }}
          >
            {refreshing ? "Scanning…" : "Refresh"}
          </button>
        )}
      </div>
      {children}
    </div>
  );
}

function Empty({ children }: { children: React.ReactNode }) {
  return (
    <p style={{ padding: "20px 16px", color: "var(--text-dim)", fontSize: 12 }}>
      {children}
    </p>
  );
}
