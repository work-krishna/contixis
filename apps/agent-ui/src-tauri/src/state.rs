use contixis_crypto::{AgentStore, DeviceIdentity};
use dashmap::DashMap;
use parking_lot::Mutex;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::oneshot;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredEntry {
    pub host_id: String,
    pub addr: String,
    /// Full mDNS instance name (used to match removal events).
    pub instance: String,
}

#[derive(Debug, Default)]
pub enum ConnState {
    #[default]
    Idle,
    Connecting { addr: String },
    PairingRequired { addr: String },
    Connected { addr: String, host_id: String },
}

pub struct AgentState {
    pub identity:      Arc<Mutex<DeviceIdentity>>,
    pub store:         Arc<Mutex<AgentStore>>,
    pub store_path:    PathBuf,
    pub discovered:    Arc<DashMap<String, DiscoveredEntry>>,
    pub conn_state:    Arc<Mutex<ConnState>>,
    /// Filled by the connection task when pairing is required; consumed by enter_pin command.
    pub pin_tx:        Arc<Mutex<Option<oneshot::Sender<String>>>>,
    /// Sending false shuts down the active connection loop.
    pub disconnect_tx: Arc<Mutex<Option<tokio::sync::watch::Sender<bool>>>>,
}
