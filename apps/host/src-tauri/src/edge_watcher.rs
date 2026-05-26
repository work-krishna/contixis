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
    use std::ffi::CString;
    use std::ptr;
    use x11::xlib::*;

    // ── Ensure DISPLAY is usable ────────────────────────────────────────────
    // On Wayland sessions DISPLAY may be unset.  Try the env var first; if
    // it's missing or broken, probe /tmp/.X{n}-lock files (set by XWayland).
    // GNOME starts XWayland on-demand via socket activation, so connecting to
    // DISPLAY=:0 is enough to trigger its startup.
    let dpy = unsafe {
        // Trust the env var if already set.
        if std::env::var("DISPLAY").is_ok() {
            XOpenDisplay(ptr::null())
        } else {
            ptr::null_mut()
        }
    };
    let dpy = if dpy.is_null() {
        // Probe :0 … :9 until we get a working connection.
        let mut found = ptr::null_mut();
        for n in 0..=9 {
            let name = CString::new(format!(":{}", n)).unwrap();
            let d = unsafe { XOpenDisplay(name.as_ptr()) };
            if !d.is_null() {
                std::env::set_var("DISPLAY", format!(":{}", n));
                found = d;
                tracing::info!(display = n, "edge watcher: opened X11 display");
                break;
            }
        }
        if found.is_null() {
            tracing::warn!(
                "edge watcher: XOpenDisplay failed on :0-:9. \
                 Running on Wayland without XWayland? \
                 Edge detection will fall back to evdev cursor tracking \
                 (less accurate due to pointer acceleration)."
            );
        }
        found
    } else {
        tracing::info!("edge watcher: opened X11 display (existing DISPLAY)");
        dpy
    };

    let use_x11 = !dpy.is_null();

    // ── Screen dimensions ───────────────────────────────────────────────────
    let (scr_w, scr_h, root) = unsafe {
        if use_x11 {
            let scr  = XDefaultScreen(dpy);
            let w    = XDisplayWidth(dpy, scr);
            let h    = XDisplayHeight(dpy, scr);
            let root = XRootWindow(dpy, scr);
            *state.screen_dims.lock() = (w, h);
            tracing::info!(w, h, "edge watcher: screen dimensions from X11");
            (w, h, root)
        } else {
            let mons = state.host_monitors.read();
            let w = mons.iter().map(|m| m.x + m.width).max().unwrap_or(1920);
            let h = mons.iter().map(|m| m.y + m.height).max().unwrap_or(1080);
            *state.screen_dims.lock() = (w, h);
            tracing::info!(w, h, "edge watcher: screen dimensions from monitor list (X11 unavailable)");
            (w, h, 0u64)
        }
    };

    // Seed the evdev tracker with the real cursor position (used as fallback).
    if use_x11 {
        unsafe {
            let mut _r = 0u64; let mut _c = 0u64;
            let mut rx = 0i32; let mut ry = 0i32;
            let mut _wx = 0i32; let mut _wy = 0i32;
            let mut _mask = 0u32;
            XQueryPointer(dpy, root, &mut _r, &mut _c, &mut rx, &mut ry,
                          &mut _wx, &mut _wy, &mut _mask);
            contixis_input::platform::set_hook_cursor(rx, ry);
        }
    } else {
        // Seed to centre of first monitor so evdev tracking starts somewhere sensible.
        let mons = state.host_monitors.read();
        if let Some(m) = mons.first() {
            contixis_input::platform::set_hook_cursor(
                m.x + m.width  / 2,
                m.y + m.height / 2,
            );
        }
    }

    // Log the monitors that will be used for edge detection so we can verify
    // they match XQueryPointer's coordinate space.
    {
        let mons = state.host_monitors.read();
        for m in mons.iter() {
            tracing::info!(
                id = %m.device_id, name = %m.name,
                x = m.x, y = m.y, w = m.width, h = m.height,
                "edge watcher: host monitor loaded"
            );
        }
    }

    const THRESHOLD: i32 = 5;
    let mut parked_dims: Option<(i32, i32)> = None;
    let mut last_edge_log = std::time::Instant::now();

    loop {
        let focused = state.focused_device.lock().clone();

        if focused.is_some() {
            if let Some((mw, mh)) = parked_dims {
                *state.screen_dims.lock() = (mw, mh);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            continue;
        }

        parked_dims = None;
        *state.screen_dims.lock() = (scr_w, scr_h);

        // ── Read cursor position ────────────────────────────────────────────
        let (rx, ry) = if use_x11 {
            // XQueryPointer gives the real, acceleration-corrected position.
            unsafe {
                let mut root_ret = 0u64; let mut child_ret = 0u64;
                let mut rx = 0i32; let mut ry = 0i32;
                let mut wx = 0i32; let mut wy = 0i32;
                let mut mask = 0u32;
                XQueryPointer(dpy, root,
                    &mut root_ret, &mut child_ret,
                    &mut rx, &mut ry,
                    &mut wx, &mut wy,
                    &mut mask);
                (rx, ry)
            }
        } else {
            // Evdev fallback — inaccurate due to pointer acceleration but
            // better than nothing on pure-Wayland hosts without XWayland.
            let (x, y) = contixis_input::platform::get_hook_cursor();
            (x.clamp(0, scr_w - 1), y.clamp(0, scr_h - 1))
        };

        // ── Edge detection ──────────────────────────────────────────────────
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

                // Rate-limit edge log to once per second to avoid spam.
                let now = std::time::Instant::now();
                if now.duration_since(last_edge_log).as_secs() >= 1 {
                    last_edge_log = now;
                    tracing::info!(
                        cursor_x = rx, cursor_y = ry,
                        monitor  = %mon_device_id,
                        enx, eny,
                        "edge detected — checking layout"
                    );
                }

                let layout = state.layout.read();
                match layout.edge_transition(&mon_device_id, enx, eny) {
                    None => {
                        tracing::info!(
                            monitor = %mon_device_id, enx, eny,
                            layout_screens = layout.screens.iter()
                                .map(|s| format!("{}@({},{}){}x{}", s.device_id, s.x, s.y, s.width, s.height))
                                .collect::<Vec<_>>().join(", "),
                            "edge_transition: no adjacent screen found"
                        );
                    }
                    Some((screen, ex, ey)) => {
                    let target_id = screen.device_id.clone();
                    let screen_id = target_id.clone();
                    drop(layout);

                    if state.connections.is_connected(&target_id) {
                        *state.focused_device.lock() = Some(target_id.clone());
                        *state.virtual_cursor.lock() = (ex, ey);
                        *state.screen_dims.lock() = (mon_w, mon_h);
                        parked_dims = Some((mon_w, mon_h));

                        // Reset evdev tracker to centre so it stays in sync.
                        contixis_input::platform::set_hook_cursor(warp_x, warp_y);

                        // Warp cursor to monitor centre so the edge isn't re-triggered
                        // immediately when focus returns.  Works on X11 and most XWayland.
                        if use_x11 {
                            unsafe {
                                XWarpPointer(dpy, 0, root, 0, 0, 0, 0, warp_x, warp_y);
                                XFlush(dpy);
                            }
                        }

                        contixis_input::platform::set_focus_active(true);

                        state.connections.send_sync(&target_id, HostMsg::FocusTransfer {
                            screen_id, entry_x: ex, entry_y: ey,
                        });

                        tracing::info!(
                            device   = %target_id,
                            entry_x  = ex,
                            entry_y  = ey,
                            warp_x,
                            warp_y,
                            "focus transferred"
                        );
                    } else {
                        tracing::info!(
                            target = %target_id,
                            "edge_transition found screen but agent not connected"
                        );
                    }
                    } // end Some(screen) arm
                } // end match edge_transition
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
