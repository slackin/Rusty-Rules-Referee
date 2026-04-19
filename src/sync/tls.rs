//! mTLS configuration helpers for master/client communication.

use std::fs;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio_rustls::TlsAcceptor;

/// Load PEM certificates from a file.
pub fn load_certs(path: &Path) -> anyhow::Result<Vec<CertificateDer<'static>>> {
    let file = fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    let certs = rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()?;
    if certs.is_empty() {
        anyhow::bail!("No certificates found in {}", path.display());
    }
    Ok(certs)
}

/// Load a PEM private key from a file (RSA, PKCS8, or EC).
pub fn load_private_key(path: &Path) -> anyhow::Result<PrivateKeyDer<'static>> {
    let file = fs::File::open(path)?;
    let mut reader = BufReader::new(file);

    // Try PKCS8 first, then RSA, then EC
    for item in rustls_pemfile::read_all(&mut reader) {
        match item? {
            rustls_pemfile::Item::Pkcs8Key(key) => return Ok(PrivateKeyDer::Pkcs8(key)),
            rustls_pemfile::Item::Pkcs1Key(key) => return Ok(PrivateKeyDer::Pkcs1(key)),
            rustls_pemfile::Item::Sec1Key(key) => return Ok(PrivateKeyDer::Sec1(key)),
            _ => continue,
        }
    }
    anyhow::bail!("No private key found in {}", path.display())
}

/// Build a TLS acceptor for the master server with client certificate verification.
pub fn build_master_tls_acceptor(
    cert_path: &Path,
    key_path: &Path,
    ca_path: &Path,
) -> anyhow::Result<TlsAcceptor> {
    let certs = load_certs(cert_path)?;
    let key = load_private_key(key_path)?;
    let ca_certs = load_certs(ca_path)?;

    // Build a root cert store with the CA cert for client verification
    let mut client_auth_roots = rustls::RootCertStore::empty();
    for cert in &ca_certs {
        client_auth_roots.add(cert.clone())?;
    }

    let client_verifier = rustls::server::WebPkiClientVerifier::builder(
        Arc::new(client_auth_roots),
    )
    .build()?;

    let config = rustls::ServerConfig::builder()
        .with_client_cert_verifier(client_verifier)
        .with_single_cert(certs, key)?;

    Ok(TlsAcceptor::from(Arc::new(config)))
}

/// Build a TLS client config for connecting to the master server.
pub fn build_client_tls_config(
    cert_path: &Path,
    key_path: &Path,
    ca_path: &Path,
) -> anyhow::Result<Arc<rustls::ClientConfig>> {
    let certs = load_certs(cert_path)?;
    let key = load_private_key(key_path)?;
    let ca_certs = load_certs(ca_path)?;

    let mut root_store = rustls::RootCertStore::empty();
    for cert in &ca_certs {
        root_store.add(cert.clone())?;
    }

    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_client_auth_cert(certs, key)?;

    Ok(Arc::new(config))
}

/// Compute the SHA-256 fingerprint of a DER-encoded certificate.
pub fn cert_fingerprint(cert: &CertificateDer<'_>) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write;

    let digest = Sha256::digest(cert.as_ref());
    let mut hex = String::with_capacity(digest.len() * 3);
    for (i, byte) in digest.iter().enumerate() {
        if i > 0 {
            hex.push(':');
        }
        write!(hex, "{:02X}", byte).unwrap();
    }
    hex
}
