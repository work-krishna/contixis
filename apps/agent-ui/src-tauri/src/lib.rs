mod commands;
mod state;

use crate::state::*;
use anyhow::Result;
use contixis_crypto::{AgentStore, DeviceIdentity};
use contixis_input::InputInjector;
use contixis_net::{
    make_agent_tls_config_insecure, make_agent_transport, AgentSession, DiscoveryEvent,
    FrameReader, FrameWriter, MdnsDiscovery,
};
use contixis_proto::{ClipboardData as ProtoCbData, FocusRelease as ProtoFocusRelease, MsgType};
use dashmap::DashMap;
use parking_lot::Mutex;
use prost::Message as ProstMessage;
use quinn::Endpoint;
use serde::Serialize;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, oneshot, watch};

// ── Events emitted to the frontend ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum AgentEvent {
    HostDiscovered {
        #[serde(rename = "hostId")] host_id: String,
        addr: String,
        instance: String,
    },
    HostLost {
        #[serde(rename = "hostId")] host_id: String,
    },
    StatusChange {
        status: String,
        #[serde(rename = "hostId", skip_serializing_if = "Option::is_none")]
        host_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        addr: Option<String>,
    },
}

pub(crate) fn emit_event(app: &AppHandle, event: AgentEvent) {
    if let Err(e) = app.emit("agent-event", &event) {
        tracing::warn!(error = %e, "emit failed");
    }
}

// ── Tauri app entry ──────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("rustls crypto provider");

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("contixis=info".parse().unwrap()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let data_dir = dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("contixis")
                .join("agent");
            std::fs::create_dir_all(&data_dir).ok();

            let identity = DeviceIdentity::load_or_generate(&data_dir.join("identity"))
                .expect("device identity");
            let store_path = data_dir.join("agent_store.json");
            let store = AgentStore::load(&store_path).unwrap_or_default();

            let state = Arc::new(AgentState {
                identity:      Arc::new(Mutex::new(identity)),
                store:         Arc::new(Mutex::new(store)),
                store_path,
                discovered:    Arc::new(DashMap::new()),
                conn_state:    Arc::new(Mutex::new(ConnState::Idle)),
                pin_tx:        Arc::new(Mutex::new(None)),
                disconnect_tx: Arc::new(Mutex::new(None)),
            });

            // Background mDNS browse loop.
            let app_handle = app.handle().clone();
            let state_ref  = state.clone();
            tauri::async_runtime::spawn(async move {
                mdns_browse_loop(app_handle, state_ref).await;
            });

            app.handle().manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_agent_info,
            commands::get_paired_hosts,
            commands::get_discovered_hosts,
            commands::connect_to_host,
            commands::disconnect,
            commands::enter_pin,
            commands::forget_host,
        ])
        .run(tauri::generate_context!())
        .expect("Tauri error");
}

// ── mDNS browse loop ─────────────────────────────────────────────────────────

async fn mdns_browse_loop(app: AppHandle, state: Arc<AgentState>) {
    loop {
        let mdns = match MdnsDiscovery::new() {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(error = %e, "mDNS init failed, retrying in 5s");
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                continue;
            }
        };

        let mut rx = match mdns.browse_events() {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "mDNS browse failed, retrying in 5s");
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                continue;
            }
        };

        tracing::info!("mDNS: browsing for contixis hosts…");

        while let Some(event) = rx.recv().await {
            match event {
                DiscoveryEvent::Found(host) => {
                    state.discovered.insert(host.host_id.clone(), DiscoveredEntry {
                        host_id:  host.host_id.clone(),
                        addr:     host.addr.to_string(),
                        instance: host.instance_name.clone(),
                    });
                    emit_event(&app, AgentEvent::HostDiscovered {
                        host_id:  host.host_id,
                        addr:     host.addr.to_string(),
                        instance: host.instance_name,
                    });
                }
                DiscoveryEvent::Lost { instance } => {
                    let removed = state.discovered.iter()
                        .find(|e| e.instance.starts_with(&instance))
                        .map(|e| e.host_id.clone());
                    if let Some(host_id) = removed {
                        state.discovered.remove(&host_id);
                        emit_event(&app, AgentEvent::HostLost { host_id });
                    }
                }
            }
        }

        // Channel closed — daemon probably stopped; restart after a short delay.
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    }
}

// ── Connection task ───────────────────────────────────────────────────────────

pub(crate) async fn run_connection(addr_str: String, app: AppHandle, state: Arc<AgentState>) {
    *state.conn_state.lock() = ConnState::Connecting { addr: addr_str.clone() };
    emit_event(&app, AgentEvent::StatusChange {
        status:  "connecting".into(),
        host_id: None,
        addr:    Some(addr_str.clone()),
    });

    match do_connect(addr_str.clone(), app.clone(), state.clone()).await {
        Ok(()) => {}
        Err(e) => tracing::warn!(error = %e, "connection ended"),
    }

    *state.conn_state.lock() = ConnState::Idle;
    *state.pin_tx.lock()        = None;
    *state.disconnect_tx.lock() = None;
    emit_event(&app, AgentEvent::StatusChange { status: "idle".into(), host_id: None, addr: None });
}

async fn do_connect(addr_str: String, app: AppHandle, state: Arc<AgentState>) -> Result<()> {
    let addr: SocketAddr = addr_str.parse()?;

    let cert_der = state.identity.lock().connection_cert_der()?;
    let key_pem  = state.identity.lock().private_key_pem();
    let mut client_cfg = make_agent_tls_config_insecure(cert_der, &key_pem)?;
    client_cfg.transport_config(make_agent_transport());

    let mut endpoint = Endpoint::client("0.0.0.0:0".parse()?)?;
    endpoint.set_default_client_config(client_cfg);

    let conn = endpoint.connect(addr, "contixis-host")?.await?;
    tracing::info!(%addr, "QUIC connected");

    ensure_display_env();
    let (sw, sh) = screen_dimensions();
    let screens = vec![contixis_proto::ScreenInfo {
        id: "screen0".into(),
        width_px: sw as u32,
        height_px: sh as u32,
        dpi_scale: 1.0,
        is_primary: true,
        offset_x: 0,
        offset_y: 0,
    }];

    let mut session = AgentSession::new(conn.clone(), state.identity.clone(), state.store.clone());

    // Build a PIN provider that signals the frontend and waits for the user's response.
    let app_ref   = app.clone();
    let state_ref = state.clone();
    let addr_ref  = addr_str.clone();
    let pin_provider = move || {
        let (tx, rx) = oneshot::channel::<String>();
        *state_ref.pin_tx.lock()     = Some(tx);
        *state_ref.conn_state.lock() = ConnState::PairingRequired { addr: addr_ref.clone() };
        tracing::info!("pairing required — emitting event and waiting for PIN");
        emit_event(&app_ref, AgentEvent::StatusChange {
            status:  "pairing".into(),
            host_id: None,
            addr:    None,
        });
        async move { rx.await.unwrap_or_default() }
    };

    let device_id = session.run_handshake(screens, pin_provider).await?;

    // Persist updated store (last_addr, new cert after pairing, etc.).
    {
        let mut store = state.store.lock();
        store.update_last_addr(&device_id, addr);
        let _ = store.save(&state.store_path);
    }

    let host_id = device_id.clone();
    *state.conn_state.lock() = ConnState::Connected { addr: addr_str.clone(), host_id: host_id.clone() };
    emit_event(&app, AgentEvent::StatusChange {
        status:  "connected".into(),
        host_id: Some(host_id),
        addr:    Some(addr_str),
    });

    tracing::info!("session established");

    let injector = InputInjector::new()?;
    let (sw, sh) = screen_dimensions();

    let (to_host_tx, to_host_rx) = mpsc::unbounded_channel::<AgentMsg>();
    let host_clipboard: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));

    let (disconnect_tx, disconnect_rx) = watch::channel(true);
    *state.disconnect_tx.lock() = Some(disconnect_tx);

    let send = conn.open_uni().await?;
    tokio::spawn(host_writer(FrameWriter::new(send), to_host_rx));
    tokio::spawn(clipboard_poller(to_host_tx.clone(), host_clipboard.clone()));

    let recv = conn.accept_uni().await?;
    event_loop(FrameReader::new(recv), injector, sw, sh, to_host_tx, host_clipboard, disconnect_rx).await;

    Ok(())
}

// ── Agent session event loop ──────────────────────────────────────────────────

enum AgentMsg {
    FocusRelease { exit_norm_x: f32, exit_norm_y: f32 },
    Clipboard    { seq: u64, data: Vec<u8> },
}

async fn event_loop(
    mut reader: FrameReader,
    injector: InputInjector,
    screen_w: i32,
    screen_h: i32,
    to_host: mpsc::UnboundedSender<AgentMsg>,
    host_clipboard: Arc<Mutex<String>>,
    mut disconnect_rx: watch::Receiver<bool>,
) {
    let mut focused       = false;
    let mut cursor_x: i32 = screen_w / 2;
    let mut cursor_y: i32 = screen_h / 2;
    let mut pending_release = false;

    loop {
        let frame = tokio::select! {
            f = reader.read_frame() => match f {
                Ok(f)  => f,
                Err(e) => { tracing::info!(error = %e, "host stream ended"); break; }
            },
            _ = disconnect_rx.changed() => {
                if !*disconnect_rx.borrow() { break; }
                continue;
            }
        };

        match frame.msg_type {
            MsgType::FocusTransfer => {
                if let Ok(ft) = contixis_proto::FocusTransfer::decode(frame.payload.as_slice()) {
                    focused = true;
                    pending_release = false;
                    cursor_x = (ft.entry_norm_x * screen_w as f32) as i32;
                    cursor_y = (ft.entry_norm_y * screen_h as f32) as i32;
                    let _ = injector.mouse_move_abs(cursor_x, cursor_y);
                    tracing::info!(entry_x = cursor_x, entry_y = cursor_y, "focus received");
                }
            }
            MsgType::FocusDrop => {
                focused = false;
                pending_release = false;
                tracing::info!("focus dropped");
            }
            MsgType::MouseMove => {
                if focused {
                    if let Ok(mm) = contixis_proto::MouseMove::decode(frame.payload.as_slice()) {
                        let dx = mm.norm_x as i32;
                        let dy = mm.norm_y as i32;
                        let prev_x = cursor_x;
                        let prev_y = cursor_y;
                        cursor_x += dx;
                        cursor_y += dy;

                        let oob = cursor_x < 0 || cursor_x >= screen_w
                               || cursor_y < 0 || cursor_y >= screen_h;
                        if oob {
                            if !pending_release {
                                pending_release = true;
                                let edge_x = cursor_x.clamp(0, screen_w - 1);
                                let edge_y = cursor_y.clamp(0, screen_h - 1);
                                let pdx = edge_x - prev_x;
                                let pdy = edge_y - prev_y;
                                if pdx != 0 || pdy != 0 { let _ = injector.mouse_move_rel(pdx, pdy); }
                                let exit_norm_x = cursor_x as f32 / screen_w as f32;
                                let exit_norm_y = cursor_y as f32 / screen_h as f32;
                                let _ = to_host.send(AgentMsg::FocusRelease { exit_norm_x, exit_norm_y });
                            }
                            cursor_x = cursor_x.clamp(0, screen_w - 1);
                            cursor_y = cursor_y.clamp(0, screen_h - 1);
                        } else {
                            pending_release = false;
                            let _ = injector.mouse_move_rel(dx, dy);
                        }
                    }
                }
            }
            MsgType::MouseButton => {
                if focused {
                    if let Ok(mb) = contixis_proto::MouseButton::decode(frame.payload.as_slice()) {
                        let _ = injector.mouse_button(mb.button as u8, mb.pressed);
                    }
                }
            }
            MsgType::MouseScroll => {
                if focused {
                    if let Ok(ms) = contixis_proto::MouseScroll::decode(frame.payload.as_slice()) {
                        let _ = injector.mouse_scroll(ms.delta_x as i32, ms.delta_y as i32);
                    }
                }
            }
            MsgType::KeyEvent => {
                if focused {
                    if let Ok(ke) = contixis_proto::KeyEvent::decode(frame.payload.as_slice()) {
                        let _ = injector.key_event(ke.hid_usage, ke.pressed, ke.modifiers);
                    }
                }
            }
            MsgType::ClipboardData => {
                if let Ok(cd) = ProtoCbData::decode(frame.payload.as_slice()) {
                    if cd.content_type == "text/plain" {
                        if let Ok(text) = String::from_utf8(cd.data) {
                            *host_clipboard.lock() = text.clone();
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

async fn host_writer(
    mut writer: FrameWriter,
    mut rx: mpsc::UnboundedReceiver<AgentMsg>,
) {
    while let Some(msg) = rx.recv().await {
        let result = match msg {
            AgentMsg::FocusRelease { exit_norm_x, exit_norm_y } =>
                writer.write_proto(MsgType::FocusRelease, &ProtoFocusRelease { exit_norm_x, exit_norm_y }).await,
            AgentMsg::Clipboard { seq, data } =>
                writer.write_proto(MsgType::ClipboardData, &ProtoCbData {
                    source_device_id: "agent".into(),
                    sequence:         seq,
                    content_type:     "text/plain".into(),
                    data,
                }).await,
        };
        if let Err(e) = result {
            tracing::warn!(error = %e, "host writer error");
            break;
        }
    }
}

async fn clipboard_poller(
    to_host: mpsc::UnboundedSender<AgentMsg>,
    host_clipboard: Arc<Mutex<String>>,
) {
    let mut last = String::new();
    let mut seq: u64 = 0;
    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        let text = tokio::task::spawn_blocking(|| {
            arboard::Clipboard::new().ok().and_then(|mut cb| cb.get_text().ok())
        })
        .await
        .ok()
        .flatten();

        let Some(text) = text else { continue };
        if text == last || text.is_empty() { continue; }
        if text == *host_clipboard.lock() { continue; }

        last = text.clone();
        seq += 1;
        let _ = to_host.send(AgentMsg::Clipboard { seq, data: text.into_bytes() });
    }
}

// ── Display / screen helpers ─────────────────────────────────────────────────

fn ensure_display_env() {
    if std::env::var("XAUTHORITY").is_err() {
        if let Ok(home) = std::env::var("HOME") {
            let p = format!("{}/.Xauthority", home);
            if std::path::Path::new(&p).exists() {
                std::env::set_var("XAUTHORITY", p);
            }
        }
    }
    if std::env::var("DISPLAY").is_err() {
        for n in 0..=9 {
            if std::path::Path::new(&format!("/tmp/.X{}-lock", n)).exists() {
                std::env::set_var("DISPLAY", format!(":{}", n));
                break;
            }
        }
        if std::env::var("DISPLAY").is_err() {
            std::env::set_var("DISPLAY", ":0");
        }
    }
}

fn screen_dimensions() -> (i32, i32) {
    #[cfg(target_os = "linux")]
    {
        use std::ptr;
        use x11::xlib::*;
        unsafe {
            let dpy = XOpenDisplay(ptr::null());
            if !dpy.is_null() {
                let scr = XDefaultScreen(dpy);
                let w   = XDisplayWidth(dpy, scr);
                let h   = XDisplayHeight(dpy, scr);
                XCloseDisplay(dpy);
                return (w, h);
            }
        }
    }
    (1920, 1080)
}
