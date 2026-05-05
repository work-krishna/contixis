use serde::{Deserialize, Serialize};
use std::time::Instant;

/// All possible states a session can be in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    /// Transport connected, waiting for Handshake.
    Connected,
    /// Handshake received, pairing required before proceeding.
    PairingRequired,
    /// Host has presented a PIN, waiting for agent to confirm.
    PairingPending,
    /// Fully paired and authenticated.
    Established,
    /// Session is shutting down.
    Terminating,
    /// Terminal state — connection closed.
    Closed,
}

/// Events that drive the session FSM.
#[derive(Debug, Clone)]
pub enum SessionEvent {
    HandshakeReceived { device_id: String, version: u32 },
    NeedsPairing,
    PairingStarted,
    PairingSucceeded,
    PairingFailed,
    SessionReady,
    TerminateRequested,
    TransportClosed,
}

/// Per-session finite state machine.
pub struct SessionFsm {
    pub device_id: String,
    pub state: SessionState,
    pub established_at: Option<Instant>,
}

impl SessionFsm {
    pub fn new(device_id: String) -> Self {
        Self {
            device_id,
            state: SessionState::Connected,
            established_at: None,
        }
    }

    /// Apply an event, transitioning to the next state.
    /// Returns the new state, or the unchanged state if the event is not
    /// applicable in the current state (log-worthy but non-fatal).
    pub fn apply(&mut self, event: SessionEvent) -> SessionState {
        use SessionEvent::*;
        use SessionState::*;

        let next = match (&self.state, event) {
            (Connected, HandshakeReceived { .. }) => Connected,
            (Connected, NeedsPairing) => PairingRequired,
            (Connected, SessionReady) => Established,

            (PairingRequired, PairingStarted) => PairingPending,

            (PairingPending, PairingSucceeded) => Established,
            (PairingPending, PairingFailed) => PairingRequired,

            (Established, TerminateRequested) => Terminating,
            (Terminating, TransportClosed) => Closed,

            // Any state can transition directly to Closed on transport error.
            (_, TransportClosed) => Closed,

            (current, _) => {
                tracing::warn!(
                    device_id = %self.device_id,
                    state = ?current,
                    "unexpected session event — ignored"
                );
                *current
            }
        };

        if next == SessionState::Established && self.established_at.is_none() {
            self.established_at = Some(Instant::now());
        }

        tracing::debug!(
            device_id = %self.device_id,
            from = ?self.state,
            to   = ?next,
            "session state transition"
        );

        self.state = next;
        next
    }

    pub fn is_established(&self) -> bool {
        self.state == SessionState::Established
    }

    pub fn is_closed(&self) -> bool {
        self.state == SessionState::Closed
    }
}
