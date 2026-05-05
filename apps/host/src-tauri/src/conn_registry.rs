use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Messages the host can send to a connected agent.
#[derive(Debug, Clone)]
pub enum HostMsg {
    FocusTransfer { screen_id: String, entry_x: f32, entry_y: f32 },
    FocusDrop,
    MouseMove    { norm_x: f32, norm_y: f32, screen_id: String },
    MouseButton  { button: u32, pressed: bool, timestamp_us: u64 },
    MouseScroll  { delta_x: f32, delta_y: f32, timestamp_us: u64 },
    KeyEvent     { hid_usage: u32, modifiers: u32, pressed: bool, timestamp_us: u64 },
    GridLayout   { cols: u32, rows: u32, cells: Vec<GridCellMsg> },
    ClipboardData{ seq: u64, content_type: String, data: Vec<u8> },
    Disconnect,
}

#[derive(Debug, Clone)]
pub struct GridCellMsg {
    pub row: u32,
    pub col: u32,
    pub device_id: String,
    pub screen_id: String,
}

/// Thread-safe registry of per-device QUIC send channels.
#[derive(Clone, Default)]
pub struct ConnRegistry {
    inner: Arc<DashMap<String, mpsc::Sender<HostMsg>>>,
}

impl ConnRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn register(&self, device_id: impl Into<String>, tx: mpsc::Sender<HostMsg>) {
        self.inner.insert(device_id.into(), tx);
    }

    pub fn unregister(&self, device_id: &str) {
        self.inner.remove(device_id);
    }

    /// Non-blocking send (drops message if channel is full).
    pub fn send_sync(&self, device_id: &str, msg: HostMsg) -> bool {
        if let Some(tx) = self.inner.get(device_id) {
            tx.try_send(msg).is_ok()
        } else {
            false
        }
    }

    /// Blocking async send.
    pub async fn send(&self, device_id: &str, msg: HostMsg) -> bool {
        if let Some(tx) = self.inner.get(device_id) {
            tx.send(msg).await.is_ok()
        } else {
            false
        }
    }

    /// Non-blocking broadcast to all connected agents.
    pub fn broadcast_sync(&self, msg: HostMsg) {
        for entry in self.inner.iter() {
            let _ = entry.value().try_send(msg.clone());
        }
    }

    pub fn device_ids(&self) -> Vec<String> {
        self.inner.iter().map(|e| e.key().clone()).collect()
    }

    pub fn is_connected(&self, device_id: &str) -> bool {
        self.inner.contains_key(device_id)
    }

    pub fn count(&self) -> usize {
        self.inner.len()
    }
}
