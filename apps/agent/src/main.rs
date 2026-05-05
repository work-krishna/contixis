use anyhow::Result;
use clap::{Parser, Subcommand};
use contixis_crypto::DeviceIdentity;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "contixis-agent", version, about = "Contixis agent daemon")]
struct Cli {
    #[arg(long)]
    data_dir: Option<PathBuf>,
    /// Skip mDNS and connect directly (e.g. 192.168.1.10:7443)
    #[arg(long)]
    host: Option<String>,
    /// Provide pairing PIN non-interactively (skips zenity/kdialog prompts)
    #[arg(long)]
    pin: Option<String>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Run,
    DeviceId,
    Csr,
    /// Install a udev rule so /dev/uinput is always accessible without sudo.
    /// Run once after installation: contixis-agent setup
    Setup,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("contixis=info".parse()?),
        )
        .init();

    // Must be called before any TLS/QUIC usage.
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    let cli = Cli::parse();
    let data_dir = cli.data_dir.unwrap_or_else(|| {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("contixis")
            .join("agent")
    });

    let identity = DeviceIdentity::load_or_generate(&data_dir.join("identity"))?;

    match cli.command.unwrap_or(Command::Run) {
        Command::DeviceId => println!("{}", identity.device_id),
        Command::Csr => println!("{}", identity.csr_pem()?),
        Command::Run => daemon::run(identity, data_dir, cli.host, cli.pin).await?,
        Command::Setup => setup::run()?,
    }
    Ok(())
}

mod setup {
    use anyhow::{bail, Result};

    const RULE: &str = r#"KERNEL=="uinput", MODE="0660", GROUP="input", TAG+="uaccess"
"#;
    const RULE_PATH: &str = "/etc/udev/rules.d/99-contixis-uinput.rules";

    pub fn run() -> Result<()> {
        // Check if we can write the rule directly (running as root).
        let as_root = unsafe { libc::geteuid() } == 0;

        if as_root {
            install_rule()?;
        } else {
            // Try pkexec first (graphical sudo), fall back to sudo.
            let written = try_write_via("pkexec") || try_write_via("sudo");
            if !written {
                bail!(
                    "Could not install udev rule. Run manually:\n\
                     echo '{RULE}' | sudo tee {RULE_PATH}\n\
                     sudo udevadm control --reload-rules\n\
                     sudo udevadm trigger /dev/uinput"
                );
            }
        }

        println!("udev rule installed: {RULE_PATH}");
        println!("Reloading udev rules…");
        reload_udev();
        println!("Done. uinput is now accessible without sudo.");
        println!("You may need to log out and back in for group changes to take effect.");
        Ok(())
    }

    fn install_rule() -> Result<()> {
        std::fs::write(RULE_PATH, RULE)
            .map_err(|e| anyhow::anyhow!("write {RULE_PATH}: {e}"))
    }

    fn try_write_via(tool: &str) -> bool {
        // Use tee to write the file via a privileged helper.
        let status = std::process::Command::new(tool)
            .args(["tee", RULE_PATH])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                if let Some(stdin) = child.stdin.as_mut() {
                    let _ = stdin.write_all(RULE.as_bytes());
                }
                child.wait()
            });
        matches!(status, Ok(s) if s.success())
    }

    fn reload_udev() {
        for args in [
            &["udevadm", "control", "--reload-rules"][..],
            &["udevadm", "trigger", "/dev/uinput"][..],
        ] {
            let _ = std::process::Command::new(args[0]).args(&args[1..]).status();
        }
    }
}

mod daemon {
    use super::*;
    use contixis_crypto::{AgentStore, DeviceIdentity};
    use contixis_input::InputInjector;
    use contixis_net::{
        make_agent_tls_config_insecure, make_agent_transport, AgentSession, FrameReader, FrameWriter, MdnsDiscovery,
    };
    use contixis_proto::{ClipboardData as ProtoCbData, MsgType};
    use parking_lot::Mutex;
    use prost::Message as ProstMessage;
    use quinn::Endpoint;
    use std::net::SocketAddr;
    use std::sync::Arc;

    pub async fn run(identity: DeviceIdentity, data_dir: PathBuf, direct_host: Option<String>, static_pin: Option<String>) -> Result<()> {
        tracing::info!(device_id = %identity.device_id, "Contixis agent starting");

        let store_path = data_dir.join("agent_store.json");
        let store = AgentStore::load(&store_path)?;
        let store = Arc::new(Mutex::new(store));
        let identity = Arc::new(Mutex::new(identity));

        let pin = Arc::new(static_pin);

        // --host flag: skip mDNS and connect directly, retrying on disconnect.
        if let Some(host_str) = direct_host {
            let addr: SocketAddr = host_str.parse()
                .map_err(|e| anyhow::anyhow!("invalid --host address '{}': {}", host_str, e))?;
            tracing::info!(%addr, "direct connect mode (mDNS skipped)");
            loop {
                match connect_and_run(addr, identity.clone(), store.clone(), pin.clone()).await {
                    Ok(()) => tracing::info!(%addr, "session ended, reconnecting in 3s"),
                    Err(e) => tracing::warn!(%addr, error = %e, "connection failed, retrying in 3s"),
                }
                let _ = store.lock().save(&store_path);
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }
        }

        // Try stored host addresses first.
        let known_addrs: Vec<SocketAddr> = store.lock()
            .all_hosts()
            .filter_map(|h| h.last_addr)
            .collect();

        for addr in known_addrs {
            tracing::info!(%addr, "trying stored host");
            match connect_and_run(addr, identity.clone(), store.clone(), pin.clone()).await {
                Ok(()) => tracing::info!(%addr, "session ended"),
                Err(e) => tracing::warn!(%addr, error = %e, "connection failed"),
            }
            let _ = store.lock().save(&store_path);
        }

        // mDNS discovery loop.
        loop {
            let mdns = match MdnsDiscovery::new() {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!(error = %e, "mDNS init failed, retrying in 5s");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
            };

            let mut rx = match mdns.browse() {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(error = %e, "mDNS browse failed, retrying in 5s");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
            };

            tracing::info!("browsing for contixis hosts via mDNS...");

            while let Some(host) = rx.recv().await {
                tracing::info!(host_id = %host.host_id, addr = %host.addr, "discovered host via mDNS");
                match connect_and_run(host.addr, identity.clone(), store.clone(), pin.clone()).await {
                    Ok(()) => {}
                    Err(e) => tracing::warn!(error = %e, "connection failed"),
                }
                let _ = store.lock().save(&store_path);
            }
        }
    }

    async fn connect_and_run(
        addr: SocketAddr,
        identity: Arc<Mutex<DeviceIdentity>>,
        store: Arc<Mutex<AgentStore>>,
        static_pin: Arc<Option<String>>,
    ) -> Result<()> {
        let cert_der = identity.lock().connection_cert_der()?;
        let key_pem = identity.lock().private_key_pem();
        let client_cfg = make_agent_tls_config_insecure(cert_der, &key_pem)?;

        let mut client_cfg = client_cfg;
        client_cfg.transport_config(make_agent_transport());

        let mut endpoint = Endpoint::client("0.0.0.0:0".parse()?)?;
        endpoint.set_default_client_config(client_cfg);

        let conn = endpoint.connect(addr, "contixis-host")?.await?;
        tracing::info!(%addr, "QUIC connected");

        // Detect screens before handshake so the host can place agent cells on the grid.
        // Also sets DISPLAY/XAUTHORITY env vars for later injector init.
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
        tracing::info!(width = sw, height = sh, "agent screen detected");

        let mut session = AgentSession::new(conn.clone(), identity, store);
        let pin_ref = static_pin.clone();
        let _device_id = session
            .run_handshake(screens, move || {
                if let Some(pin) = pin_ref.as_deref() {
                    tracing::info!("using --pin flag for pairing");
                    return pin.to_string();
                }
                // Notify the desktop that a PIN is needed, then open a GUI dialog.
                // block_in_place is required because this runs inside an async context.
                tokio::task::block_in_place(|| {
                    notify_pairing("Contixis", "Pairing requested — enter the PIN shown on the host.");
                    prompt_pin()
                })
            })
            .await?;

        tracing::info!("session established");

        let injector = InputInjector::new()?;
        let (sw, sh) = screen_dimensions();

        // Open agent→host uni stream for clipboard updates.
        let send = conn.open_uni().await?;
        tokio::spawn(clipboard_sender(FrameWriter::new(send)));

        // Accept host→agent uni stream for input/focus/clipboard events.
        let recv = conn.accept_uni().await?;
        event_loop(FrameReader::new(recv), injector, sw, sh).await;

        Ok(())
    }

    /// Show a desktop notification (fire-and-forget; silently ignored if unavailable).
    fn notify_pairing(summary: &str, body: &str) {
        let _ = std::process::Command::new("notify-send")
            .args([summary, body, "--app-name=Contixis", "--urgency=critical"])
            .spawn();
    }

    /// Show a GUI PIN entry dialog. Tries zenity, then kdialog, then falls back to stdin.
    fn prompt_pin() -> String {
        // zenity (GNOME / most desktops)
        if let Ok(out) = std::process::Command::new("zenity")
            .args([
                "--entry",
                "--title=Contixis Pairing",
                "--text=Enter the PIN shown on the host PC:",
            ])
            .output()
        {
            if out.status.success() {
                return String::from_utf8_lossy(&out.stdout).trim().to_string();
            }
        }

        // kdialog (KDE)
        if let Ok(out) = std::process::Command::new("kdialog")
            .args(["--title", "Contixis Pairing", "--inputbox", "Enter the PIN shown on the host PC:"])
            .output()
        {
            if out.status.success() {
                return String::from_utf8_lossy(&out.stdout).trim().to_string();
            }
        }

        // Terminal fallback
        eprint!("Enter pairing PIN (shown on host): ");
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
        line.trim().to_string()
    }

    async fn event_loop(
        mut reader: FrameReader,
        injector: InputInjector,
        screen_w: i32,
        screen_h: i32,
    ) {
        let mut focused = false;

        loop {
            let frame = match reader.read_frame().await {
                Ok(f) => f,
                Err(e) => {
                    tracing::info!(error = %e, "host stream ended");
                    break;
                }
            };

            match frame.msg_type {
                MsgType::FocusTransfer => {
                    if let Ok(ft) =
                        contixis_proto::FocusTransfer::decode(frame.payload.as_slice())
                    {
                        focused = true;
                        let x = (ft.entry_norm_x * screen_w as f32) as i32;
                        let y = (ft.entry_norm_y * screen_h as f32) as i32;
                        let _ = injector.mouse_move_abs(x, y);
                        tracing::info!(entry_x = x, entry_y = y, "focus received");
                    }
                }
                MsgType::FocusDrop => {
                    focused = false;
                    tracing::info!("focus dropped");
                }
                MsgType::MouseMove => {
                    if focused {
                        if let Ok(mm) =
                            contixis_proto::MouseMove::decode(frame.payload.as_slice())
                        {
                            // norm_x/norm_y carry raw hardware deltas (not normalised).
                            tracing::trace!(dx = mm.norm_x, dy = mm.norm_y, "inject mouse_move_rel");
                            let _ = injector.mouse_move_rel(mm.norm_x as i32, mm.norm_y as i32);
                        }
                    }
                }
                MsgType::MouseButton => {
                    if focused {
                        if let Ok(mb) =
                            contixis_proto::MouseButton::decode(frame.payload.as_slice())
                        {
                            let _ = injector.mouse_button(mb.button as u8, mb.pressed);
                        }
                    }
                }
                MsgType::MouseScroll => {
                    if focused {
                        if let Ok(ms) =
                            contixis_proto::MouseScroll::decode(frame.payload.as_slice())
                        {
                            let _ = injector.mouse_scroll(ms.delta_x as i32, ms.delta_y as i32);
                        }
                    }
                }
                MsgType::KeyEvent => {
                    if focused {
                        if let Ok(ke) =
                            contixis_proto::KeyEvent::decode(frame.payload.as_slice())
                        {
                            let _ = injector.key_event(ke.hid_usage, ke.pressed, ke.modifiers);
                        }
                    }
                }
                MsgType::ClipboardData => {
                    if let Ok(cd) = ProtoCbData::decode(frame.payload.as_slice()) {
                        if cd.content_type == "text/plain" {
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
                MsgType::GridLayout => {}
                _ => {}
            }
        }
    }

    async fn clipboard_sender(mut writer: FrameWriter) {
        let mut last = String::new();
        let mut seq: u64 = 0;
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            let text = tokio::task::spawn_blocking(|| {
                arboard::Clipboard::new().ok().and_then(|mut cb| cb.get_text().ok())
            })
            .await
            .ok()
            .flatten();

            let Some(text) = text else { continue };
            if text == last || text.is_empty() {
                continue;
            }

            last = text.clone();
            seq += 1;
            let msg = ProtoCbData {
                source_device_id: "agent".into(),
                sequence: seq,
                content_type: "text/plain".into(),
                data: text.into_bytes(),
            };
            if let Err(e) = writer.write_proto(MsgType::ClipboardData, &msg).await {
                tracing::warn!(error = %e, "clipboard send error");
                break;
            }
        }
    }

    /// Set DISPLAY and XAUTHORITY env vars so X11 calls work when started outside a desktop session.
    fn ensure_display_env() {
        // ── XAUTHORITY ────────────────────────────────────────────────────────
        if std::env::var("XAUTHORITY").is_err() {
            // 1. Standard X.org location
            if let Ok(home) = std::env::var("HOME") {
                let p = format!("{}/.Xauthority", home);
                if std::path::Path::new(&p).exists() {
                    std::env::set_var("XAUTHORITY", p);
                }
            }
        }
        if std::env::var("XAUTHORITY").is_err() {
            // 2. GNOME/mutter XWayland auth: /run/user/<uid>/.mutter-Xwaylandauth.*
            let uid = unsafe { libc::getuid() };
            let run_user = format!("/run/user/{}", uid);
            if let Ok(entries) = std::fs::read_dir(&run_user) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let n = name.to_string_lossy();
                    if n.starts_with(".mutter-Xwaylandauth") || n.starts_with(".Xauthority") {
                        tracing::debug!(path = %entry.path().display(), "using XWayland auth file");
                        std::env::set_var("XAUTHORITY", entry.path());
                        break;
                    }
                }
            }
        }

        // ── DISPLAY ───────────────────────────────────────────────────────────
        if std::env::var("DISPLAY").is_err() {
            // Scan /tmp/.X<N>-lock to find the first active X display
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

        tracing::debug!(
            display = std::env::var("DISPLAY").as_deref().unwrap_or("(unset)"),
            xauthority = std::env::var("XAUTHORITY").as_deref().unwrap_or("(unset)"),
            "display env"
        );
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
                    let w = XDisplayWidth(dpy, scr);
                    let h = XDisplayHeight(dpy, scr);
                    XCloseDisplay(dpy);
                    return (w, h);
                }
            }
        }
        (1920, 1080)
    }
}
