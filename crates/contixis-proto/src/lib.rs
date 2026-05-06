// Re-export all generated protobuf types
include!(concat!(env!("OUT_DIR"), "/contixis.rs"));

/// Message type byte used in wire framing
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgType {
    Handshake       = 0x01,
    PairingRequired = 0x02,
    PairingRequest  = 0x03,
    PairingSuccess  = 0x04,
    PairingFailed   = 0x05,
    SessionReady    = 0x06,
    SessionTerminate= 0x07,
    Heartbeat       = 0x10,
    HeartbeatAck    = 0x11,
    GridLayout      = 0x20,
    ScreenInfoUpdate= 0x21,
    FocusTransfer   = 0x30,
    FocusDrop       = 0x31,
    FocusRelease    = 0x36,
    MouseMove       = 0x32,
    MouseButton     = 0x33,
    MouseScroll     = 0x34,
    KeyEvent        = 0x35,
    ClipboardNotify = 0x40,
    ClipboardRequest= 0x41,
    ClipboardData   = 0x42,
    AgentStatus     = 0x50,
    TimeSyncPing    = 0x60,
    TimeSyncPong    = 0x61,
}

impl TryFrom<u8> for MsgType {
    type Error = u8;
    fn try_from(v: u8) -> Result<Self, u8> {
        match v {
            0x01 => Ok(Self::Handshake),
            0x02 => Ok(Self::PairingRequired),
            0x03 => Ok(Self::PairingRequest),
            0x04 => Ok(Self::PairingSuccess),
            0x05 => Ok(Self::PairingFailed),
            0x06 => Ok(Self::SessionReady),
            0x07 => Ok(Self::SessionTerminate),
            0x10 => Ok(Self::Heartbeat),
            0x11 => Ok(Self::HeartbeatAck),
            0x20 => Ok(Self::GridLayout),
            0x21 => Ok(Self::ScreenInfoUpdate),
            0x30 => Ok(Self::FocusTransfer),
            0x31 => Ok(Self::FocusDrop),
            0x36 => Ok(Self::FocusRelease),
            0x32 => Ok(Self::MouseMove),
            0x33 => Ok(Self::MouseButton),
            0x34 => Ok(Self::MouseScroll),
            0x35 => Ok(Self::KeyEvent),
            0x40 => Ok(Self::ClipboardNotify),
            0x41 => Ok(Self::ClipboardRequest),
            0x42 => Ok(Self::ClipboardData),
            0x50 => Ok(Self::AgentStatus),
            0x60 => Ok(Self::TimeSyncPing),
            0x61 => Ok(Self::TimeSyncPong),
            other => Err(other),
        }
    }
}

/// Keyboard modifier bitmask constants
pub mod modifiers {
    pub const SHIFT: u32 = 1 << 0;
    pub const CTRL:  u32 = 1 << 1;
    pub const ALT:   u32 = 1 << 2;
    pub const META:  u32 = 1 << 3;  // Win key / Cmd key
}

/// Common HID usage codes for key normalization
pub mod hid {
    pub const A: u32 = 0x04;
    pub const Z: u32 = 0x1D;
    pub const ENTER: u32 = 0x28;
    pub const ESCAPE: u32 = 0x29;
    pub const BACKSPACE: u32 = 0x2A;
    pub const TAB: u32 = 0x2B;
    pub const SPACE: u32 = 0x2C;
    pub const F1:  u32 = 0x3A;
    pub const F12: u32 = 0x45;
    pub const SCROLL_LOCK: u32 = 0x47;
    pub const PAUSE: u32 = 0x48;
    pub const LEFT_CTRL:  u32 = 0xE0;
    pub const LEFT_SHIFT: u32 = 0xE1;
    pub const LEFT_ALT:   u32 = 0xE2;
    pub const LEFT_META:  u32 = 0xE3;
    pub const RIGHT_CTRL: u32 = 0xE4;
}
