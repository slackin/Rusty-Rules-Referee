use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};

use crate::web::auth::{AdminOnly, AuthUser};
use crate::web::state::AppState;

/// GET /api/v1/config — return current config (secrets redacted).
pub async fn get_config(
    AdminOnly(_claims): AdminOnly,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let mut config = state.config.clone();
    config.server.rcon_password = "********".to_string();
    if let Some(ref mut _s) = config.web.jwt_secret {
        *_s = "********".to_string();
    }
    Json(serde_json::json!({"config": config}))
}

/// PUT /api/v1/config — update config and write to disk.
pub async fn update_config(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    // Read the current TOML file, merge updates, write back
    let path = std::path::Path::new(&state.config_path);
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("Cannot read config: {}", e)}))).into_response();
        }
    };

    let mut doc: toml::Table = match content.parse() {
        Ok(d) => d,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("Cannot parse config: {}", e)}))).into_response();
        }
    };

    // Merge provided fields into the TOML document
    if let Some(referee) = body.get("referee").and_then(|v| v.as_object()) {
        let section = doc.entry("referee").or_insert_with(|| toml::Value::Table(toml::Table::new()));
        if let Some(t) = section.as_table_mut() {
            for (k, v) in referee {
                if let Some(tv) = json_to_toml(v) {
                    t.insert(k.clone(), tv);
                }
            }
        }
    }

    if let Some(server) = body.get("server").and_then(|v| v.as_object()) {
        let section = doc.entry("server").or_insert_with(|| toml::Value::Table(toml::Table::new()));
        if let Some(t) = section.as_table_mut() {
            for (k, v) in server {
                // Don't allow overwriting rcon_password with the redacted value
                if k == "rcon_password" && v.as_str() == Some("********") {
                    continue;
                }
                if let Some(tv) = json_to_toml(v) {
                    t.insert(k.clone(), tv);
                }
            }
        }
    }

    if let Some(web) = body.get("web").and_then(|v| v.as_object()) {
        let section = doc.entry("web").or_insert_with(|| toml::Value::Table(toml::Table::new()));
        if let Some(t) = section.as_table_mut() {
            for (k, v) in web {
                if k == "jwt_secret" && v.as_str() == Some("********") {
                    continue;
                }
                if let Some(tv) = json_to_toml(v) {
                    t.insert(k.clone(), tv);
                }
            }
        }
    }

    // Merge plugins array
    if let Some(plugins) = body.get("plugins").and_then(|v| v.as_array()) {
        if let Some(tv) = json_to_toml(&serde_json::Value::Array(plugins.clone())) {
            doc.insert("plugins".to_string(), tv);
        }
    }

    let output = toml::to_string_pretty(&doc).unwrap_or_default();
    if let Err(e) = std::fs::write(path, &output) {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("Cannot write config: {}", e)}))).into_response();
    }

    // Audit log
    let _ = state.storage.save_audit_entry(&crate::core::AuditEntry {
        id: 0,
        admin_user_id: Some(claims.user_id),
        action: "config_update".to_string(),
        detail: "Configuration updated via web UI".to_string(),
        ip_address: None,
        created_at: chrono::Utc::now(),
    }).await;

    Json(serde_json::json!({"status": "ok", "message": "Configuration saved. Some changes may require a restart."})).into_response()
}

fn json_to_toml(v: &serde_json::Value) -> Option<toml::Value> {
    match v {
        serde_json::Value::String(s) => Some(toml::Value::String(s.clone())),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(toml::Value::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Some(toml::Value::Float(f))
            } else {
                None
            }
        }
        serde_json::Value::Bool(b) => Some(toml::Value::Boolean(*b)),
        serde_json::Value::Array(arr) => {
            let items: Vec<toml::Value> = arr.iter().filter_map(json_to_toml).collect();
            Some(toml::Value::Array(items))
        }
        serde_json::Value::Object(obj) => {
            let mut table = toml::Table::new();
            for (k, v) in obj {
                if let Some(tv) = json_to_toml(v) {
                    table.insert(k.clone(), tv);
                }
            }
            Some(toml::Value::Table(table))
        }
        serde_json::Value::Null => None,
    }
}
