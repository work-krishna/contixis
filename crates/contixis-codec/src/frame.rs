/// A captured video frame (raw BGRA pixels).
pub struct RawFrame {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub data: Vec<u8>,
}

/// An encoded video frame ready for transmission.
pub struct EncodedFrame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
    pub keyframe: bool,
    pub pts_us: u64,
}
