pub mod grid;
pub mod spatial;
pub mod session;
pub mod input;
pub mod clipboard;
pub mod config;
pub mod device;
pub mod pool;

pub use grid::{VirtualGrid, GridCell, ScreenInfo, GridPosition};
pub use spatial::{SpatialLayout, ScreenPlacement};
pub use session::{SessionFsm, SessionState, SessionEvent};
pub use input::InputRouter;
pub use clipboard::ClipboardManager;
pub use config::AppConfig;
pub use device::DeviceRegistry;
pub use pool::BufferPool;
