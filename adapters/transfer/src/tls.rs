use crate::Error;
use rustls::{
    ClientConfig, ServerConfig,
    pki_types::{CertificateDer, PrivatePkcs8KeyDer},
};
use std::sync::Arc;
pub struct Identity {
    pub certificate: Vec<u8>,
    pub server: Arc<ServerConfig>,
}
impl Identity {
    pub fn generate() -> Result<Self, Error> {
        Self::generate_names(vec!["cine-transfer.local".into()])
    }
    pub fn generate_names(names: Vec<String>) -> Result<Self, Error> {
        let cert = rcgen::generate_simple_self_signed(names).map_err(|_| Error::Tls)?;
        let certificate = cert.cert.der().to_vec();
        Self::from_der(certificate, cert.signing_key.serialize_der())
    }
    /// Explicit operator-provisioned identity, with the same TLS/pin model.
    /// The caller reads private keys locally; never serialize them into signaling.
    pub fn from_der(certificate: Vec<u8>, pkcs8: Vec<u8>) -> Result<Self, Error> {
        if certificate.is_empty() || certificate.len() > 2048 || pkcs8.len() > 8192 {
            return Err(Error::Tls);
        }
        let key = PrivatePkcs8KeyDer::from(pkcs8);
        let mut config =
            ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_protocol_versions(&[&rustls::version::TLS13])
                .map_err(|_| Error::Tls)?
                .with_no_client_auth()
                .with_single_cert(vec![CertificateDer::from(certificate.clone())], key.into())
                .map_err(|_| Error::Tls)?;
        config.max_early_data_size = 0;
        config.send_tls13_tickets = 0;
        Ok(Self {
            certificate,
            server: Arc::new(config),
        })
    }
}
pub fn client(certificate: &[u8]) -> Result<Arc<ClientConfig>, Error> {
    if certificate.len() > 2048 {
        return Err(Error::Tls);
    }
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(CertificateDer::from(certificate.to_vec()))
        .map_err(|_| Error::Tls)?;
    let mut config =
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_protocol_versions(&[&rustls::version::TLS13])
            .map_err(|_| Error::Tls)?
            .with_root_certificates(roots)
            .with_no_client_auth();
    config.resumption = rustls::client::Resumption::disabled();
    Ok(Arc::new(config))
}
