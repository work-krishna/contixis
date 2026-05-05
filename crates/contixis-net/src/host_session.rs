use anyhow::{anyhow, Result};
use base64::Engine;
use contixis_core::session::{SessionEvent, SessionFsm};
use contixis_crypto::{CertificateAuthority, HostStore, PairingManager};
use contixis_proto::{Handshake, MsgType, PairingRequest, PairingRequired, SessionReady};
use parking_lot::Mutex;
use quinn::Connection;
use std::sync::Arc;
use tokio::time::{timeout, Duration};

use crate::framing::{FrameReader, FrameWriter};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Information returned after a successful handshake.
pub struct HandshakeInfo {
    pub device_id: String,
    pub display_name: String,
    pub os_type: String,
    pub screens: Vec<contixis_proto::ScreenInfo>,
}

/// Events emitted during the handshake before it completes.
pub enum HandshakeEvent {
    Connected { device_id: String, display_name: String, os_type: String },
    PairingPin { device_id: String, pin: String },
}

pub struct HostSession {
    conn: Connection,
    ca: Arc<CertificateAuthority>,
    pairing: Arc<PairingManager>,
    store: Arc<Mutex<HostStore>>,
}

impl HostSession {
    pub fn new(
        conn: Connection,
        ca: Arc<CertificateAuthority>,
        pairing: Arc<PairingManager>,
        store: Arc<Mutex<HostStore>>,
    ) -> Self {
        Self { conn, ca, pairing, store }
    }

    /// Drive handshake → pairing (if needed) → established.
    /// `on_event` is called synchronously as status updates arrive.
    pub async fn run_handshake<F>(&mut self, on_event: F) -> Result<HandshakeInfo>
    where
        F: Fn(HandshakeEvent),
    {
        let (send, recv) = timeout(HANDSHAKE_TIMEOUT, self.conn.accept_bi()).await??;
        let mut reader = FrameReader::new(recv);
        let mut writer = FrameWriter::new(send);

        let frame = reader.read_frame().await?;
        if frame.msg_type != MsgType::Handshake {
            return Err(anyhow!("expected Handshake, got {:?}", frame.msg_type));
        }
        let hs: Handshake = prost::Message::decode(frame.payload.as_slice())?;
        let device_id    = hs.device_id.clone();
        let display_name = hs.device_name.clone();
        let os_type      = hs.os_type.clone();

        on_event(HandshakeEvent::Connected {
            device_id:    device_id.clone(),
            display_name: display_name.clone(),
            os_type:      os_type.clone(),
        });
        tracing::info!(%device_id, version = %hs.protocol_version, os = %os_type, "handshake received");

        let mut fsm = SessionFsm::new(device_id.clone());

        if self.store.lock().is_trusted(&device_id) {
            fsm.apply(SessionEvent::SessionReady);
            writer.write_proto(MsgType::SessionReady, &SessionReady {
                session_id: uuid::Uuid::new_v4().to_string(),
                ca_cert_der: self.ca.cert_der().to_vec(),
            }).await?;
        } else {
            fsm.apply(SessionEvent::NeedsPairing);
            fsm.apply(SessionEvent::PairingStarted);

            let pin = self.pairing.create_session(device_id.clone());
            tracing::info!(%device_id, %pin, "*** PAIRING PIN — enter this on the agent machine ***");
            on_event(HandshakeEvent::PairingPin { device_id: device_id.clone(), pin });

            use rand::RngCore;
            let mut nonce = vec![0u8; 16];
            rand::thread_rng().fill_bytes(&mut nonce);
            writer.write_proto(MsgType::PairingRequired, &PairingRequired {
                nonce: nonce.clone(),
            }).await?;

            let frame = timeout(Duration::from_secs(120), reader.read_frame()).await??;
            if frame.msg_type != MsgType::PairingRequest {
                return Err(anyhow!("expected PairingRequest"));
            }
            let pr: PairingRequest = prost::Message::decode(frame.payload.as_slice())?;

            let csr_pem = bytes_to_pem(&pr.csr);
            match self.pairing.verify_and_consume_bytes(&device_id, &pr.csr, &nonce, &pr.pin_mac) {
                Ok(_) => {
                    let cert_der    = self.ca.sign_csr(&csr_pem, &device_id)?;
                    let ca_cert_der = self.ca.cert_der().to_vec();

                    writer.write_proto(MsgType::PairingSuccess, &contixis_proto::PairingSuccess {
                        agent_cert_der: cert_der.clone(),
                        ca_cert_der: ca_cert_der.clone(),
                    }).await?;

                    self.store.lock().add_agent(contixis_crypto::TrustedAgent {
                        device_id: device_id.clone(),
                        cert_der,
                        display_name: if display_name.is_empty() { None } else { Some(display_name.clone()) },
                    });

                    fsm.apply(SessionEvent::PairingSucceeded);
                    writer.write_proto(MsgType::SessionReady, &SessionReady {
                        session_id: uuid::Uuid::new_v4().to_string(),
                        ca_cert_der,
                    }).await?;
                }
                Err(e) => {
                    fsm.apply(SessionEvent::PairingFailed);
                    writer.write_proto(MsgType::PairingFailed, &contixis_proto::PairingFailed {
                        reason: e.to_string(),
                    }).await?;
                    return Err(e);
                }
            }
        }

        Ok(HandshakeInfo { device_id, display_name, os_type, screens: hs.screens })
    }
}

fn bytes_to_pem(der: &[u8]) -> String {
    let b64 = base64::engine::general_purpose::STANDARD.encode(der);
    let lines: Vec<String> = b64.as_bytes()
        .chunks(64)
        .map(|c| String::from_utf8_lossy(c).into_owned())
        .collect();
    format!(
        "-----BEGIN CERTIFICATE REQUEST-----\n{}\n-----END CERTIFICATE REQUEST-----\n",
        lines.join("\n")
    )
}
