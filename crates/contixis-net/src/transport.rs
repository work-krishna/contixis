use anyhow::Result;
use contixis_crypto::{agent_cert_verifier, pinned_root_store};
use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use quinn::{ClientConfig, ServerConfig};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, Error as TlsError, SignatureScheme};
use rustls_pemfile::private_key;
use std::io::BufReader;
use std::sync::Arc;

/// Host-side QUIC server: verifies agent certs against CA (for reconnects from paired agents).
pub fn make_host_tls_config(
    cert_der: Vec<u8>,
    key_pem: &str,
    agent_ca_der: &[u8],
) -> Result<ServerConfig> {
    let cert = CertificateDer::from(cert_der);
    let key  = load_private_key(key_pem)?;
    let verifier = agent_cert_verifier(agent_ca_der)?;
    let tls = rustls::ServerConfig::builder()
        .with_client_cert_verifier(verifier)
        .with_single_cert(vec![cert], key)
        .map_err(|e| anyhow::anyhow!("server TLS config: {}", e))?;
    make_server_cfg(tls)
}

/// Host-side QUIC server: no client cert required (used during pairing).
pub fn make_host_tls_config_open(cert_der: Vec<u8>, key_pem: &str) -> Result<ServerConfig> {
    let cert = CertificateDer::from(cert_der);
    let key  = load_private_key(key_pem)?;
    let tls = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .map_err(|e| anyhow::anyhow!("server TLS config (open): {}", e))?;
    make_server_cfg(tls)
}

/// Agent-side QUIC client: verifies host cert against its CA (for paired reconnects).
pub fn make_agent_tls_config(
    cert_der: Vec<u8>,
    key_pem: &str,
    host_ca_der: &[u8],
) -> Result<ClientConfig> {
    let cert = CertificateDer::from(cert_der);
    let key  = load_private_key(key_pem)?;
    let root_store = pinned_root_store(host_ca_der)?;
    let tls = rustls::ClientConfig::builder()
        .with_root_certificates((*root_store).clone())
        .with_client_auth_cert(vec![cert], key)
        .map_err(|e| anyhow::anyhow!("client TLS config: {}", e))?;
    let quic = QuicClientConfig::try_from(tls)
        .map_err(|e| anyhow::anyhow!("QuicClientConfig: {}", e))?;
    Ok(ClientConfig::new(Arc::new(quic)))
}

/// Agent-side QUIC client: skips host cert verification (used for initial pairing connection).
pub fn make_agent_tls_config_insecure(cert_der: Vec<u8>, key_pem: &str) -> Result<ClientConfig> {
    let cert = CertificateDer::from(cert_der);
    let key  = load_private_key(key_pem)?;
    let tls = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(NoVerifier))
        .with_client_auth_cert(vec![cert], key)
        .map_err(|e| anyhow::anyhow!("client TLS config (insecure): {}", e))?;
    let quic = QuicClientConfig::try_from(tls)
        .map_err(|e| anyhow::anyhow!("QuicClientConfig: {}", e))?;
    Ok(ClientConfig::new(Arc::new(quic)))
}

fn make_server_cfg(tls: rustls::ServerConfig) -> Result<ServerConfig> {
    let quic = QuicServerConfig::try_from(tls)
        .map_err(|e| anyhow::anyhow!("QuicServerConfig: {}", e))?;
    let mut server_cfg = ServerConfig::with_crypto(Arc::new(quic));
    let transport = Arc::get_mut(&mut server_cfg.transport).unwrap();
    transport.max_idle_timeout(Some(
        quinn::IdleTimeout::try_from(std::time::Duration::from_secs(120)).unwrap(),
    ));
    transport.keep_alive_interval(Some(std::time::Duration::from_secs(15)));
    // Allow agents to open unidirectional streams (clipboard, etc.).
    transport.max_concurrent_uni_streams(16u32.into());
    Ok(server_cfg)
}

pub fn make_agent_transport() -> std::sync::Arc<quinn::TransportConfig> {
    let mut t = quinn::TransportConfig::default();
    t.max_idle_timeout(Some(
        quinn::IdleTimeout::try_from(std::time::Duration::from_secs(120)).unwrap(),
    ));
    t.keep_alive_interval(Some(std::time::Duration::from_secs(15)));
    // Allow the host to open unidirectional streams (event/clipboard forwarding).
    t.max_concurrent_uni_streams(16u32.into());
    std::sync::Arc::new(t)
}

fn load_private_key(key_pem: &str) -> Result<PrivateKeyDer<'static>> {
    let mut reader = BufReader::new(key_pem.as_bytes());
    private_key(&mut reader)?
        .ok_or_else(|| anyhow::anyhow!("no private key found in PEM"))
}

/// TLS verifier that accepts any server certificate — used only for initial pairing.
#[derive(Debug)]
struct NoVerifier;

impl ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls12_signature(
            message, cert, dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls13_signature(
            message, cert, dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}
