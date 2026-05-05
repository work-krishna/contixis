use crate::conn_registry::{GridCellMsg, HostMsg};
use crate::state::HostState;
use anyhow::Result;
use contixis_crypto::DeviceIdentity;
use contixis_net::{make_host_tls_config_open, FrameReader, FrameWriter, HandshakeEvent, HostSession};
use contixis_proto::{ClipboardData as ProtoCbData, MsgType};
use prost::Message as ProstMessage;
use quinn::Endpoint;
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, watch};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DeviceEvent {
    Connected {
        #[serde(rename = "deviceId")]   device_id:    String,
        #[serde(rename = "displayName")]display_name: String,
        #[serde(rename = "osType")]     os_type:      String,
    },
    Pairing {
        #[serde(rename = "deviceId")] device_id: String,
        pin: String,
    },
    Established {
        #[serde(rename = "deviceId")] device_id: String,
    },
    Disconnected {
        #[serde(rename = "deviceId")] device_id: String,
    },
    GridUpdate {
        cells: Vec<GridCellUiPayload>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct GridCellUiPayload {
    pub row:       u32,
    pub col:       u32,
    #[serde(rename = "deviceId")]  pub device_id:  String,
    #[serde(rename = "screenId")]  pub screen_id:  String,
}

fn emit(app: &AppHandle, event: DeviceEvent) {
    if let Err(e) = app.emit("device-event", &event) {
        tracing::warn!(error = %e, "emit failed");
    }
}

fn emit_grid_update(app: &AppHandle, state: &HostState) {
    let cells: Vec<GridCellUiPayload> = state.grid.read().cells().iter()
        .filter_map(|c| c.screen.as_ref().map(|s| GridCellUiPayload {
            row:       c.position.row as u32,
            col:       c.position.col as u32,
            device_id: s.device_id.clone(),
            screen_id: s.device_id.clone(),
        }))
        .collect();
    emit(app, DeviceEvent::GridUpdate { cells });
}

pub async fn start(
    addr: SocketAddr,
    app: AppHandle,
    host_state: Arc<HostState>,
) -> Result<watch::Sender<bool>> {
    let (shutdown_tx, mut shutdown_rx) = watch::channel(true);

    let identity = DeviceIdentity::generate()?;
    let cert_der = identity
        .self_signed_cert_der()
        .map_err(|e| anyhow::anyhow!("server cert: {}", e))?;
    let key_pem = identity.private_key_pem();
    let server_cfg = make_host_tls_config_open(cert_der, &key_pem)?;

    let endpoint = Endpoint::server(server_cfg, addr)?;
    tracing::info!(%addr, "QUIC server listening");

    let host_id = hex::encode(&host_state.ca.cert_der()[..8]);
    // Advertise via mDNS.  The MdnsDiscovery value must be kept alive inside
    // the server task — dropping it shuts down the ServiceDaemon and immediately
    // unregisters the service, making the host invisible to agents.
    let mdns = contixis_net::MdnsDiscovery::new().ok();
    if let Some(ref m) = mdns {
        if let Err(e) = m.advertise(&host_id, addr.port()) {
            tracing::warn!(error = %e, "mDNS advertise failed");
        }
    }

    tokio::spawn(async move {
        let _mdns = mdns; // keep ServiceDaemon alive for the duration of the server
        loop {
            tokio::select! {
                Some(incoming) = endpoint.accept() => {
                    let app2   = app.clone();
                    let state2 = host_state.clone();
                    tokio::spawn(handle_connection(incoming, app2, state2));
                }
                _ = shutdown_rx.changed() => {
                    if !*shutdown_rx.borrow() {
                        tracing::info!("QUIC server shutting down");
                        endpoint.close(0u32.into(), b"stopping");
                        break;
                    }
                }
            }
        }
    });

    Ok(shutdown_tx)
}

async fn handle_connection(
    incoming: quinn::Incoming,
    app: AppHandle,
    state: Arc<HostState>,
) {
    let remote = incoming.remote_address();
    tracing::info!(%remote, "incoming QUIC connection");

    let conn = match incoming.await {
        Ok(c) => c,
        Err(e) => { tracing::warn!(%remote, error = %e, "accept failed"); return; }
    };

    let mut session = HostSession::new(
        conn.clone(),
        state.ca.clone(),
        state.pairing.clone(),
        state.store.clone(),
    );

    let app_ref = app.clone();
    let info = match session.run_handshake(|ev| match ev {
        HandshakeEvent::Connected { device_id, display_name, os_type } =>
            emit(&app_ref, DeviceEvent::Connected { device_id, display_name, os_type }),
        HandshakeEvent::PairingPin { device_id, pin } =>
            emit(&app_ref, DeviceEvent::Pairing { device_id, pin }),
    }).await {
        Ok(i)  => i,
        Err(e) => { tracing::warn!(%remote, error = %e, "handshake failed"); return; }
    };

    let device_id = info.device_id.clone();

    {
        use contixis_core::device::{DeviceInfo, DeviceStatus};
        use contixis_core::grid::{GridPosition, ScreenInfo};
        state.registry.insert(DeviceInfo {
            device_id: device_id.clone(),
            display_name: if info.display_name.is_empty() { None } else { Some(info.display_name.clone()) },
            addr: remote,
            status: DeviceStatus::Established,
            grid_pos: None,
            connected_at: std::time::Instant::now(),
        });

        let agent_col = {
            let g = state.grid.read();
            g.cols.saturating_sub(1) as u8
        };
        // Place agent screens in the rightmost column, at the same row as the
        // adjacent host screen (col-1) so edge_transition finds them when the
        // cursor exits the rightmost host monitor.
        let start_row: u8 = {
            let g = state.grid.read();
            let adj_col = agent_col.saturating_sub(1);
            g.cells().iter()
                .filter(|c| c.position.col == adj_col && c.screen.is_some())
                .map(|c| c.position.row)
                .min()
                .unwrap_or(0)
        };
        let mut grid = state.grid.write();
        for (i, screen) in info.screens.iter().enumerate() {
            let pos = GridPosition { row: start_row + i as u8, col: agent_col };
            tracing::info!(
                device_id = %device_id, row = pos.row, col = pos.col,
                "placing agent screen on grid"
            );
            grid.place_screen(pos, ScreenInfo {
                device_id: device_id.clone(),
                width_px: screen.width_px,
                height_px: screen.height_px,
                scale_factor: screen.dpi_scale,
            });
        }
    }

    emit(&app, DeviceEvent::Established { device_id: device_id.clone() });
    emit_grid_update(&app, &state);

    let (msg_tx, msg_rx) = mpsc::channel::<HostMsg>(512);
    state.connections.register(device_id.clone(), msg_tx);

    let send = match conn.open_uni().await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(%device_id, error = %e, "open_uni failed");
            state.connections.unregister(&device_id);
            return;
        }
    };
    tokio::spawn(writer_task(msg_rx, FrameWriter::new(send)));

    push_grid_layout(&state, &device_id);

    loop {
        match conn.accept_uni().await {
            Ok(recv) => {
                tokio::spawn(handle_agent_stream(recv, device_id.clone(), state.clone()));
            }
            Err(e) => {
                tracing::info!(%device_id, error = %e, "connection closed");
                break;
            }
        }
    }

    state.connections.unregister(&device_id);
    state.registry.remove(&device_id);
    state.grid.write().remove_screen(&device_id);
    {
        let mut focused = state.focused_device.lock();
        if focused.as_deref() == Some(device_id.as_str()) {
            *focused = None;
            contixis_input::platform::set_focus_active(false);
        }
    }
    emit(&app, DeviceEvent::Disconnected { device_id });
    emit_grid_update(&app, &state);
}

async fn writer_task(mut rx: mpsc::Receiver<HostMsg>, mut w: FrameWriter) {
    use contixis_proto::*;
    while let Some(msg) = rx.recv().await {
        let r = match msg {
            HostMsg::MouseMove { norm_x, norm_y, screen_id } =>
                w.write_proto(MsgType::MouseMove, &MouseMove { norm_x, norm_y, target_screen_id: screen_id }).await,
            HostMsg::MouseButton { button, pressed, timestamp_us } =>
                w.write_proto(MsgType::MouseButton, &MouseButton { button, pressed, timestamp_us }).await,
            HostMsg::MouseScroll { delta_x, delta_y, timestamp_us } =>
                w.write_proto(MsgType::MouseScroll, &MouseScroll { delta_x, delta_y, timestamp_us }).await,
            HostMsg::KeyEvent { hid_usage, modifiers, pressed, timestamp_us } =>
                w.write_proto(MsgType::KeyEvent, &KeyEvent { hid_usage, modifiers, pressed, timestamp_us, character: None }).await,
            HostMsg::FocusTransfer { screen_id, entry_x, entry_y } =>
                w.write_proto(MsgType::FocusTransfer, &FocusTransfer { target_screen_id: screen_id, entry_norm_x: entry_x, entry_norm_y: entry_y }).await,
            HostMsg::FocusDrop =>
                w.write_proto(MsgType::FocusDrop, &FocusDrop {}).await,
            HostMsg::GridLayout { cols, rows, cells } =>
                w.write_proto(MsgType::GridLayout, &GridLayout {
                    cols, rows,
                    cells: cells.iter().map(|c| GridCell { col: c.col, row: c.row, device_id: c.device_id.clone(), screen_id: c.screen_id.clone() }).collect(),
                }).await,
            HostMsg::ClipboardData { seq, content_type, data } =>
                w.write_proto(MsgType::ClipboardData, &ProtoCbData { source_device_id: "host".into(), sequence: seq, content_type, data }).await,
            HostMsg::Disconnect => break,
        };
        if let Err(e) = r { tracing::warn!(error = %e, "writer error"); break; }
    }
}

async fn handle_agent_stream(recv: quinn::RecvStream, device_id: String, state: Arc<HostState>) {
    let mut reader = FrameReader::new(recv);
    loop {
        let frame = match reader.read_frame().await {
            Ok(f) => f,
            Err(_) => break,
        };
        match frame.msg_type {
            MsgType::ClipboardData => {
                if let Ok(cd) = ProtoCbData::decode(frame.payload.as_slice()) {
                    if state.clipboard.notify(&device_id, cd.content_type, cd.data.clone()).is_some() {
                        if let Ok(text) = String::from_utf8(cd.data) {
                            tokio::task::spawn_blocking(move || {
                                if let Ok(mut cb) = arboard::Clipboard::new() {
                                    let _ = cb.set_text(text);
                                }
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

pub fn push_grid_layout(state: &HostState, device_id: &str) {
    let grid = state.grid.read();
    let cells = grid_cells(&grid);
    let msg = HostMsg::GridLayout { rows: grid.rows as u32, cols: grid.cols as u32, cells };
    drop(grid);
    state.connections.send_sync(device_id, msg);
}

pub fn broadcast_grid_layout(state: &HostState) {
    let grid = state.grid.read();
    let cells = grid_cells(&grid);
    let msg = HostMsg::GridLayout { rows: grid.rows as u32, cols: grid.cols as u32, cells };
    drop(grid);
    state.connections.broadcast_sync(msg);
}

fn grid_cells(grid: &contixis_core::VirtualGrid) -> Vec<GridCellMsg> {
    grid.cells().iter()
        .filter_map(|c| c.screen.as_ref().map(|s| GridCellMsg {
            row: c.position.row as u32,
            col: c.position.col as u32,
            device_id: s.device_id.clone(),
            screen_id: s.device_id.clone(),
        }))
        .collect()
}
