use anyhow::{anyhow, Result};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::Mutex;
use rand::Rng;

type HmacSha256 = Hmac<Sha256>;

const PIN_EXPIRY: Duration = Duration::from_secs(120);
const PIN_LENGTH: usize = 6;

#[derive(Debug)]
pub struct PairingSession {
    pub device_id: String,
    pub pin: String,
    created_at: Instant,
}

impl PairingSession {
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > PIN_EXPIRY
    }

    /// Compute HMAC-SHA256(pin, data) as raw bytes.
    pub fn compute_hmac_bytes(&self, data: &[u8]) -> Vec<u8> {
        let mut mac = HmacSha256::new_from_slice(self.pin.as_bytes())
            .expect("HMAC accepts any key length");
        mac.update(data);
        mac.finalize().into_bytes().to_vec()
    }

    /// Compute HMAC-SHA256(pin, csr_pem) as hex — legacy text-only variant.
    pub fn compute_hmac(&self, csr_pem: &str) -> String {
        hex::encode(self.compute_hmac_bytes(csr_pem.as_bytes()))
    }

    /// Constant-time comparison of expected HMAC (hex string) against provided.
    pub fn verify_hmac(&self, csr_pem: &str, provided_hmac: &str) -> bool {
        let expected = self.compute_hmac(csr_pem);
        if expected.len() != provided_hmac.len() {
            return false;
        }
        expected.as_bytes().ct_eq(provided_hmac.as_bytes()).into()
    }

    /// Verify raw bytes MAC (HMAC-SHA256(pin, csr_der || nonce)).
    pub fn verify_mac_bytes(&self, csr_der: &[u8], nonce: &[u8], provided_mac: &[u8]) -> bool {
        let mut data = Vec::with_capacity(csr_der.len() + nonce.len());
        data.extend_from_slice(csr_der);
        data.extend_from_slice(nonce);
        let expected = self.compute_hmac_bytes(&data);
        if expected.len() != provided_mac.len() {
            return false;
        }
        expected.ct_eq(provided_mac).into()
    }
}

/// Manages in-flight pairing sessions keyed by device_id.
pub struct PairingManager {
    sessions: Arc<Mutex<HashMap<String, PairingSession>>>,
}

impl PairingManager {
    pub fn new() -> Self {
        Self { sessions: Arc::new(Mutex::new(HashMap::new())) }
    }

    /// Create a new pairing session, returning the PIN to display to the user.
    pub fn create_session(&self, device_id: String) -> String {
        let pin = generate_pin();
        let session = PairingSession {
            device_id: device_id.clone(),
            pin: pin.clone(),
            created_at: Instant::now(),
        };
        self.sessions.lock().insert(device_id, session);
        pin
    }

    /// Verify HMAC (hex string) and consume the session on success.
    pub fn verify_and_consume(
        &self,
        device_id: &str,
        csr_pem: &str,
        provided_hmac: &str,
    ) -> Result<PairingSession> {
        let mut map = self.sessions.lock();
        let session = map.remove(device_id)
            .ok_or_else(|| anyhow!("no active pairing session for device {}", device_id))?;

        if session.is_expired() {
            return Err(anyhow!("pairing session expired"));
        }
        if !session.verify_hmac(csr_pem, provided_hmac) {
            return Err(anyhow!("pairing HMAC verification failed"));
        }
        Ok(session)
    }

    /// Verify raw-bytes MAC (HMAC over csr_der || nonce) and consume the session.
    pub fn verify_and_consume_bytes(
        &self,
        device_id: &str,
        csr_der: &[u8],
        nonce: &[u8],
        provided_mac: &[u8],
    ) -> Result<PairingSession> {
        let mut map = self.sessions.lock();
        let session = map.remove(device_id)
            .ok_or_else(|| anyhow!("no active pairing session for device {}", device_id))?;

        if session.is_expired() {
            return Err(anyhow!("pairing session expired"));
        }
        if !session.verify_mac_bytes(csr_der, nonce, provided_mac) {
            return Err(anyhow!("pairing MAC verification failed"));
        }
        Ok(session)
    }

    /// Remove all expired sessions.
    pub fn purge_expired(&self) {
        self.sessions.lock().retain(|_, s| !s.is_expired());
    }
}

impl Default for PairingManager {
    fn default() -> Self { Self::new() }
}

fn generate_pin() -> String {
    let n: u32 = rand::thread_rng().gen_range(0..1_000_000);
    format!("{:0>width$}", n, width = PIN_LENGTH)
}
