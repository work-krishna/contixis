use crate::conn_registry::ConnRegistry;
use contixis_core::grid::GridPosition;
use contixis_core::{AppConfig, ClipboardManager, DeviceRegistry, SpatialLayout, VirtualGrid};
use contixis_crypto::{CertificateAuthority, HostStore, PairingManager};
use contixis_input::InputHook;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::{atomic::AtomicU64, Arc};
use tokio::sync::watch;

/// One physical monitor on the host machine.
#[derive(Debug, Clone)]
pub struct HostMonitor {
    /// Absolute X offset on the combined X11 desktop (px).
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    /// Cell in the VirtualGrid that represents this monitor.
    pub grid_pos: GridPosition,
    /// XRandR output name (e.g. "DP-1", "HDMI-1", "eDP-1").
    pub name: String,
    /// Stable device_id used in SpatialLayout (e.g. "__host__0").
    pub device_id: String,
}

pub struct HostState {
    pub config:         AppConfig,
    pub ca:             Arc<CertificateAuthority>,
    pub pairing:        Arc<PairingManager>,
    pub store:          Arc<Mutex<HostStore>>,
    pub registry:       DeviceRegistry,
    pub grid:           Arc<parking_lot::RwLock<VirtualGrid>>,
    pub clipboard:      ClipboardManager,
    pub server_tx:      Mutex<Option<watch::Sender<bool>>>,
    pub connections:    ConnRegistry,
    /// Currently focused agent device_id; None = focus is on host.
    pub focused_device: Arc<Mutex<Option<String>>>,
    /// Normalised [0,1] virtual cursor on the focused agent's screen.
    pub virtual_cursor: Arc<Mutex<(f32, f32)>>,
    /// Host screen dimensions (full desktop, px) queried from X11 at startup.
    pub screen_dims:    Arc<Mutex<(i32, i32)>>,
    /// Individual host monitors with their grid positions.
    pub host_monitors:  Arc<parking_lot::RwLock<Vec<HostMonitor>>>,
    /// Spatial layout: pixel-coordinate positions for all screens (host + agents).
    pub layout:         Arc<parking_lot::RwLock<SpatialLayout>>,
    /// Monotonic clipboard sequence counter.
    pub clipboard_seq:  Arc<AtomicU64>,
    /// Keeps the input hook alive for the app's lifetime.
    pub _hook:          Option<InputHook>,
}

impl HostState {
    pub fn new() -> Self {
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("contixis")
            .join("host");

        let config = AppConfig::load_or_default(&data_dir.join("config.json"));
        let ca = CertificateAuthority::load_or_generate(
            &data_dir.join("ca.crt.pem"),
            &data_dir.join("ca.key.pem"),
        )
        .expect("CA init");
        let store = HostStore::load(&data_dir.join("host_store.json"))
            .unwrap_or_default();
        let grid = VirtualGrid::new(config.grid_rows, config.grid_cols);

        Self {
            config,
            ca: Arc::new(ca),
            pairing: Arc::new(PairingManager::new()),
            store: Arc::new(Mutex::new(store)),
            registry: DeviceRegistry::new(),
            grid: Arc::new(parking_lot::RwLock::new(grid)),
            clipboard: ClipboardManager::new(),
            server_tx: Mutex::new(None),
            connections: ConnRegistry::new(),
            focused_device: Arc::new(Mutex::new(None)),
            virtual_cursor: Arc::new(Mutex::new((0.5, 0.5))),
            screen_dims: Arc::new(Mutex::new((1920, 1080))),
            host_monitors: Arc::new(parking_lot::RwLock::new(Vec::new())),
            clipboard_seq: Arc::new(AtomicU64::new(1)),
            layout: Arc::new(parking_lot::RwLock::new(SpatialLayout::new())),
            _hook: None,
        }
    }
}
