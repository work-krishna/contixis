use crate::conn_registry::HostMsg;
use crate::state::HostState;
use std::sync::Arc;

/// Spawn the edge-detection thread. Runs for the lifetime of the application.
pub fn start(state: Arc<HostState>) {
    #[cfg(target_os = "linux")]
    {
        let s = state.clone();
        std::thread::Builder::new()
            .name("contixis-edge".into())
            .spawn(move || linux_watch(s))
            .expect("edge watcher thread");
    }
}

#[cfg(target_os = "linux")]
fn linux_watch(state: Arc<HostState>) {
    use std::ptr;
    use x11::xlib::*;

    unsafe {
        let dpy = XOpenDisplay(ptr::null());
        if dpy.is_null() {
            tracing::warn!("edge watcher: XOpenDisplay failed");
            return;
        }

        let scr_num = XDefaultScreen(dpy);
        let scr_w   = XDisplayWidth(dpy, scr_num);
        let scr_h   = XDisplayHeight(dpy, scr_num);
        let root    = XRootWindow(dpy, scr_num);

        *state.screen_dims.lock() = (scr_w, scr_h);
        tracing::info!(scr_w, scr_h, "edge watcher: screen dimensions");

        const THRESHOLD: i32 = 1;

        // Position to park cursor at while agent has focus, plus the active monitor dims.
        // Using the monitor's own dimensions for mouse-delta scaling gives near-1:1 feel.
        let mut park: Option<(i32, i32, i32, i32)> = None; // (x, y, mon_w, mon_h)

        loop {
            let mut root_ret: Window = 0;
            let mut child_ret: Window = 0;
            let mut rx: i32 = 0; let mut ry: i32 = 0;
            let mut wx: i32 = 0; let mut wy: i32 = 0;
            let mut mask: u32 = 0;

            XQueryPointer(dpy, root,
                &mut root_ret, &mut child_ret,
                &mut rx, &mut ry,
                &mut wx, &mut wy,
                &mut mask);

            let focused = state.focused_device.lock().clone();

            if focused.is_some() {
                // EVIOCGRAB is active — X11 can't move the cursor, no warp needed.
                // Just keep screen_dims pointed at the monitor the cursor exited from so
                // mouse-delta scaling stays 1:1.
                if let Some((_, _, mw, mh)) = park {
                    *state.screen_dims.lock() = (mw, mh);
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            } else {
                // Focus is back on host — restore full display dims for edge detection.
                park = None;
                *state.screen_dims.lock() = (scr_w, scr_h);

                // Find the host monitor the cursor is currently on.
                let monitors = state.host_monitors.read();
                let current = monitors.iter().find(|m| {
                    rx >= m.x && rx < m.x + m.width &&
                    ry >= m.y && ry < m.y + m.height
                });

                if let Some(mon) = current {
                    let norm_x = (rx - mon.x) as f32 / mon.width  as f32;
                    let norm_y = (ry - mon.y) as f32 / mon.height as f32;

                    let edge = if rx <= mon.x + THRESHOLD {
                        Some((-0.01f32, norm_y))
                    } else if rx >= mon.x + mon.width - THRESHOLD - 1 {
                        Some((1.01f32, norm_y))
                    } else if ry <= mon.y + THRESHOLD {
                        Some((norm_x, -0.01f32))
                    } else if ry >= mon.y + mon.height - THRESHOLD - 1 {
                        Some((norm_x, 1.01f32))
                    } else {
                        None
                    };

                    if let Some((enx, eny)) = edge {
                        let mon_device_id = mon.device_id.clone();
                        let warp_x = mon.x + mon.width  / 2;
                        let warp_y = mon.y + mon.height / 2;
                        let mon_w  = mon.width;
                        let mon_h  = mon.height;
                        drop(monitors);

                        let layout = state.layout.read();
                        if let Some((screen, ex, ey)) =
                            layout.edge_transition(&mon_device_id, enx, eny)
                        {
                            let target_id = screen.device_id.clone();
                            let screen_id = target_id.clone();
                            drop(layout);

                            // Only transition to actual agent connections, not other host cells.
                            if state.connections.is_connected(&target_id) {
                                *state.focused_device.lock() = Some(target_id.clone());
                                *state.virtual_cursor.lock() = (ex, ey);
                                *state.screen_dims.lock() = (mon_w, mon_h);
                                park = Some((warp_x, warp_y, mon_w, mon_h));

                                // Warp to centre so cursor isn't sitting at the edge on return.
                                XWarpPointer(dpy, 0, root, 0, 0, 0, 0, warp_x, warp_y);
                                XFlush(dpy);

                                // Grab input devices so X11 stops seeing events (no cursor flicker).
                                contixis_input::platform::set_focus_active(true);

                                state.connections.send_sync(&target_id, HostMsg::FocusTransfer {
                                    screen_id, entry_x: ex, entry_y: ey,
                                });

                                tracing::info!(
                                    device = %target_id,
                                    entry_x = ex, entry_y = ey,
                                    "focus transferred"
                                );
                            }
                        }
                    }
                }
            }

            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}
