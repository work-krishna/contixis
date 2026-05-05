use crate::grid::GridPosition;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceStatus {
    Connected,
    Established,
    Disconnected,
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub device_id: String,
    pub display_name: Option<String>,
    pub addr: SocketAddr,
    pub status: DeviceStatus,
    pub grid_pos: Option<GridPosition>,
    pub connected_at: Instant,
}

/// Thread-safe registry of all currently-connected devices.
#[derive(Clone)]
pub struct DeviceRegistry {
    devices: Arc<RwLock<HashMap<String, DeviceInfo>>>,
}

impl DeviceRegistry {
    pub fn new() -> Self {
        Self { devices: Arc::new(RwLock::new(HashMap::new())) }
    }

    pub fn insert(&self, info: DeviceInfo) {
        self.devices.write().insert(info.device_id.clone(), info);
    }

    pub fn remove(&self, device_id: &str) -> Option<DeviceInfo> {
        self.devices.write().remove(device_id)
    }

    pub fn get(&self, device_id: &str) -> Option<DeviceInfo> {
        self.devices.read().get(device_id).cloned()
    }

    pub fn set_status(&self, device_id: &str, status: DeviceStatus) {
        if let Some(d) = self.devices.write().get_mut(device_id) {
            d.status = status;
        }
    }

    pub fn set_grid_pos(&self, device_id: &str, pos: Option<GridPosition>) {
        if let Some(d) = self.devices.write().get_mut(device_id) {
            d.grid_pos = pos;
        }
    }

    pub fn all(&self) -> Vec<DeviceInfo> {
        self.devices.read().values().cloned().collect()
    }

    pub fn established_count(&self) -> usize {
        self.devices.read().values()
            .filter(|d| d.status == DeviceStatus::Established)
            .count()
    }
}

impl Default for DeviceRegistry {
    fn default() -> Self { Self::new() }
}
