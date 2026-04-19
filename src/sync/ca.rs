//! Certificate Authority operations for quick-connect pairing.
//!
//! The master server acts as its own CA. On first startup (or via the installer),
//! it generates a self-signed CA certificate. When a client bot pairs via
//! quick-connect, the master signs a client certificate using this CA.
//!
//! All certificates are generated in pure Rust using the `rcgen` crate.

use std::fs;
use std::path::Path;

use rcgen::{
    BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, KeyPair,
    KeyUsagePurpose,
};
use tracing::info;

/// Result of generating a CA.
pub struct CaFiles {
    pub ca_cert_pem: String,
    pub ca_key_pem: String,
}

/// Result of generating a signed certificate (server or client).
pub struct CertFiles {
    pub cert_pem: String,
    pub key_pem: String,
}

/// Generate a self-signed CA certificate and private key.
///
/// If `output_dir` is provided, writes `ca.crt` and `ca.key` to that directory.
pub fn generate_ca(output_dir: Option<&Path>) -> anyhow::Result<CaFiles> {
    let mut params = CertificateParams::default();
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params
        .distinguished_name
        .push(DnType::CommonName, "R3 Certificate Authority");
    params
        .distinguished_name
        .push(DnType::OrganizationName, "Rusty Rules Referee");
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];

    // 10-year validity
    let not_before = time::OffsetDateTime::now_utc();
    let not_after = not_before + time::Duration::days(3650);
    params.not_before = not_before;
    params.not_after = not_after;

    let key_pair = KeyPair::generate()?;
    let cert = params.self_signed(&key_pair)?;

    let ca_cert_pem = cert.pem();
    let ca_key_pem = key_pair.serialize_pem();

    if let Some(dir) = output_dir {
        fs::create_dir_all(dir)?;
        let cert_path = dir.join("ca.crt");
        let key_path = dir.join("ca.key");
        fs::write(&cert_path, &ca_cert_pem)?;
        fs::write(&key_path, &ca_key_pem)?;
        // Restrict key file permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600))?;
        }
        info!(path = %cert_path.display(), "CA certificate written");
        info!(path = %key_path.display(), "CA private key written");
    }

    Ok(CaFiles {
        ca_cert_pem,
        ca_key_pem,
    })
}

/// Generate a server certificate signed by the CA.
///
/// The certificate includes the given hostnames/IPs as Subject Alternative Names.
pub fn generate_server_cert(
    ca_cert_pem: &str,
    ca_key_pem: &str,
    san_entries: &[String],
    output_dir: Option<&Path>,
) -> anyhow::Result<CertFiles> {
    let ca_key = KeyPair::from_pem(ca_key_pem)?;
    let ca_params = CertificateParams::from_ca_cert_pem(ca_cert_pem)?;
    let ca_cert = ca_params.self_signed(&ca_key)?;

    let mut params = CertificateParams::default();
    params
        .distinguished_name
        .push(DnType::CommonName, "R3 Master Server");
    params
        .distinguished_name
        .push(DnType::OrganizationName, "Rusty Rules Referee");

    // Add SANs
    let mut subject_alt_names = Vec::new();
    for entry in san_entries {
        subject_alt_names.push(rcgen::SanType::DnsName(entry.clone().try_into()?));
    }
    // Always add localhost
    subject_alt_names.push(rcgen::SanType::DnsName("localhost".to_string().try_into()?));
    params.subject_alt_names = subject_alt_names;

    params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];

    // 5-year validity
    let not_before = time::OffsetDateTime::now_utc();
    let not_after = not_before + time::Duration::days(1825);
    params.not_before = not_before;
    params.not_after = not_after;

    let server_key = KeyPair::generate()?;
    let server_cert = params.signed_by(&server_key, &ca_cert, &ca_key)?;

    let cert_pem = server_cert.pem();
    let key_pem = server_key.serialize_pem();

    if let Some(dir) = output_dir {
        fs::create_dir_all(dir)?;
        let cert_path = dir.join("server.crt");
        let key_path = dir.join("server.key");
        fs::write(&cert_path, &cert_pem)?;
        fs::write(&key_path, &key_pem)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600))?;
        }
        info!(path = %cert_path.display(), "Server certificate written");
        info!(path = %key_path.display(), "Server private key written");
    }

    Ok(CertFiles { cert_pem, key_pem })
}

/// Generate a client certificate signed by the CA.
///
/// The `server_name` is embedded as the certificate's Common Name.
pub fn generate_client_cert(
    ca_cert_pem: &str,
    ca_key_pem: &str,
    server_name: &str,
    output_dir: Option<&Path>,
) -> anyhow::Result<CertFiles> {
    let ca_key = KeyPair::from_pem(ca_key_pem)?;
    let ca_params = CertificateParams::from_ca_cert_pem(ca_cert_pem)?;
    let ca_cert = ca_params.self_signed(&ca_key)?;

    let mut params = CertificateParams::default();
    params
        .distinguished_name
        .push(DnType::CommonName, server_name);
    params
        .distinguished_name
        .push(DnType::OrganizationName, "Rusty Rules Referee");

    params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];

    // 2-year validity
    let not_before = time::OffsetDateTime::now_utc();
    let not_after = not_before + time::Duration::days(730);
    params.not_before = not_before;
    params.not_after = not_after;

    let client_key = KeyPair::generate()?;
    let client_cert = params.signed_by(&client_key, &ca_cert, &ca_key)?;

    let cert_pem = client_cert.pem();
    let key_pem = client_key.serialize_pem();

    if let Some(dir) = output_dir {
        fs::create_dir_all(dir)?;
        let cert_path = dir.join("client.crt");
        let key_path = dir.join("client.key");
        fs::write(&cert_path, &cert_pem)?;
        fs::write(&key_path, &key_pem)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600))?;
        }
        info!(path = %cert_path.display(), "Client certificate written");
        info!(path = %key_path.display(), "Client private key written");
    }

    Ok(CertFiles { cert_pem, key_pem })
}

/// Ensure CA + server certs exist in the given directory, generating them if missing.
///
/// Called on master startup to auto-provision TLS infrastructure.
/// Returns `(ca_cert_pem, ca_key_pem)` for use during pairing.
pub fn ensure_master_certs(
    certs_dir: &Path,
    san_entries: &[String],
) -> anyhow::Result<(String, String)> {
    let ca_cert_path = certs_dir.join("ca.crt");
    let ca_key_path = certs_dir.join("ca.key");
    let server_cert_path = certs_dir.join("server.crt");
    let server_key_path = certs_dir.join("server.key");

    let (ca_cert_pem, ca_key_pem) = if ca_cert_path.exists() && ca_key_path.exists() {
        info!("Using existing CA certificate");
        (
            fs::read_to_string(&ca_cert_path)?,
            fs::read_to_string(&ca_key_path)?,
        )
    } else {
        info!("Generating new CA certificate...");
        let ca = generate_ca(Some(certs_dir))?;
        (ca.ca_cert_pem, ca.ca_key_pem)
    };

    if !server_cert_path.exists() || !server_key_path.exists() {
        info!("Generating new server certificate...");
        generate_server_cert(&ca_cert_pem, &ca_key_pem, san_entries, Some(certs_dir))?;
    } else {
        info!("Using existing server certificate");
    }

    Ok((ca_cert_pem, ca_key_pem))
}
