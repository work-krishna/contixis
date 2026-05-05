use anyhow::Result;
use rcgen::{
    Certificate, CertificateParams, CertificateSigningRequestParams,
    DistinguishedName, DnType, IsCa, BasicConstraints, KeyPair, PKCS_ECDSA_P256_SHA256,
};
use rustls_pemfile::certs;
use std::io::Cursor;
use std::path::Path;

/// Host's certificate authority — signs all agent certificates.
/// The CA private key never leaves the host.
pub struct CertificateAuthority {
    cert: Certificate,
    key_pair: KeyPair,
    cert_der: Vec<u8>,
}

impl CertificateAuthority {
    /// Generate a new CA on first launch. Persisted to disk.
    pub fn generate() -> Result<Self> {
        let key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;

        let mut params = CertificateParams::default();
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);

        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, "Contixis Root CA");
        dn.push(DnType::OrganizationName, "Contixis");
        params.distinguished_name = dn;

        params.not_before = rcgen::date_time_ymd(2024, 1, 1);
        params.not_after  = rcgen::date_time_ymd(2035, 1, 1);

        let cert = params.self_signed(&key_pair)?;
        let cert_der = cert.der().as_ref().to_vec();
        tracing::info!("Generated new CA certificate");
        Ok(Self { cert, key_pair, cert_der })
    }

    /// Load CA from PEM files.
    pub fn load(cert_pem: &str, key_pem: &str) -> Result<Self> {
        // Preserve original DER from the saved PEM before we reconstruct.
        let cert_der = pem_to_der(cert_pem);

        let key_pair = KeyPair::from_pem(key_pem)?;
        let params = CertificateParams::from_ca_cert_pem(cert_pem)?;
        let cert = params.self_signed(&key_pair)?;
        Ok(Self { cert, key_pair, cert_der })
    }

    /// Save CA to PEM files.
    pub fn save(&self, cert_path: &Path, key_path: &Path) -> Result<()> {
        std::fs::write(cert_path, self.cert.pem())?;
        std::fs::write(key_path, self.key_pair.serialize_pem())?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(key_path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }

    /// Load from files, generating if they don't exist.
    pub fn load_or_generate(cert_path: &Path, key_path: &Path) -> Result<Self> {
        if cert_path.exists() && key_path.exists() {
            let cert_pem = std::fs::read_to_string(cert_path)?;
            let key_pem  = std::fs::read_to_string(key_path)?;
            Self::load(&cert_pem, &key_pem)
        } else {
            let ca = Self::generate()?;
            if let Some(parent) = cert_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            ca.save(cert_path, key_path)?;
            Ok(ca)
        }
    }

    /// Sign an agent's CSR. Returns DER-encoded agent certificate.
    pub fn sign_csr(&self, csr_pem: &str, device_id: &str) -> Result<Vec<u8>> {
        let csr = CertificateSigningRequestParams::from_pem(csr_pem)?;
        let cert = csr.signed_by(&self.cert, &self.key_pair)?;
        tracing::info!(device_id, "Signed certificate for device");
        Ok(cert.der().as_ref().to_vec())
    }

    pub fn cert_der(&self) -> &[u8] { &self.cert_der }
    pub fn cert_pem(&self) -> Result<String> { Ok(self.cert.pem()) }
}

fn pem_to_der(pem: &str) -> Vec<u8> {
    let mut reader = Cursor::new(pem.as_bytes());
    let all: Vec<_> = certs(&mut reader).filter_map(|r| r.ok()).collect();
    all.into_iter().next().map(|c| c.as_ref().to_vec()).unwrap_or_default()
}
