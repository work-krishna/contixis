use anyhow::Result;
use rustls::pki_types::CertificateDer;
use rustls::RootCertStore;
use rustls::server::WebPkiClientVerifier;
use std::sync::Arc;

/// Build a `RootCertStore` pinned to a single CA DER certificate.
pub fn pinned_root_store(ca_der: &[u8]) -> Result<Arc<RootCertStore>> {
    let mut store = RootCertStore::empty();
    store.add(CertificateDer::from(ca_der.to_vec()))
        .map_err(|e| anyhow::anyhow!("failed to add CA cert: {}", e))?;
    Ok(Arc::new(store))
}

/// Build a rustls client-cert verifier for the **host** (server) side that only
/// trusts agent certificates issued by the host's own CA.
pub fn agent_cert_verifier(
    agent_ca_der: &[u8],
) -> Result<Arc<dyn rustls::server::danger::ClientCertVerifier>> {
    let store = pinned_root_store(agent_ca_der)?;
    let verifier = WebPkiClientVerifier::builder(store)
        .build()
        .map_err(|e| anyhow::anyhow!("build client verifier: {}", e))?;
    Ok(verifier)
}

