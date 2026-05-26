use anyhow::{anyhow, Result};
use contixis_crypto::{AgentStore, DeviceIdentity, TrustedHost};
use contixis_proto::{Handshake, MsgType, PairingRequest, SessionReady};
use quinn::Connection;
use std::sync::Arc;
use parking_lot::Mutex;
use tokio::time::{timeout, Duration};

use crate::framing::{FrameReader, FrameWriter};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);

/// Manages the outbound connection lifecycle on the agent side.
pub struct AgentSession {
    conn: Connection,
    identity: Arc<Mutex<DeviceIdentity>>,
    store: Arc<Mutex<AgentStore>>,
}

impl AgentSession {
    pub fn new(
        conn: Connection,
        identity: Arc<Mutex<DeviceIdentity>>,
        store: Arc<Mutex<AgentStore>>,
    ) -> Self {
        Self { conn, identity, store }
    }

    /// Open the control stream and drive handshake → pairing (if needed) → established.
    pub async fn run_handshake<F, Fut>(
        &mut self,
        screens: Vec<contixis_proto::ScreenInfo>,
        pin_provider: F,
    ) -> Result<String>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = String>,
    {
        let (send, recv) = self.conn.open_bi().await?;
        let mut writer = FrameWriter::new(send);
        let mut reader = FrameReader::new(recv);

        let device_id = self.identity.lock().device_id.clone();

        // Send Handshake.
        let hs = Handshake {
            device_id: device_id.clone(),
            protocol_version: "1".to_string(),
            device_name: hostname(),
            os_type: std::env::consts::OS.to_string(),
            screens,
        };
        writer.write_proto(MsgType::Handshake, &hs).await?;

        // Read host response.
        let frame = timeout(HANDSHAKE_TIMEOUT, reader.read_frame()).await??;

        match frame.msg_type {
            MsgType::SessionReady => {
                let ready: SessionReady = prost::Message::decode(frame.payload.as_slice())?;
                tracing::info!(%device_id, session = %ready.session_id, "session established (already paired)");
                return Ok(device_id);
            }
            MsgType::PairingRequired => {
                let pr: contixis_proto::PairingRequired =
                    prost::Message::decode(frame.payload.as_slice())?;

                let pin = pin_provider().await;
                tracing::info!(%device_id, "pairing required — user entered PIN");

                let csr_pem = self.identity.lock().csr_pem()?;
                let csr_der = pem_to_der(&csr_pem);

                // pin_mac = HMAC-SHA256(pin, csr_der || nonce)
                let pin_mac = compute_hmac(&pin, &csr_der, &pr.nonce);

                let pairing_req = PairingRequest {
                    device_name: hostname(),
                    csr: csr_der,
                    pin_mac,
                };
                writer.write_proto(MsgType::PairingRequest, &pairing_req).await?;

                // Wait for success or failure.
                let frame = timeout(Duration::from_secs(30), reader.read_frame()).await??;
                match frame.msg_type {
                    MsgType::PairingSuccess => {
                        let success: contixis_proto::PairingSuccess =
                            prost::Message::decode(frame.payload.as_slice())?;

                        self.identity.lock().install_cert(success.agent_cert_der.clone());

                        let host_id = hex::encode(&success.ca_cert_der[..8.min(success.ca_cert_der.len())]);
                        self.store.lock().add_host(TrustedHost {
                            host_id: host_id.clone(),
                            ca_der: success.ca_cert_der,
                            last_addr: Some(self.conn.remote_address()),
                            display_name: None,
                        });

                        tracing::info!(%device_id, %host_id, "pairing successful");

                        // Expect SessionReady.
                        let frame = timeout(Duration::from_secs(10), reader.read_frame()).await??;
                        if frame.msg_type != MsgType::SessionReady {
                            return Err(anyhow!("expected SessionReady after pairing"));
                        }
                        Ok(device_id)
                    }
                    MsgType::PairingFailed => {
                        let fail: contixis_proto::PairingFailed =
                            prost::Message::decode(frame.payload.as_slice())?;
                        Err(anyhow!("pairing failed: {}", fail.reason))
                    }
                    other => Err(anyhow!("unexpected {:?} during pairing", other)),
                }
            }
            other => Err(anyhow!("unexpected {:?} in handshake", other)),
        }
    }
}

fn compute_hmac(pin: &str, csr_der: &[u8], nonce: &[u8]) -> Vec<u8> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(pin.as_bytes()).unwrap();
    mac.update(csr_der);
    mac.update(nonce);
    mac.finalize().into_bytes().to_vec()
}

fn pem_to_der(pem: &str) -> Vec<u8> {
    use rustls_pemfile::csr;
    let mut cursor = std::io::BufReader::new(pem.as_bytes());
    match csr(&mut cursor) {
        Ok(Some(c)) => c.as_ref().to_vec(),
        _ => vec![],
    }
}

fn hostname() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "contixis-agent".into())
}
