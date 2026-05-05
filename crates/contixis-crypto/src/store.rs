use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;

/// A trusted host record stored on an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedHost {
    pub host_id: String,
    /// DER-encoded CA certificate for this host's PKI.
    pub ca_der: Vec<u8>,
    /// Last-known address — used as a reconnect hint.
    pub last_addr: Option<SocketAddr>,
    /// Friendly display name chosen by the host.
    pub display_name: Option<String>,
}

/// A trusted agent record stored on the host.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustedAgent {
    pub device_id: String,
    /// DER-encoded agent certificate (issued by this host's CA).
    pub cert_der: Vec<u8>,
    pub display_name: Option<String>,
}

/// Persists paired-host records for the agent side.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AgentStore {
    trusted_hosts: HashMap<String, TrustedHost>,
}

impl AgentStore {
    pub fn load(path: &Path) -> Result<Self> {
        if path.exists() {
            let data = std::fs::read(path).context("reading agent store")?;
            let store = serde_json::from_slice(&data).context("parsing agent store")?;
            Ok(store)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_vec_pretty(self)?;
        std::fs::write(path, data).context("writing agent store")?;
        Ok(())
    }

    pub fn add_host(&mut self, host: TrustedHost) {
        self.trusted_hosts.insert(host.host_id.clone(), host);
    }

    pub fn remove_host(&mut self, host_id: &str) {
        self.trusted_hosts.remove(host_id);
    }

    pub fn get_host(&self, host_id: &str) -> Option<&TrustedHost> {
        self.trusted_hosts.get(host_id)
    }

    pub fn update_last_addr(&mut self, host_id: &str, addr: SocketAddr) {
        if let Some(h) = self.trusted_hosts.get_mut(host_id) {
            h.last_addr = Some(addr);
        }
    }

    pub fn all_hosts(&self) -> impl Iterator<Item = &TrustedHost> {
        self.trusted_hosts.values()
    }
}

/// Persists paired-agent records for the host side.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct HostStore {
    trusted_agents: HashMap<String, TrustedAgent>,
}

impl HostStore {
    pub fn load(path: &Path) -> Result<Self> {
        if path.exists() {
            let data = std::fs::read(path).context("reading host store")?;
            let store = serde_json::from_slice(&data).context("parsing host store")?;
            Ok(store)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_vec_pretty(self)?;
        std::fs::write(path, data).context("writing host store")?;
        Ok(())
    }

    pub fn add_agent(&mut self, agent: TrustedAgent) {
        self.trusted_agents.insert(agent.device_id.clone(), agent);
    }

    pub fn remove_agent(&mut self, device_id: &str) {
        self.trusted_agents.remove(device_id);
    }

    pub fn get_agent(&self, device_id: &str) -> Option<&TrustedAgent> {
        self.trusted_agents.get(device_id)
    }

    pub fn is_trusted(&self, device_id: &str) -> bool {
        self.trusted_agents.contains_key(device_id)
    }

    pub fn all_agents(&self) -> impl Iterator<Item = &TrustedAgent> {
        self.trusted_agents.values()
    }
}
