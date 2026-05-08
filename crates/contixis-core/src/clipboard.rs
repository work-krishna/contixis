use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub sequence: u64,
    pub source_device: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

/// Host-side clipboard arbitrator.
///
/// Sequence numbers prevent echo: when the host distributes clipboard data to
/// all agents, each agent records the sequence it last received.  If the agent
/// subsequently notifies the host of a "new" clipboard with the same sequence,
/// the host ignores it as an echo.
pub struct ClipboardManager {
    inner: Arc<Mutex<ClipboardState>>,
}

struct ClipboardState {
    current: Option<ClipboardEntry>,
    seq: u64,
    /// Per-device last-ack'd sequence to suppress echoes.
    device_seq: std::collections::HashMap<String, u64>,
}

impl ClipboardManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ClipboardState {
                current: None,
                seq: 0,
                device_seq: Default::default(),
            })),
        }
    }

    /// A device reports it has new clipboard content of the given MIME type.
    /// Returns the sequence number the host assigned, or None if content is identical.
    pub fn notify(&self, device_id: &str, mime_type: String, data: Vec<u8>) -> Option<u64> {
        let mut s = self.inner.lock();

        // Suppress no-op updates — identical content regardless of source.
        // Natural loop prevention: the host monitor ignores unchanged clipboard,
        // and the agent poller tracks its own `last` sent value.
        if let Some(entry) = &s.current {
            if entry.data == data && entry.mime_type == mime_type {
                return None;
            }
        }

        s.seq += 1;
        let seq = s.seq;
        s.current = Some(ClipboardEntry {
            sequence: seq,
            source_device: device_id.to_string(),
            mime_type,
            data,
        });
        Some(seq)
    }

    /// A device acknowledges receiving clipboard sequence `seq`.
    pub fn ack(&self, device_id: &str, seq: u64) {
        self.inner.lock().device_seq.insert(device_id.to_string(), seq);
    }

    /// Returns the current clipboard entry if `requesting_device` doesn't
    /// already have it.
    pub fn get_for_device(&self, requesting_device: &str) -> Option<ClipboardEntry> {
        let s = self.inner.lock();
        let entry = s.current.as_ref()?;
        let last = s.device_seq.get(requesting_device).copied().unwrap_or(0);
        if last >= entry.sequence {
            None
        } else {
            Some(entry.clone())
        }
    }

    pub fn current_seq(&self) -> u64 {
        self.inner.lock().seq
    }
}

impl Default for ClipboardManager {
    fn default() -> Self { Self::new() }
}
