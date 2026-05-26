use crate::state::HostMonitor;
use contixis_core::grid::{GridPosition, ScreenInfo};
use contixis_core::VirtualGrid;
use parking_lot::RwLockWriteGuard;

/// Detect host monitors via XRandR, place each one in row 1 of the VirtualGrid
/// (sorted left → right by X offset), and return their metadata.
pub fn detect_and_place(grid: &mut RwLockWriteGuard<VirtualGrid>) -> Vec<HostMonitor> {
    let mut raw = query_displays();
    raw.sort_by_key(|(x, _, _, _, _)| *x);

    raw.iter()
        .enumerate()
        .map(|(i, (x, y, w, h, name))| {
            let pos = GridPosition { row: 1, col: i as u8 };
            let placed = grid.place_screen(
                pos,
                ScreenInfo {
                    device_id: format!("__host__{}", i),
                    width_px: *w as u32,
                    height_px: *h as u32,
                    scale_factor: 1.0,
                },
            );
            tracing::info!(
                index = i, name = %name,
                x = x, y = y, w = w, h = h,
                row = pos.row, col = pos.col,
                placed = placed,
                "host monitor"
            );
            HostMonitor {
                x: *x, y: *y, width: *w, height: *h,
                grid_pos: pos,
                name: name.clone(),
                device_id: format!("__host__{}", i),
            }
        })
        .collect()
}

/// Ensure DISPLAY is set so XOpenDisplay works on Wayland sessions.
/// On GNOME, XWayland starts on-demand when XOpenDisplay connects to :0;
/// we don't need the display to be running yet — just the env var set.
#[cfg(target_os = "linux")]
fn ensure_display_env() {
    if std::env::var("DISPLAY").is_ok() {
        return;
    }
    // Check for lock files written by a running X server / XWayland.
    for n in 0..=9 {
        if std::path::Path::new(&format!("/tmp/.X{}-lock", n)).exists() {
            std::env::set_var("DISPLAY", format!(":{}", n));
            return;
        }
    }
    // Fall through to :0 — GNOME will start XWayland on the first connection.
    std::env::set_var("DISPLAY", ":0");
}

/// (x, y, width, height, name) for each active monitor in **logical pixels**.
///
/// On Wayland+XWayland with HiDPI scaling both `XRRGetMonitors` and
/// `xrandr --current` report *physical* pixel dimensions while
/// `XQueryPointer` returns *logical* pixel positions — causing edge
/// detection to never fire.  The authoritative source of logical geometry
/// is the GNOME Mutter D-Bus API (`GetCurrentState` → logical_monitors).
#[cfg(target_os = "linux")]
fn query_displays() -> Vec<(i32, i32, i32, i32, String)> {
    ensure_display_env();
    if let Some(d) = query_displays_mutter_dbus() { return d; }
    // Physical-pixel fallback — correct on non-HiDPI or bare X11.
    tracing::warn!("Mutter D-Bus unavailable; falling back to XRandR (may report physical pixels on HiDPI Wayland)");
    query_displays_xrandr_capi()
}

/// Query GNOME Mutter DisplayConfig via D-Bus using a Python3 one-liner.
/// Returns (x, y, logical_w, logical_h, connector_name) in logical pixels.
/// python3-gi (PyGObject) is installed by default on Ubuntu GNOME — no extra deps needed.
#[cfg(target_os = "linux")]
fn query_displays_mutter_dbus() -> Option<Vec<(i32, i32, i32, i32, String)>> {
    // Ensure the session bus address is findable even if launched outside a
    // full login session (e.g. from a terminal with a custom env).
    let bus_addr = std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap_or_else(|_| {
        let uid = std::process::Command::new("id")
            .arg("-u").output().ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        format!("unix:path=/run/user/{}/bus", uid)
    });

    let script = r#"
import sys, os
os.environ.setdefault('DBUS_SESSION_BUS_ADDRESS', sys.argv[1])
try:
    import gi
    gi.require_version('GLib', '2.0')
    from gi.repository import Gio, GLib
    proxy = Gio.DBusProxy.new_for_bus_sync(
        Gio.BusType.SESSION, Gio.DBusProxyFlags.NONE, None,
        'org.gnome.Mutter.DisplayConfig',
        '/org/gnome/Mutter/DisplayConfig',
        'org.gnome.Mutter.DisplayConfig', None)
    _serial, monitors, logical_monitors, _props = proxy.call_sync(
        'GetCurrentState', None, Gio.DBusCallFlags.NONE, -1, None).unpack()
    # Build map: connector -> current physical (w, h)
    phys = {}
    for specs, modes, _mp in monitors:
        conn = specs[0]
        for _mid, w, h, _freq, _sc, _scs, mprops in modes:
            if mprops.get('is-current'):
                phys[conn] = (int(w), int(h))
                break
    for lx, ly, scale, _tf, _prim, connected, _lp in logical_monitors:
        for conn, _v, _p, _s in connected:
            if conn in phys:
                pw, ph = phys[conn]
                lw = round(pw / float(scale))
                lh = round(ph / float(scale))
                print(conn, int(lx), int(ly), lw, lh)
except Exception as e:
    print('error:', e, file=sys.stderr)
    sys.exit(1)
"#;

    let out = std::process::Command::new("python3")
        .args(["-c", script, &bus_addr])
        .output()
        .ok()?;

    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        tracing::debug!(error = %err, "Mutter D-Bus query failed");
        return None;
    }

    let text = String::from_utf8(out.stdout).ok()?;
    let mut results = Vec::new();
    for line in text.lines() {
        let p: Vec<&str> = line.split_whitespace().collect();
        if p.len() < 5 { continue; }
        let name = p[0].to_string();
        let x: i32 = p[1].parse().ok()?;
        let y: i32 = p[2].parse().ok()?;
        let w: i32 = p[3].parse().ok()?;
        let h: i32 = p[4].parse().ok()?;
        if w > 0 && h > 0 { results.push((x, y, w, h, name)); }
    }

    if results.is_empty() { None } else {
        tracing::info!(count = results.len(), "monitor geometry from Mutter D-Bus (logical pixels)");
        Some(results)
    }
}

/// XRandR C API — returns *physical* pixels on HiDPI Wayland.
/// Used only when Mutter D-Bus is unavailable (non-GNOME, bare X11, etc.).
#[cfg(target_os = "linux")]
fn query_displays_xrandr_capi() -> Vec<(i32, i32, i32, i32, String)> {
    use std::ffi::CStr;
    use std::ptr;
    use x11::xlib::*;
    use x11::xrandr::*;

    unsafe {
        let dpy = XOpenDisplay(ptr::null());
        if dpy.is_null() {
            return vec![(0, 0, 1920, 1080, "Display".to_string())];
        }
        let root = XDefaultRootWindow(dpy);
        let scr  = XDefaultScreen(dpy);
        let mut n: i32 = 0;
        let monitors = XRRGetMonitors(dpy, root, 1, &mut n);

        let result = if monitors.is_null() || n <= 0 {
            vec![(0, 0, XDisplayWidth(dpy, scr), XDisplayHeight(dpy, scr), "Display".to_string())]
        } else {
            (0..n as usize)
                .map(|i| {
                    let m = &*monitors.add(i);
                    let name = {
                        let ptr = XGetAtomName(dpy, m.name);
                        if ptr.is_null() {
                            format!("Display {}", i + 1)
                        } else {
                            let s = CStr::from_ptr(ptr).to_string_lossy().to_string();
                            XFree(ptr as *mut _);
                            s
                        }
                    };
                    (m.x, m.y, m.width, m.height, name)
                })
                .collect()
        };

        if !monitors.is_null() {
            XRRFreeMonitors(monitors);
        }
        XCloseDisplay(dpy);
        result
    }
}

#[cfg(not(target_os = "linux"))]
fn query_displays() -> Vec<(i32, i32, i32, i32, String)> {
    vec![(0, 0, 1920, 1080, "Display".to_string())]
}
