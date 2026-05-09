use crate::conn_registry::HostMsg;
use crate::server::{self, broadcast_grid_layout, collect_layout_screens, emit, emit_layout_update, DeviceEvent};
use crate::state::HostState;
use contixis_core::ScreenPlacement;
use contixis_input::platform;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, State};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridCellPayload {
    pub row: u32,
    pub col: u32,
    pub device_id: Option<String>,
    pub screen_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenPositionPayload {
    pub device_id: String,
    pub x: i32,
    pub y: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostScreenInfo {
    pub device_id: String,
    pub x:         i32,
    pub y:         i32,
    pub width_px:  u32,
    pub height_px: u32,
    pub name:      String,
    pub hostname:  String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostInfo {
    pub host_id:      String,
    pub addr:         String,
    pub host_screens: Vec<HostScreenInfo>,
}

#[tauri::command]
pub async fn start_server(
    addr: String,
    app: AppHandle,
    state: State<'_, Arc<HostState>>,
) -> Result<(), String> {
    let listen_addr: std::net::SocketAddr = addr
        .parse()
        .map_err(|e| format!("invalid address: {}", e))?;

    if state.server_tx.lock().is_some() {
        return Err("server already running".into());
    }

    let state_arc = state.inner().clone();
    let shutdown_tx = server::start(listen_addr, app, state_arc)
        .await
        .map_err(|e| format!("server start failed: {}", e))?;

    *state.server_tx.lock() = Some(shutdown_tx);
    Ok(())
}

#[tauri::command]
pub async fn stop_server(state: State<'_, Arc<HostState>>) -> Result<(), String> {
    if let Some(tx) = state.server_tx.lock().take() {
        let _ = tx.send(false);
    }
    Ok(())
}

#[tauri::command]
pub async fn get_host_info(state: State<'_, Arc<HostState>>) -> Result<HostInfo, String> {
    let hostname = std::fs::read_to_string("/etc/hostname")
        .unwrap_or_else(|_| "host".to_string())
        .trim()
        .to_string();

    let host_screens = state.host_monitors.read()
        .iter()
        .map(|m| HostScreenInfo {
            device_id: m.device_id.clone(),
            x:         m.x,
            y:         m.y,
            width_px:  m.width  as u32,
            height_px: m.height as u32,
            name:      m.name.clone(),
            hostname:  hostname.clone(),
        })
        .collect();

    Ok(HostInfo {
        host_id:      hex::encode(&state.ca.cert_der()[..8]),
        addr:         state.config.listen_addr.to_string(),
        host_screens,
    })
}

#[tauri::command]
pub async fn update_grid_layout(
    cells: Vec<GridCellPayload>,
    state: State<'_, Arc<HostState>>,
) -> Result<(), String> {
    use contixis_core::grid::{GridPosition, ScreenInfo};

    {
        let mut grid = state.grid.write();
        for device_id in state.connections.device_ids() {
            grid.remove_screen(&device_id);
        }
        for cell in &cells {
            if let (Some(device_id), Some(_screen_id)) = (&cell.device_id, &cell.screen_id) {
                let pos = GridPosition { row: cell.row as u8, col: cell.col as u8 };
                grid.place_screen(pos, ScreenInfo {
                    device_id: device_id.clone(),
                    width_px: 1920,
                    height_px: 1080,
                    scale_factor: 1.0,
                });
            }
        }
    }

    broadcast_grid_layout(&state);
    Ok(())
}

#[tauri::command]
pub async fn disconnect_device(
    device_id: String,
    app: AppHandle,
    state: State<'_, Arc<HostState>>,
) -> Result<(), String> {
    state.connections.send_sync(&device_id, HostMsg::FocusDrop);
    state.connections.send_sync(&device_id, HostMsg::Disconnect);
    state.connections.unregister(&device_id);
    state.registry.remove(&device_id);
    state.grid.write().remove_screen(&device_id);
    // Remove immediately from spatial layout so the canvas updates right away
    // (the QUIC connection may take a moment to fully close; we don't want to wait).
    state.layout.write().remove(&device_id);
    {
        let mut focused = state.focused_device.lock();
        if focused.as_deref() == Some(device_id.as_str()) {
            *focused = None;
            platform::set_focus_active(false);
        }
    }
    // Tell the frontend immediately — don't wait for the QUIC close event.
    emit_layout_update(&app, &state);
    emit(&app, DeviceEvent::Disconnected { device_id: device_id.clone() });
    tracing::info!(%device_id, "device disconnected by user");
    Ok(())
}

/// Return all screens currently in the spatial layout (host + agents).
/// Used on frontend mount to populate the canvas with any already-connected agents.
#[tauri::command]
pub async fn get_spatial_layout(
    state: State<'_, Arc<HostState>>,
) -> Result<Vec<server::ScreenUiPayload>, String> {
    Ok(collect_layout_screens(&state))
}

/// Update the pixel positions of agent screens in the spatial layout (from UI drag).
/// Only positions are updated; screen dimensions come from the agent's reported resolution.
#[tauri::command]
pub async fn update_spatial_layout(
    screens: Vec<ScreenPositionPayload>,
    app: AppHandle,
    state: State<'_, Arc<HostState>>,
) -> Result<(), String> {
    {
        let mut layout = state.layout.write();
        for sp in &screens {
            if let Some(existing) = layout.find_mut(&sp.device_id) {
                existing.x = sp.x;
                existing.y = sp.y;
            }
        }
    }
    emit_layout_update(&app, &state);
    broadcast_grid_layout(&state);
    Ok(())
}

/// Returns the current focus-release shortcut as (keysym, modifiers).
#[tauri::command]
pub async fn get_release_shortcut(
    state: State<'_, Arc<HostState>>,
) -> Result<(u32, u32), String> {
    Ok(*state.release_shortcut.lock())
}

/// Updates the focus-release shortcut at runtime without restarting.
#[tauri::command]
pub async fn set_release_shortcut(
    keysym: u32,
    modifiers: u32,
    state: State<'_, Arc<HostState>>,
) -> Result<(), String> {
    if keysym == 0 {
        return Err("keysym must be non-zero".into());
    }
    *state.release_shortcut.lock() = (keysym, modifiers);
    Ok(())
}

fn autostart_desktop_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("autostart").join("contixis.desktop"))
}

#[tauri::command]
pub async fn get_autostart() -> bool {
    autostart_desktop_path()
        .map(|p| p.exists())
        .unwrap_or(false)
}

#[tauri::command]
pub async fn toggle_autostart(enable: bool) -> Result<(), String> {
    let path = autostart_desktop_path().ok_or("cannot determine config directory")?;
    if enable {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let exe = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .to_string();
        let content = format!(
            "[Desktop Entry]\nType=Application\nName=Contixis\nExec={exe}\nHidden=false\nNoDisplay=false\nX-GNOME-Autostart-enabled=true\n"
        );
        std::fs::write(&path, content).map_err(|e| e.to_string())?;
    } else {
        let _ = std::fs::remove_file(&path);
    }
    Ok(())
}
