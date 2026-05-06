use anyhow::{anyhow, Result};
use crate::hook::HookEvent;
use tokio::sync::mpsc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

static HOOK_TX: OnceLock<mpsc::UnboundedSender<HookEvent>> = OnceLock::new();
static HOOK_RUNNING: AtomicBool = AtomicBool::new(false);
// Set true while an agent holds focus; the evdev loop uses this to EVIOCGRAB/ungrab.
static AGENT_FOCUSED: AtomicBool = AtomicBool::new(false);

/// Called by the host when an agent gains or loses input focus.
/// Triggers EVIOCGRAB (exclusive kernel grab) so X11 stops seeing mouse/keyboard
/// events while the agent has focus — eliminating cursor flicker on the host.
pub fn set_focus_active(active: bool) {
    AGENT_FOCUSED.store(active, Ordering::SeqCst);
}

// Tracked cursor position used by inject_mouse_move_rel to avoid XQueryPointer.
static CURSOR_X: AtomicI32 = AtomicI32::new(960);
static CURSOR_Y: AtomicI32 = AtomicI32::new(540);

// ── Hook (evdev capture) ─────────────────────────────────────────────────────

pub fn install_hook(tx: mpsc::UnboundedSender<HookEvent>) -> Result<()> {
    HOOK_TX.set(tx).map_err(|_| anyhow!("hook already installed"))?;
    HOOK_RUNNING.store(true, Ordering::SeqCst);
    std::thread::Builder::new()
        .name("contixis-input-hook".into())
        .spawn(hook_thread)?;
    Ok(())
}

pub fn uninstall_hook() {
    HOOK_RUNNING.store(false, Ordering::SeqCst);
}

fn hook_thread() {
    set_realtime_priority();
    // Open X11 display for keysym lookup (layout-aware, like Barrier).
    unsafe {
        if std::env::var("DISPLAY").is_err() {
            std::env::set_var("DISPLAY", ":0");
        }
        let dpy = x11::xlib::XOpenDisplay(std::ptr::null());
        if !dpy.is_null() {
            KEYSYM_DPY.get_or_init(|| XDpy(dpy));
        } else {
            tracing::warn!("hook: XOpenDisplay failed — keysym lookup unavailable");
        }
    }
    let kbd_paths   = find_keyboards();
    let mouse_paths = find_mice();
    if kbd_paths.is_empty() || mouse_paths.is_empty() {
        tracing::warn!(keyboards = ?kbd_paths, mice = ?mouse_paths, "evdev: could not find all devices");
        return;
    }
    tracing::info!(keyboards = ?kbd_paths, mice = ?mouse_paths, "evdev devices found");
    if let Err(e) = run_evdev_loop(&kbd_paths, &mouse_paths) {
        tracing::warn!(error = %e, "evdev loop exited");
    }
}

/// Return ALL real keyboard devices (EV_KEY, no EV_REL, supports KEY_ESC).
/// Opening every qualifying keyboard ensures we capture the user's actual keyboard
/// regardless of whether it's PS/2, USB, or Bluetooth.
fn find_keyboards() -> Vec<String> {
    let content = match std::fs::read_to_string("/proc/bus/input/devices") {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let mut results     = vec![];
    let mut handlers    = String::new();
    let mut ev_bits     = String::new();
    let mut key_low_u64 = 0u64;
    for line in content.lines() {
        if line.starts_with("H: Handlers=") {
            handlers = line["H: Handlers=".len()..].to_string();
        } else if line.starts_with("B: EV=") {
            ev_bits = line["B: EV=".len()..].to_string();
        } else if line.starts_with("B: KEY=") {
            // Kernel prints bitmap words from highest keycode to lowest; the last word
            // covers keycodes 0–63.  Bit 1 = KEY_ESC — every real keyboard has it.
            let key_str = line["B: KEY=".len()..].trim();
            if let Some(last) = key_str.split_whitespace().last() {
                key_low_u64 = u64::from_str_radix(last, 16).unwrap_or(0);
            }
        } else if line.is_empty() {
            let ev = u64::from_str_radix(ev_bits.trim(), 16).unwrap_or(0);
            let is_keyboard = ev & (1 << 1) != 0   // EV_KEY
                && ev & (1 << 2) == 0               // no EV_REL (excludes mice)
                && key_low_u64 & 2 != 0;            // KEY_ESC present
            if is_keyboard {
                if let Some(n) = handlers.split_whitespace().find(|h| h.starts_with("event")) {
                    results.push(format!("/dev/input/{}", n));
                }
            }
            handlers.clear();
            ev_bits.clear();
            key_low_u64 = 0;
        }
    }
    results
}

/// Return ALL pointing devices (anything with EV_REL) so every physical mouse
/// and touchpad is grabbed when the agent has focus.
fn find_mice() -> Vec<String> {
    let content = match std::fs::read_to_string("/proc/bus/input/devices") {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let mut results  = vec![];
    let mut handlers = String::new();
    let mut ev_bits  = String::new();
    for line in content.lines() {
        if line.starts_with("H: Handlers=") {
            handlers = line["H: Handlers=".len()..].to_string();
        } else if line.starts_with("B: EV=") {
            ev_bits = line["B: EV=".len()..].to_string();
        } else if line.is_empty() {
            let ev = u64::from_str_radix(ev_bits.trim(), 16).unwrap_or(0);
            if ev & (1 << 2) != 0 {  // EV_REL
                if let Some(n) = handlers.split_whitespace().find(|h| h.starts_with("event")) {
                    results.push(format!("/dev/input/{}", n));
                }
            }
            handlers.clear();
            ev_bits.clear();
        }
    }
    results
}

/// Raw Linux `input_event` (24 bytes on 64-bit).
#[repr(C)]
#[derive(Copy, Clone)]
struct InputEvent {
    tv_sec:  i64,
    tv_usec: i64,
    kind:    u16,
    code:    u16,
    value:   i32,
}

const EV_KEY: u16 = 1;
const EV_REL: u16 = 2;
const REL_X:      u16 = 0;
const REL_Y:      u16 = 1;
const REL_HWHEEL: u16 = 6;
const REL_WHEEL:  u16 = 8;
const BTN_LEFT:   u16 = 0x110;
const BTN_RIGHT:  u16 = 0x111;
const BTN_MIDDLE: u16 = 0x112;

fn run_evdev_loop(kbd_paths: &[String], mouse_paths: &[String]) -> Result<()> {
    use std::os::unix::io::AsRawFd;

    let kbd_files: Vec<std::fs::File> = kbd_paths.iter()
        .filter_map(|p| std::fs::File::open(p)
            .map_err(|e| { tracing::warn!(path = %p, error = %e, "evdev: cannot open keyboard"); e })
            .ok())
        .collect();
    if kbd_files.is_empty() {
        return Err(anyhow!("could not open any keyboard device"));
    }

    // Open all mice; silently skip ones that fail (e.g. permission denied on VMware virtual devices).
    let mouse_files: Vec<std::fs::File> = mouse_paths.iter()
        .filter_map(|p| std::fs::File::open(p)
            .map_err(|e| { tracing::warn!(path = %p, error = %e, "evdev: cannot open mouse"); e })
            .ok())
        .collect();
    if mouse_files.is_empty() {
        return Err(anyhow!("could not open any mouse device"));
    }

    let kbd_fds:   Vec<i32> = kbd_files.iter().map(|f| f.as_raw_fd()).collect();
    let mouse_fds: Vec<i32> = mouse_files.iter().map(|f| f.as_raw_fd()).collect();

    unsafe {
        for &fd in kbd_fds.iter().chain(mouse_fds.iter()) {
            libc::fcntl(fd, libc::F_SETFL, libc::O_NONBLOCK);
        }
    }

    let epoll_fd = unsafe { libc::epoll_create1(0) };
    if epoll_fd < 0 {
        return Err(anyhow!("epoll_create1: {}", std::io::Error::last_os_error()));
    }

    for &fd in kbd_fds.iter().chain(mouse_fds.iter()) {
        let mut ev = libc::epoll_event { events: libc::EPOLLIN as u32, u64: fd as u64 };
        unsafe { libc::epoll_ctl(epoll_fd, libc::EPOLL_CTL_ADD, fd, &mut ev) };
    }

    // EVIOCGRAB = _IOW('E', 0x90, int) = 0x40044590
    const EVIOCGRAB: libc::c_ulong = 0x40044590;
    let all_fds: Vec<i32> = kbd_fds.iter().chain(mouse_fds.iter()).copied().collect();

    const MAX_EV: usize = 16;
    let mut epoll_events = [libc::epoll_event { events: 0, u64: 0 }; MAX_EV];
    const SZ: usize = std::mem::size_of::<InputEvent>();
    let mut buf = [0u8; SZ * 32];
    let mut modifiers: u32 = 0;
    let mut is_grabbed = false;

    while HOOK_RUNNING.load(Ordering::Relaxed) {
        // Sync kernel grab state with agent focus.
        let want_grab = AGENT_FOCUSED.load(Ordering::Relaxed);
        if want_grab != is_grabbed {
            let val: libc::c_int = if want_grab { 1 } else { 0 };
            for &fd in &all_fds {
                unsafe { libc::ioctl(fd, EVIOCGRAB, val); }
            }
            is_grabbed = want_grab;
            tracing::info!(grabbed = want_grab, "evdev: input grab state changed");
        }

        let n = unsafe {
            libc::epoll_wait(epoll_fd, epoll_events.as_mut_ptr(), MAX_EV as i32, 20)
        };
        if n < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted { continue; }
            break;
        }
        for i in 0..n as usize {
            let fd = epoll_events[i].u64 as i32;
            let read = unsafe {
                libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len())
            };
            if read <= 0 { continue; }
            let count = read as usize / SZ;
            let is_kbd = kbd_fds.contains(&fd);
            for j in 0..count {
                let ev: InputEvent = unsafe {
                    std::ptr::read(buf[j * SZ..].as_ptr() as *const InputEvent)
                };
                dispatch(ev, is_kbd, &mut modifiers);
            }
        }
    }

    // Safety net: release grab on thread exit.
    if is_grabbed {
        for &fd in &all_fds {
            unsafe { libc::ioctl(fd, EVIOCGRAB, 0i32); }
        }
    }

    unsafe { libc::close(epoll_fd) };
    Ok(())
}

fn dispatch(ev: InputEvent, is_kbd: bool, modifiers: &mut u32) {
    let Some(tx) = HOOK_TX.get() else { return };
    match ev.kind {
        EV_KEY => {
            if ev.value == 2 { return; } // ignore repeat
            let pressed = ev.value == 1;
            if is_kbd {
                update_mods(ev.code, pressed, modifiers);
                // Barrier approach: translate via X11 keysym so it's layout-aware.
                // evdev code + 8 = X11 keycode.
                let keysym = if let Some(XDpy(dpy)) = KEYSYM_DPY.get() {
                    let x11_code = (ev.code + 8) as u8;
                    unsafe { x11::xlib::XKeycodeToKeysym(*dpy, x11_code, 0) as u32 }
                } else {
                    0
                };
                // 0 = no keysym, 0xFFFFFF = XK_VoidSymbol (also invalid)
                if keysym != 0 && keysym != 0xFFFFFF {
                    let _ = tx.send(HookEvent::KeyEvent {
                        keysym,
                        pressed,
                        modifiers: *modifiers,
                    });
                }
            } else {
                let button = match ev.code {
                    BTN_LEFT   => 1,
                    BTN_MIDDLE => 2,
                    BTN_RIGHT  => 3,
                    _ => return,
                };
                let _ = tx.send(HookEvent::MouseButton { button, pressed });
            }
        }
        EV_REL if !is_kbd => match ev.code {
            REL_X      => { let _ = tx.send(HookEvent::MouseMove { dx: ev.value, dy: 0 }); }
            REL_Y      => { let _ = tx.send(HookEvent::MouseMove { dx: 0, dy: ev.value }); }
            REL_WHEEL  => { let _ = tx.send(HookEvent::MouseScroll { dx: 0, dy: ev.value }); }
            REL_HWHEEL => { let _ = tx.send(HookEvent::MouseScroll { dx: ev.value, dy: 0 }); }
            _ => {}
        },
        _ => {}
    }
}

fn update_mods(code: u16, pressed: bool, mods: &mut u32) {
    use contixis_proto::modifiers::*;
    let bit: u32 = match code {
        42 | 54  => SHIFT,
        29 | 97  => CTRL,
        56 | 100 => ALT,
        125| 126 => META,
        _ => return,
    };
    if pressed { *mods |= bit; } else { *mods &= !bit; }
}

// ── Injector ─────────────────────────────────────────────────────────────────
// Strategy: try uinput first (works on both X11 and Wayland, requires the
// user to be in the 'uinput' group), fall back to XTest (X11 / XWayland only).

struct XDpy(*mut x11::xlib::Display);
unsafe impl Send for XDpy {}
unsafe impl Sync for XDpy {}

// Host-side display for XKeycodeToKeysym (capture path).
static KEYSYM_DPY: OnceLock<XDpy>       = OnceLock::new();
// Agent-side display for XKeysymToKeycode + XTest (injection path).
static INJ_DPY:    OnceLock<XDpy>       = OnceLock::new();
static UINPUT_FD:  OnceLock<Option<i32>> = OnceLock::new();

const EV_SYN:    u16 = 0;
const SYN_REPORT: u16 = 0;

fn uinput_fd() -> Option<i32> {
    UINPUT_FD.get()?.as_ref().copied()
}

fn uinput_write(fd: i32, type_: u16, code: u16, value: i32) {
    let ev = InputEvent { tv_sec: 0, tv_usec: 0, kind: type_, code, value };
    unsafe {
        libc::write(fd, &ev as *const _ as *const libc::c_void,
                    std::mem::size_of::<InputEvent>());
    }
}

fn try_open_uinput() -> Option<i32> {
    const UI_SET_EVBIT:  libc::c_ulong = 0x40045564;
    const UI_SET_KEYBIT: libc::c_ulong = 0x40045565;
    const UI_SET_RELBIT: libc::c_ulong = 0x40045566;
    const UI_DEV_CREATE: libc::c_ulong = 0x5501;

    let path = std::ffi::CString::new("/dev/uinput").ok()?;
    let fd = unsafe { libc::open(path.as_ptr(), libc::O_WRONLY | libc::O_NONBLOCK) };
    if fd < 0 { return None; }

    unsafe {
        libc::ioctl(fd, UI_SET_EVBIT, EV_SYN as libc::c_int);
        libc::ioctl(fd, UI_SET_EVBIT, EV_REL as libc::c_int);
        libc::ioctl(fd, UI_SET_EVBIT, EV_KEY as libc::c_int);

        libc::ioctl(fd, UI_SET_RELBIT, REL_X      as libc::c_int);
        libc::ioctl(fd, UI_SET_RELBIT, REL_Y      as libc::c_int);
        libc::ioctl(fd, UI_SET_RELBIT, REL_WHEEL  as libc::c_int);
        libc::ioctl(fd, UI_SET_RELBIT, REL_HWHEEL as libc::c_int);

        // Mouse buttons
        libc::ioctl(fd, UI_SET_KEYBIT, 0x110i32); // BTN_LEFT
        libc::ioctl(fd, UI_SET_KEYBIT, 0x111i32); // BTN_RIGHT
        libc::ioctl(fd, UI_SET_KEYBIT, 0x112i32); // BTN_MIDDLE

        // Keyboard keys — linux keycodes 1-127 cover all standard keys
        for k in 1i32..=127 { libc::ioctl(fd, UI_SET_KEYBIT, k); }
    }

    #[repr(C)]
    struct UinputUserDev {
        name:           [u8; 80],
        bustype:        u16,
        vendor:         u16,
        product:        u16,
        version:        u16,
        ff_effects_max: u32,
        absmax:         [i32; 64],
        absmin:         [i32; 64],
        absfuzz:        [i32; 64],
        absflat:        [i32; 64],
    }

    let mut dev = UinputUserDev {
        name: [0u8; 80],
        bustype: 0x03, vendor: 0x0001, product: 0x0001, version: 0x0001,
        ff_effects_max: 0,
        absmax: [0; 64], absmin: [0; 64], absfuzz: [0; 64], absflat: [0; 64],
    };
    let label = b"Contixis Virtual Input";
    dev.name[..label.len()].copy_from_slice(label);

    let w = unsafe {
        libc::write(fd, &dev as *const _ as *const libc::c_void,
                    std::mem::size_of::<UinputUserDev>())
    };
    if w < 0 { unsafe { libc::close(fd) }; return None; }

    let ret = unsafe { libc::ioctl(fd, UI_DEV_CREATE, 0i32) };
    if ret < 0 { unsafe { libc::close(fd) }; return None; }

    Some(fd)
}

pub fn init_injector() -> Result<()> {
    // ── uinput (preferred — Wayland-native) ──────────────────────────────────
    let ufd = try_open_uinput();
    if ufd.is_some() {
        tracing::info!("uinput virtual device ready — Wayland-native injection active");
    } else {
        tracing::warn!(
            "uinput unavailable (not in 'uinput' group?). \
             Fix: sudo usermod -a -G uinput $USER  then log out and back in. \
             Falling back to XTest (may not work on Wayland)."
        );
    }
    UINPUT_FD.get_or_init(|| ufd);

    // ── XTest / XWarpPointer fallback ────────────────────────────────────────
    if std::env::var("XAUTHORITY").is_err() {
        let candidates = [
            std::env::var("HOME").ok().map(|h| format!("{}/.Xauthority", h)),
            Some("/run/user/1000/gdm/Xauthority".into()),
        ];
        for path in candidates.into_iter().flatten() {
            if std::path::Path::new(&path).exists() {
                std::env::set_var("XAUTHORITY", &path);
                break;
            }
        }
    }
    if std::env::var("DISPLAY").is_err() {
        std::env::set_var("DISPLAY", ":0");
    }
    unsafe {
        let dpy = x11::xlib::XOpenDisplay(std::ptr::null());
        let dpy = if dpy.is_null() {
            [":0", ":1", ":0.0"].iter().find_map(|name| {
                let c = std::ffi::CString::new(*name).unwrap();
                let d = x11::xlib::XOpenDisplay(c.as_ptr());
                if d.is_null() { None } else { Some(d) }
            })
        } else {
            Some(dpy)
        };
        if let Some(d) = dpy {
            INJ_DPY.get_or_init(|| XDpy(d));
            if uinput_fd().is_none() {
                tracing::info!("XTest injector ready (fallback)");
            }
        } else if uinput_fd().is_none() {
            tracing::warn!("XOpenDisplay failed — all injection disabled");
        }
    }
    Ok(())
}

pub fn inject_mouse_move_rel(dx: i32, dy: i32) -> Result<()> {
    if let Some(fd) = uinput_fd() {
        uinput_write(fd, EV_REL, REL_X, dx);
        uinput_write(fd, EV_REL, REL_Y, dy);
        uinput_write(fd, EV_SYN, SYN_REPORT, 0);
        return Ok(());
    }
    let Some(XDpy(dpy)) = INJ_DPY.get() else { return Ok(()); };
    unsafe {
        let scr   = x11::xlib::XDefaultScreen(*dpy);
        let scr_w = x11::xlib::XDisplayWidth(*dpy, scr);
        let scr_h = x11::xlib::XDisplayHeight(*dpy, scr);
        let new_x = (CURSOR_X.load(Ordering::Relaxed) + dx).clamp(0, scr_w - 1);
        let new_y = (CURSOR_Y.load(Ordering::Relaxed) + dy).clamp(0, scr_h - 1);
        CURSOR_X.store(new_x, Ordering::Relaxed);
        CURSOR_Y.store(new_y, Ordering::Relaxed);
        x11::xtest::XTestFakeMotionEvent(*dpy, -1, new_x, new_y, x11::xlib::CurrentTime);
        x11::xlib::XFlush(*dpy);
    }
    Ok(())
}

pub fn inject_mouse_move_abs(x: i32, y: i32) -> Result<()> {
    // uinput has no absolute positioning; just update tracking for fallback path.
    // On Wayland the cursor starts wherever it was — it moves correctly once
    // relative events arrive.
    CURSOR_X.store(x, Ordering::Relaxed);
    CURSOR_Y.store(y, Ordering::Relaxed);
    if uinput_fd().is_some() { return Ok(()); }
    let Some(XDpy(dpy)) = INJ_DPY.get() else { return Ok(()); };
    unsafe {
        let scr   = x11::xlib::XDefaultScreen(*dpy);
        let scr_w = x11::xlib::XDisplayWidth(*dpy, scr);
        let scr_h = x11::xlib::XDisplayHeight(*dpy, scr);
        let cx = x.clamp(0, scr_w - 1);
        let cy = y.clamp(0, scr_h - 1);
        CURSOR_X.store(cx, Ordering::Relaxed);
        CURSOR_Y.store(cy, Ordering::Relaxed);
        x11::xtest::XTestFakeMotionEvent(*dpy, -1, cx, cy, x11::xlib::CurrentTime);
        x11::xlib::XFlush(*dpy);
    }
    Ok(())
}

pub fn inject_mouse_button(button: u8, pressed: bool) -> Result<()> {
    if let Some(fd) = uinput_fd() {
        let code: u16 = match button { 1 => 0x110, 2 => 0x112, 3 => 0x111, _ => return Ok(()) };
        uinput_write(fd, EV_KEY, code, pressed as i32);
        uinput_write(fd, EV_SYN, SYN_REPORT, 0);
        return Ok(());
    }
    let Some(XDpy(dpy)) = INJ_DPY.get() else { return Ok(()); };
    unsafe {
        x11::xtest::XTestFakeButtonEvent(*dpy, button as u32, pressed as i32, x11::xlib::CurrentTime);
        x11::xlib::XFlush(*dpy);
    }
    Ok(())
}

pub fn inject_mouse_scroll(dx: i32, dy: i32) -> Result<()> {
    if let Some(fd) = uinput_fd() {
        if dy != 0 { uinput_write(fd, EV_REL, REL_WHEEL,  dy); }
        if dx != 0 { uinput_write(fd, EV_REL, REL_HWHEEL, dx); }
        uinput_write(fd, EV_SYN, SYN_REPORT, 0);
        return Ok(());
    }
    let Some(XDpy(dpy)) = INJ_DPY.get() else { return Ok(()); };
    unsafe {
        let fire = |btn: u32| {
            x11::xtest::XTestFakeButtonEvent(*dpy, btn, 1, x11::xlib::CurrentTime);
            x11::xtest::XTestFakeButtonEvent(*dpy, btn, 0, x11::xlib::CurrentTime);
        };
        for _ in 0..dy.unsigned_abs() { fire(if dy > 0 { 5 } else { 4 }); }
        for _ in 0..dx.unsigned_abs() { fire(if dx > 0 { 7 } else { 6 }); }
        x11::xlib::XFlush(*dpy);
    }
    Ok(())
}

pub fn inject_key_event(keysym: u32, pressed: bool, _modifiers: u32) -> Result<()> {
    let Some(XDpy(dpy)) = INJ_DPY.get() else { return Ok(()); };
    // Resolve keysym → X11 keycode on the agent's display (layout-aware).
    let x11_code = unsafe {
        x11::xlib::XKeysymToKeycode(*dpy, keysym as x11::xlib::KeySym)
    };
    if x11_code == 0 { return Ok(()); }

    if let Some(fd) = uinput_fd() {
        // uinput uses evdev keycodes = X11 keycode − 8
        let evdev = (x11_code as u32).saturating_sub(8);
        if evdev > 0 {
            uinput_write(fd, EV_KEY, evdev as u16, pressed as i32);
            uinput_write(fd, EV_SYN, SYN_REPORT, 0);
            return Ok(());
        }
    }
    // XTest fallback: use X11 keycode directly
    unsafe {
        x11::xtest::XTestFakeKeyEvent(*dpy, x11_code as u32, pressed as i32, x11::xlib::CurrentTime);
        x11::xlib::XFlush(*dpy);
    }
    Ok(())
}


fn set_realtime_priority() {
    unsafe {
        let param = libc::sched_param { sched_priority: 10 };
        libc::sched_setscheduler(0, libc::SCHED_FIFO, &param);
    }
}
