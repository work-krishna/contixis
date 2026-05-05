mod commands;
mod conn_registry;
mod edge_watcher;
mod monitors;
mod server;
mod state;

use conn_registry::HostMsg;
use contixis_input::{HookEvent, InputHook};
use std::sync::{atomic::Ordering, Arc};
use tauri::Manager;
use tracing_subscriber::EnvFilter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("rustls crypto provider");

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("contixis=info".parse().unwrap()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let mut host_state = state::HostState::new();

            // ── Input hook: capture keyboard/mouse and forward to focused agent ─
            match InputHook::install() {
                Ok((hook, mut hook_rx)) => {
                    host_state._hook = Some(hook);
                    let conns  = host_state.connections.clone();
                    let focus  = host_state.focused_device.clone();
                    let cursor = host_state.virtual_cursor.clone();
                    let dims   = host_state.screen_dims.clone();

                    tauri::async_runtime::spawn(async move {
                        while let Some(event) = hook_rx.recv().await {
                            let device_id = focus.lock().clone();
                            let Some(did) = device_id else { continue };

                            let now_us = || {
                                std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_micros() as u64
                            };

                            match event {
                                HookEvent::MouseMove { dx, dy } => {
                                    // Send raw hardware deltas; agent applies relative to its cursor.
                                    tracing::trace!(dx, dy, device = %did, "→ agent mouse_move");
                                    conns.send_sync(&did, HostMsg::MouseMove {
                                        norm_x: dx as f32,
                                        norm_y: dy as f32,
                                        screen_id: did.clone(),
                                    });
                                }
                                HookEvent::MouseButton { button, pressed } => {
                                    conns.send_sync(&did, HostMsg::MouseButton {
                                        button: button as u32, pressed, timestamp_us: now_us(),
                                    });
                                }
                                HookEvent::MouseScroll { dx, dy } => {
                                    conns.send_sync(&did, HostMsg::MouseScroll {
                                        delta_x: dx as f32, delta_y: dy as f32, timestamp_us: now_us(),
                                    });
                                }
                                HookEvent::KeyEvent { hid_usage, pressed, modifiers } => {
                                    tracing::info!(hid = %format!("0x{:02X}", hid_usage), pressed, device = %did, "hook key");
                                    // Escape → return focus to host.
                                    if hid_usage == contixis_proto::hid::ESCAPE && pressed {
                                        tracing::info!(device = %did, "ESC → releasing focus");
                                        contixis_input::platform::set_focus_active(false);
                                        conns.send_sync(&did, HostMsg::FocusDrop);
                                        *focus.lock() = None;
                                        continue;
                                    }
                                    conns.send_sync(&did, HostMsg::KeyEvent {
                                        hid_usage, modifiers, pressed, timestamp_us: now_us(),
                                    });
                                }
                            }
                        }
                    });
                }
                Err(e) => {
                    tracing::warn!(error = %e, "input hook install failed");
                }
            }

            // ── Detect host monitors and place them in the grid ──────────────
            {
                let mut grid = host_state.grid.write();
                let mons = monitors::detect_and_place(&mut grid);
                tracing::info!(count = mons.len(), "host monitors detected");
                *host_state.host_monitors.write() = mons;
            }

            let host_state = Arc::new(host_state);

            // ── Edge watcher: cursor edge detection ──────────────────────────
            edge_watcher::start(host_state.clone());

            // ── Clipboard monitor: propagate host clipboard changes to agents ─
            {
                let conns = host_state.connections.clone();
                let seq   = host_state.clipboard_seq.clone();
                std::thread::Builder::new()
                    .name("contixis-clipboard".into())
                    .spawn(move || {
                        let mut cb = match arboard::Clipboard::new() {
                            Ok(c) => c,
                            Err(e) => { tracing::warn!(error=%e, "clipboard init"); return; }
                        };
                        let mut last = String::new();
                        loop {
                            if let Ok(text) = cb.get_text() {
                                if text != last && !text.is_empty() {
                                    last = text.clone();
                                    let s = seq.fetch_add(1, Ordering::Relaxed);
                                    conns.broadcast_sync(HostMsg::ClipboardData {
                                        seq: s,
                                        content_type: "text/plain".into(),
                                        data: text.into_bytes(),
                                    });
                                }
                            }
                            std::thread::sleep(std::time::Duration::from_millis(500));
                        }
                    })
                    .expect("clipboard thread");
            }

            app.handle().manage(host_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_server,
            commands::stop_server,
            commands::get_host_info,
            commands::update_grid_layout,
            commands::disconnect_device,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
