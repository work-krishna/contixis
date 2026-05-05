use anyhow::Result;
use crate::platform;

/// Synthesises input events into the local OS input queue.
pub struct InputInjector {
    _private: (),
}

impl InputInjector {
    pub fn new() -> Result<Self> {
        platform::init_injector()?;
        Ok(Self { _private: () })
    }

    /// Move the cursor to absolute pixel coordinates.
    pub fn mouse_move_abs(&self, x: i32, y: i32) -> Result<()> {
        platform::inject_mouse_move_abs(x, y)
    }

    /// Move the cursor by a relative delta (raw hardware counts).
    pub fn mouse_move_rel(&self, dx: i32, dy: i32) -> Result<()> {
        platform::inject_mouse_move_rel(dx, dy)
    }

    /// Synthesise a mouse button press or release.
    pub fn mouse_button(&self, button: u8, pressed: bool) -> Result<()> {
        platform::inject_mouse_button(button, pressed)
    }

    /// Synthesise a scroll event (in HID units).
    pub fn mouse_scroll(&self, dx: i32, dy: i32) -> Result<()> {
        platform::inject_mouse_scroll(dx, dy)
    }

    /// Synthesise a key press or release via HID usage code.
    pub fn key_event(&self, hid_usage: u32, pressed: bool, modifiers: u32) -> Result<()> {
        platform::inject_key_event(hid_usage, pressed, modifiers)
    }
}

impl Default for InputInjector {
    fn default() -> Self { Self::new().expect("InputInjector::new") }
}
