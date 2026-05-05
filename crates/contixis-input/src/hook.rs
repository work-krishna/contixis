use anyhow::Result;
use tokio::sync::mpsc;
use crate::platform;

/// Raw input event captured by the platform hook.
#[derive(Debug, Clone)]
pub enum HookEvent {
    /// Relative mouse movement from hardware (not affected by cursor warps).
    MouseMove { dx: i32, dy: i32 },
    MouseButton { button: u8, pressed: bool },
    MouseScroll { dx: i32, dy: i32 },
    KeyEvent { keysym: u32, pressed: bool, modifiers: u32 },
}

/// Platform-specific low-level input hook.
pub struct InputHook {
    _private: (),
}

impl InputHook {
    /// Install the hook and return a channel that receives events.
    /// The hook runs on a dedicated OS thread; the channel is async-friendly.
    pub fn install() -> Result<(Self, mpsc::UnboundedReceiver<HookEvent>)> {
        let (tx, rx) = mpsc::unbounded_channel();
        platform::install_hook(tx)?;
        Ok((Self { _private: () }, rx))
    }
}

impl Drop for InputHook {
    fn drop(&mut self) {
        platform::uninstall_hook();
    }
}
