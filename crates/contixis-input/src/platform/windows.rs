use anyhow::{anyhow, Result};
use crate::hook::HookEvent;
use tokio::sync::mpsc;
use std::sync::OnceLock;

static HOOK_TX: OnceLock<mpsc::UnboundedSender<HookEvent>> = OnceLock::new();

pub fn install_hook(tx: mpsc::UnboundedSender<HookEvent>) -> Result<()> {
    HOOK_TX.set(tx).map_err(|_| anyhow!("hook already installed"))?;
    // TODO: SetWindowsHookEx(WH_MOUSE_LL) + SetWindowsHookEx(WH_KEYBOARD_LL)
    tracing::warn!("Windows low-level hook is a stub");
    Ok(())
}

pub fn uninstall_hook() {
    // TODO: UnhookWindowsHookEx
}

pub fn init_injector() -> Result<()> {
    Ok(())
}

pub fn inject_mouse_move_rel(_dx: i32, _dy: i32) -> Result<()> { Ok(()) }
pub fn inject_mouse_move_abs(x: i32, y: i32) -> Result<()> {
    tracing::trace!("inject mouse_move_abs({}, {})", x, y);
    // TODO: SendInput with MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_MOVE
    Ok(())
}

pub fn inject_mouse_button(button: u8, pressed: bool) -> Result<()> {
    tracing::trace!("inject mouse_button({}, {})", button, pressed);
    // TODO: SendInput MOUSEINPUT
    Ok(())
}

pub fn inject_mouse_scroll(dx: i32, dy: i32) -> Result<()> {
    tracing::trace!("inject mouse_scroll({}, {})", dx, dy);
    Ok(())
}

pub fn inject_key_event(hid_usage: u32, pressed: bool, _modifiers: u32) -> Result<()> {
    tracing::trace!("inject key_event(0x{:02X}, {})", hid_usage, pressed);
    // TODO: SendInput KEYBDINPUT with KEYEVENTF_SCANCODE
    Ok(())
}
