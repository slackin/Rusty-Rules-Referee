//! Quick-connect pairing API endpoints.
//!
//! These endpoints allow a master server admin to enable quick-connect,
//! generate a time-limited pairing token, and have client bots pair
//! by presenting that token to receive TLS certificates.

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::config::RefereeConfig;
use crate::sync::ca;

use crate::web::auth::AdminOnly;
use crate::web::state::AppState;

// ---- Request / Response types ----

#[derive(Debug, Deserialize)]
pub struct EnableRequest {
    /// Token validity in minutes (default 30).
    #[serde(default = "default_expiry_minutes")]
    pub expiry_minutes: u64,
}

fn default_expiry_minutes() -> u64 {
    30
}

#[derive(Debug, Serialize)]
pub struct EnableResponse {
    pub token: String,
    pub expires_at: String,
    pub connect_command: String,
}

#[derive(Debug, Deserialize)]
pub struct PairRequest {
    pub token: String,
    pub server_name: String,
    pub address: String,
    pub port: u16,
}

#[derive(Debug, Serialize)]
pub struct PairResponse {
    pub server_id: i64,
    pub ca_cert: String,
    pub client_cert: String,
    pub client_key: String,
    pub master_sync_url: String,
}

// ---- Helpers ----

/// Generate a cryptographically random base62 token of the given length.
fn generate_token(len: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// Persist quick-connect state to the config TOML file.
fn persist_quick_connect(
    config_path: &str,
    enabled: bool,
    token: Option<&str>,
    expiry: Option<&str>,
) -> Result<(), String> {
    let content =
        std::fs::read_to_string(config_path).map_err(|e| format!("read config: {}", e))?;
    let mut doc: toml::Value =
        toml::from_str(&content).map_err(|e| format!("parse config: {}", e))?;

    if let Some(master) = doc.get_mut("master") {
        if let Some(table) = master.as_table_mut() {
            table.insert(
                "quick_connect_enabled".to_string(),
                toml::Value::Boolean(enabled),
            );
            if let Some(t) = token {
                table.insert(
                    "quick_connect_token".to_string(),
                    toml::Value::String(t.to_string()),
                );
            } else {
                table.remove("quick_connect_token");
            }
            if let Some(e) = expiry {
                table.insert(
                    "quick_connect_expiry".to_string(),
                    toml::Value::String(e.to_string()),
                );
            } else {
                table.remove("quick_connect_expiry");
            }
        }
    }

    let output = toml::to_string_pretty(&doc).map_err(|e| format!("serialize config: {}", e))?;
    std::fs::write(config_path, &output).map_err(|e| format!("write config: {}", e))?;
    Ok(())
}

// ---- Endpoints ----

/// POST /api/v1/pairing/enable — Generate a quick-connect token (AdminOnly).
pub async fn enable_pairing(
    AdminOnly(_claims): AdminOnly,
    State(state): State<AppState>,
    Json(body): Json<EnableRequest>,
) -> impl IntoResponse {
    // Check that we're running in master mode (have a [master] config section)
    let master_config = match &state.config.master {
        Some(m) => m.clone(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Quick-connect is only available in master mode"})),
            )
                .into_response();
        }
    };

    // Ensure CA exists (needed to sign client certs)
    if master_config.ca_cert.is_empty() || master_config.ca_key.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Master CA is not configured. Run the master setup first."})),
        )
            .into_response();
    }

    let token = generate_token(32);
    let expiry =
        chrono::Utc::now() + chrono::Duration::minutes(body.expiry_minutes as i64);
    let expiry_str = expiry.to_rfc3339();

    // Persist to config file
    if let Err(e) = persist_quick_connect(
        &state.config_path,
        true,
        Some(&token),
        Some(&expiry_str),
    ) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to persist config: {}", e)})),
        )
            .into_response();
    }

    // Build a user-friendly connect command
    let web_port = state.config.web.port;
    let public_ip = &state.config.server.public_ip;
    let connect_command =
        format!("Quick-connect key: {token}  |  Master: {public_ip}:{web_port}");

    info!(
        expiry_minutes = body.expiry_minutes,
        "Quick-connect pairing enabled"
    );

    // Log audit event
    let _ = state
        .storage
        .save_audit_entry(&crate::core::AuditEntry {
            id: 0,
            admin_user_id: Some(_claims.user_id),
            action: "pairing_enabled".to_string(),
            detail: format!("Quick-connect enabled for {} minutes", body.expiry_minutes),
            ip_address: None,
            created_at: chrono::Utc::now(),
        })
        .await;

    Json(EnableResponse {
        token,
        expires_at: expiry_str,
        connect_command,
    })
    .into_response()
}

/// POST /api/v1/pairing/disable — Disable quick-connect (AdminOnly).
pub async fn disable_pairing(
    AdminOnly(_claims): AdminOnly,
    State(state): State<AppState>,
) -> impl IntoResponse {
    if let Err(e) = persist_quick_connect(&state.config_path, false, None, None) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to persist config: {}", e)})),
        )
            .into_response();
    }

    info!("Quick-connect pairing disabled");

    let _ = state
        .storage
        .save_audit_entry(&crate::core::AuditEntry {
            id: 0,
            admin_user_id: Some(_claims.user_id),
            action: "pairing_disabled".to_string(),
            detail: "Quick-connect disabled".to_string(),
            ip_address: None,
            created_at: chrono::Utc::now(),
        })
        .await;

    Json(serde_json::json!({"ok": true})).into_response()
}

/// POST /api/v1/pairing/pair — Pair a client bot using a quick-connect token (NO AUTH).
///
/// This endpoint is intentionally unauthenticated — access is controlled by
/// the time-limited quick-connect token.
pub async fn pair_client(
    State(state): State<AppState>,
    Json(body): Json<PairRequest>,
) -> impl IntoResponse {
    // Load current config from disk to get the latest token state
    let config = match RefereeConfig::from_file(std::path::Path::new(&state.config_path)) {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to read server configuration"})),
            )
                .into_response();
        }
    };

    let master_config = match &config.master {
        Some(m) => m,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Server is not running in master mode"})),
            )
                .into_response();
        }
    };

    // Check quick-connect is enabled
    if !master_config.quick_connect_enabled {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "Quick-connect is not enabled on this server"})),
        )
            .into_response();
    }

    // Validate token
    let expected_token = match &master_config.quick_connect_token {
        Some(t) => t,
        None => {
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({"error": "No quick-connect token configured"})),
            )
                .into_response();
        }
    };

    // Constant-time comparison to prevent timing attacks
    if body.token.len() != expected_token.len()
        || !body
            .token
            .bytes()
            .zip(expected_token.bytes())
            .all(|(a, b)| a == b)
    {
        warn!(
            address = %body.address,
            "Quick-connect pairing failed: invalid token"
        );
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "Invalid quick-connect token"})),
        )
            .into_response();
    }

    // Check expiry
    if let Some(expiry_str) = &master_config.quick_connect_expiry {
        if let Ok(expiry) = chrono::DateTime::parse_from_rfc3339(expiry_str) {
            if chrono::Utc::now() > expiry {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({"error": "Quick-connect token has expired"})),
                )
                    .into_response();
            }
        }
    }

    // Load CA cert + key to sign the client certificate
    let ca_cert_pem = match std::fs::read_to_string(&master_config.ca_cert) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to read CA cert: {}", e)})),
            )
                .into_response();
        }
    };

    let ca_key_pem = match std::fs::read_to_string(&master_config.ca_key) {
        Ok(k) => k,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to read CA key: {}", e)})),
            )
                .into_response();
        }
    };

    // Generate client certificate
    let client_certs = match ca::generate_client_cert(
        &ca_cert_pem,
        &ca_key_pem,
        &body.server_name,
        None, // Don't write to disk — return PEM strings to the client
    ) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to generate client cert: {}", e)})),
            )
                .into_response();
        }
    };

    // Compute cert fingerprint for registration
    let fingerprint = {
        use sha2::{Digest, Sha256};
        // Parse the PEM to get DER bytes
        let mut reader = std::io::BufReader::new(client_certs.cert_pem.as_bytes());
        let der_certs: Vec<_> = rustls_pemfile::certs(&mut reader)
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_default();
        if let Some(cert_der) = der_certs.first() {
            let digest = Sha256::digest(cert_der.as_ref());
            digest
                .iter()
                .enumerate()
                .map(|(i, b)| {
                    if i > 0 {
                        format!(":{:02X}", b)
                    } else {
                        format!("{:02X}", b)
                    }
                })
                .collect::<String>()
        } else {
            String::new()
        }
    };

    // Register the server in the database
    let server = crate::core::GameServer {
        id: 0,
        name: body.server_name.clone(),
        address: body.address.clone(),
        port: body.port,
        status: "online".to_string(),
        current_map: None,
        player_count: 0,
        max_clients: 0,
        last_seen: Some(chrono::Utc::now()),
        config_json: None,
        config_version: 1,
        cert_fingerprint: Some(fingerprint),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let server_id = match state.storage.save_server(&server).await {
        Ok(id) => id,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to register server: {}", e)})),
            )
                .into_response();
        }
    };

    // Build the master sync URL for the client config
    let master_sync_url = format!(
        "https://{}:{}",
        state.config.server.public_ip, master_config.port
    );

    info!(
        server_id,
        server_name = %body.server_name,
        address = %body.address,
        "Client bot paired via quick-connect"
    );

    let _ = state
        .storage
        .save_audit_entry(&crate::core::AuditEntry {
            id: 0,
            admin_user_id: None,
            action: "client_paired".to_string(),
            detail: format!(
                "Client '{}' ({}:{}) paired via quick-connect, assigned server_id={}",
                body.server_name, body.address, body.port, server_id
            ),
            ip_address: None,
            created_at: chrono::Utc::now(),
        })
        .await;

    Json(PairResponse {
        server_id,
        ca_cert: ca_cert_pem,
        client_cert: client_certs.cert_pem,
        client_key: client_certs.key_pem,
        master_sync_url,
    })
    .into_response()
}
