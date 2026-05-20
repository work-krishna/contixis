import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ConnStatus, DiscoveredHost, PairedHost } from "../store";

// ── Commands ────────────────────────────────────────────────────────────────

export interface AgentInfo {
  deviceId:    string;
  connStatus:  ConnStatus;
  connHostId:  string | null;
  connAddr:    string | null;
}

export function getAgentInfo(): Promise<AgentInfo> {
  return invoke("get_agent_info");
}

export function getPairedHosts(): Promise<PairedHost[]> {
  return invoke("get_paired_hosts");
}

export function getDiscoveredHosts(): Promise<DiscoveredHost[]> {
  return invoke("get_discovered_hosts");
}

export function connectToHost(addr: string): Promise<void> {
  return invoke("connect_to_host", { addr });
}

export function disconnect(): Promise<void> {
  return invoke("disconnect");
}

export function enterPin(pin: string): Promise<void> {
  return invoke("enter_pin", { pin });
}

export function forgetHost(hostId: string): Promise<void> {
  return invoke("forget_host", { hostId });
}

// ── Events ───────────────────────────────────────────────────────────────────

export type AgentEvent =
  | { type: "hostDiscovered"; hostId: string; addr: string; instance: string }
  | { type: "hostLost";       hostId: string }
  | { type: "statusChange";   status: ConnStatus; hostId?: string; addr?: string };

export function onAgentEvent(cb: (e: AgentEvent) => void) {
  return listen<AgentEvent>("agent-event", (event) => cb(event.payload));
}
