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
            HostMonitor { x: *x, y: *y, width: *w, height: *h, grid_pos: pos, name: name.clone() }
        })
        .collect()
}

/// (x, y, width, height, name) for each active monitor.
#[cfg(target_os = "linux")]
fn query_displays() -> Vec<(i32, i32, i32, i32, String)> {
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
