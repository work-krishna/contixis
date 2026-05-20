use crate::state::{AgentState, ConnState};
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, State};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInfo {
    pub device_id:    String,
    pub conn_status:  String,
    pub conn_host_id: Option<String>,
    pub conn_addr:    Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairedHostInfo {
    pub host_id:      String,
    pub display_name: Option<String>,
    pub last_addr:    Option<String>,
    pub is_online:    bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredInfo {
    pub host_id:   String,
    pub addr:      String,
    pub is_paired: bool,
}

#[tauri::command]
pub async fn get_agent_info(state: State<'_, Arc<AgentState>>) -> Result<AgentInfo, String> {
    let device_id = state.identity.lock().device_id.clone();
    let (conn_status, conn_host_id, conn_addr) = match &*state.conn_state.lock() {
        ConnState::Idle => ("idle".into(), None, None),
        ConnState::Connecting { addr } => ("connecting".into(), None, Some(addr.clone())),
        ConnState::PairingRequired { addr } => ("pairing".into(), None, Some(addr.clone())),
        ConnState::Connected { addr, host_id } => {
            ("connected".into(), Some(host_id.clone()), Some(addr.clone()))
        }
    };
    Ok(AgentInfo { device_id, conn_status, conn_host_id, conn_addr })
}

#[tauri::command]
pub async fn get_paired_hosts(state: State<'_, Arc<AgentState>>) -> Result<Vec<PairedHostInfo>, String> {
    let store = state.store.lock();
    let discovered = &state.discovered;
    Ok(store.all_hosts().map(|h| PairedHostInfo {
        host_id:      h.host_id.clone(),
        display_name: h.display_name.clone(),
        last_addr:    h.last_addr.map(|a| a.to_string()),
        is_online:    discovered.contains_key(&h.host_id),
    }).collect())
}

#[tauri::command]
pub async fn get_discovered_hosts(state: State<'_, Arc<AgentState>>) -> Result<Vec<DiscoveredInfo>, String> {
    let store = state.store.lock();
    Ok(state.discovered.iter().map(|e| DiscoveredInfo {
        host_id:   e.host_id.clone(),
        addr:      e.addr.clone(),
        is_paired: store.get_host(&e.host_id).is_some(),
    }).collect())
}

#[tauri::command]
pub async fn connect_to_host(
    addr: String,
    app: AppHandle,
    state: State<'_, Arc<AgentState>>,
) -> Result<(), String> {
    {
        let cs = state.conn_state.lock();
        if !matches!(*cs, ConnState::Idle) {
            return Err("already connecting or connected — disconnect first".into());
        }
    }
    let state_arc = state.inner().clone();
    tauri::async_runtime::spawn(crate::run_connection(addr, app, state_arc));
    Ok(())
}

#[tauri::command]
pub async fn disconnect(state: State<'_, Arc<AgentState>>) -> Result<(), String> {
    if let Some(tx) = state.disconnect_tx.lock().take() {
        let _ = tx.send(false);
    }
    Ok(())
}

#[tauri::command]
pub async fn enter_pin(pin: String, state: State<'_, Arc<AgentState>>) -> Result<(), String> {
    if let Some(tx) = state.pin_tx.lock().take() {
        let _ = tx.send(pin);
        Ok(())
    } else {
        Err("no pairing session in progress".into())
    }
}

#[tauri::command]
pub async fn forget_host(host_id: String, state: State<'_, Arc<AgentState>>) -> Result<(), String> {
    {
        let mut store = state.store.lock();
        store.remove_host(&host_id);
        store.save(&state.store_path).map_err(|e| e.to_string())?;
    }
    Ok(())
}
