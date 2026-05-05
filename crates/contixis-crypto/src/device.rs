use anyhow::Result;
use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, PKCS_ECDSA_P256_SHA256};
use std::path::Path;
use uuid::Uuid;

/// Per-device identity: stable UUID + TLS keypair.
pub struct DeviceIdentity {
    pub device_id: String,
    key_pair: KeyPair,
    cert_der: Option<Vec<u8>>,
}

impl DeviceIdentity {
    /// Generate a new identity (first launch).
    pub fn generate() -> Result<Self> {
        let device_id = Uuid::new_v4().to_string();
        let key_pair  = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;
        tracing::info!(device_id, "Generated new device identity");
        Ok(Self { device_id, key_pair, cert_der: None })
    }

    /// Export a CSR (PEM) to send to the host during pairing.
    pub fn csr_pem(&self) -> Result<String> {
        let mut params = CertificateParams::default();
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, format!("contixis-device-{}", self.device_id));
        params.distinguished_name = dn;
        let csr = params.serialize_request(&self.key_pair)?;
        Ok(csr.pem()?)
    }

    /// Store the CA-signed certificate received after pairing.
    pub fn install_cert(&mut self, cert_der: Vec<u8>) {
        self.cert_der = Some(cert_der);
    }

    /// Private key PEM (never transmitted).
    pub fn private_key_pem(&self) -> String {
        self.key_pair.serialize_pem()
    }

    /// Save identity to disk.
    pub fn save(&self, dir: &Path) -> Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::write(dir.join("device_id.txt"), &self.device_id)?;
        std::fs::write(dir.join("device.key.pem"), self.private_key_pem())?;
        if let Some(der) = &self.cert_der {
            std::fs::write(dir.join("device.crt.der"), der)?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                dir.join("device.key.pem"),
                std::fs::Permissions::from_mode(0o600),
            )?;
        }
        Ok(())
    }

    /// Load from disk.
    pub fn load(dir: &Path) -> Result<Self> {
        let device_id = std::fs::read_to_string(dir.join("device_id.txt"))?.trim().to_string();
        let key_pem   = std::fs::read_to_string(dir.join("device.key.pem"))?;
        let key_pair  = KeyPair::from_pem(&key_pem)?;

        let cert_der_path = dir.join("device.crt.der");
        let cert_der = if cert_der_path.exists() {
            Some(std::fs::read(cert_der_path)?)
        } else {
            None
        };

        Ok(Self { device_id, key_pair, cert_der })
    }

    /// Load or generate, saving to disk.
    pub fn load_or_generate(dir: &Path) -> Result<Self> {
        if dir.join("device_id.txt").exists() {
            Self::load(dir)
        } else {
            let id = Self::generate()?;
            id.save(dir)?;
            Ok(id)
        }
    }

    pub fn has_cert(&self) -> bool { self.cert_der.is_some() }
    pub fn cert_der(&self) -> Option<&[u8]> { self.cert_der.as_deref() }

    /// A self-signed DER cert for use as the TLS transport cert before pairing.
    pub fn self_signed_cert_der(&self) -> Result<Vec<u8>> {
        let mut params = CertificateParams::default();
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, format!("contixis-agent-{}", self.device_id));
        params.distinguished_name = dn;
        let cert = params.self_signed(&self.key_pair)?;
        Ok(cert.der().to_vec())
    }

    /// DER cert to use for TLS connections: issued cert if paired, else self-signed.
    pub fn connection_cert_der(&self) -> Result<Vec<u8>> {
        if let Some(der) = &self.cert_der {
            Ok(der.clone())
        } else {
            self.self_signed_cert_der()
        }
    }
}
