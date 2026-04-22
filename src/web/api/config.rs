use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::Row;
use tracing::{error, info};

use crate::storage::mysql::MysqlStorage;
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

    // Merge [update] section
    if let Some(update) = body.get("update").and_then(|v| v.as_object()) {
        let section = doc.entry("update").or_insert_with(|| toml::Value::Table(toml::Table::new()));
        if let Some(t) = section.as_table_mut() {
            for (k, v) in update {
                if let Some(tv) = json_to_toml(v) {
                    t.insert(k.clone(), tv);
                }
            }
        }
    }

    // Merge [map_repo] section
    if let Some(map_repo) = body.get("map_repo").and_then(|v| v.as_object()) {
        let section = doc.entry("map_repo").or_insert_with(|| toml::Value::Table(toml::Table::new()));
        if let Some(t) = section.as_table_mut() {
            for (k, v) in map_repo {
                if let Some(tv) = json_to_toml(v) {
                    t.insert(k.clone(), tv);
                }
            }
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
        server_id: None,
    }).await;

    Json(serde_json::json!({"status": "ok", "message": "Configuration saved. Some changes may require a restart."})).into_response()
}

// ---- Database Migration: SQLite → MySQL ----

#[derive(Deserialize)]
pub struct MigrateRequest {
    pub host: String,
    pub port: Option<u16>,
    pub username: String,
    pub password: String,
    pub database: Option<String>,
}

/// POST /api/v1/config/migrate-to-mysql — migrate SQLite data to MySQL.
pub async fn migrate_to_mysql(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    Json(body): Json<MigrateRequest>,
) -> impl IntoResponse {
    // Validate that current storage is SQLite
    if state.storage.protocol() != crate::storage::StorageProtocol::Sqlite {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": "Current database is not SQLite. Migration only works from SQLite to MySQL."
        }))).into_response();
    }

    let db_name = body.database.as_deref().unwrap_or("b3");
    let port = body.port.unwrap_or(3306);

    // Sanitize database name (alphanumeric, underscores, hyphens only)
    if !db_name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": "Invalid database name. Use only letters, numbers, underscores, and hyphens."
        }))).into_response();
    }

    // 1. Connect to MySQL server (without database) to create the database
    let admin_url = format!("mysql://{}:{}@{}:{}", body.username, body.password, body.host, port);
    let admin_pool = match sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
    {
        Ok(p) => p,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": format!("Cannot connect to MySQL server: {}", e)
            }))).into_response();
        }
    };

    // Create the database if it doesn't exist
    let create_db_sql = format!("CREATE DATABASE IF NOT EXISTS `{}`", db_name);
    if let Err(e) = sqlx::query(&create_db_sql).execute(&admin_pool).await {
        admin_pool.close().await;
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "error": format!("Cannot create database '{}': {}", db_name, e)
        }))).into_response();
    }
    admin_pool.close().await;

    // 2. Connect to MySQL with the target database and run migrations
    let mysql_url = format!("mysql://{}:{}@{}:{}/{}", body.username, body.password, body.host, port, db_name);
    let mysql_storage = match MysqlStorage::new(&mysql_url).await {
        Ok(s) => s,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": format!("Cannot initialize MySQL database: {}", e)
            }))).into_response();
        }
    };
    let mysql_pool = mysql_storage.pool();

    // 3. Open the SQLite database for reading
    let sqlite_dsn = &state.config.referee.database;
    let sqlite_path = sqlite_dsn.strip_prefix("sqlite://").unwrap_or(sqlite_dsn);
    let sqlite_opts = SqliteConnectOptions::new()
        .filename(sqlite_path)
        .read_only(true);
    let sqlite_pool = match SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(sqlite_opts)
        .await
    {
        Ok(p) => p,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": format!("Cannot open SQLite database: {}", e)
            }))).into_response();
        }
    };

    // 4. Migrate data table by table
    if let Err(e) = do_migrate(&sqlite_pool, mysql_pool).await {
        sqlite_pool.close().await;
        error!(error = %e, "Database migration failed");
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "error": format!("Migration failed: {}", e)
        }))).into_response();
    }
    sqlite_pool.close().await;

    // 5. Update the config file to use MySQL
    let config_path = std::path::Path::new(&state.config_path);
    if let Ok(content) = std::fs::read_to_string(config_path) {
        if let Ok(mut doc) = content.parse::<toml::Table>() {
            if let Some(referee) = doc.get_mut("referee").and_then(|v| v.as_table_mut()) {
                referee.insert(
                    "database".to_string(),
                    toml::Value::String(mysql_url.clone()),
                );
            }
            if let Ok(output) = toml::to_string_pretty(&doc) {
                let _ = std::fs::write(config_path, &output);
            }
        }
    }

    // 6. Audit log
    let _ = state.storage.save_audit_entry(&crate::core::AuditEntry {
        id: 0,
        admin_user_id: Some(claims.user_id),
        action: "db_migration".to_string(),
        detail: format!("Migrated database from SQLite to MySQL ({}:{})", body.host, port),
        ip_address: None,
        created_at: chrono::Utc::now(),
        server_id: None,
    }).await;

    info!(mysql = %mysql_url, "Database migration from SQLite to MySQL completed");

    Json(serde_json::json!({
        "status": "ok",
        "message": "Migration complete. Please restart the bot to use the new MySQL database."
    })).into_response()
}

/// Execute the actual data migration from SQLite to MySQL.
async fn do_migrate(
    sqlite: &sqlx::SqlitePool,
    mysql: &sqlx::MySqlPool,
) -> Result<(), String> {
    // Helper to map errors
    let e = |msg: &str, err: sqlx::Error| format!("{}: {}", msg, err);

    // --- groups ---
    let rows = sqlx::query("SELECT id, name, keyword, level, time_add, time_edit FROM `groups`")
        .fetch_all(sqlite).await.map_err(|err| e("Read groups", err))?;
    for row in &rows {
        let id: i64 = row.get("id");
        let name: String = row.get("name");
        let keyword: String = row.get("keyword");
        let level: i32 = row.get("level");
        let time_add: String = row.get("time_add");
        let time_edit: String = row.get("time_edit");
        sqlx::query(
            "INSERT INTO `groups` (id, name, keyword, level, time_add, time_edit) VALUES (?, ?, ?, ?, ?, ?) \
             ON DUPLICATE KEY UPDATE name=VALUES(name), keyword=VALUES(keyword), level=VALUES(level)"
        )
        .bind(id).bind(&name).bind(&keyword).bind(level)
        .bind(&time_add).bind(&time_edit)
        .execute(mysql).await.map_err(|err| e("Write groups", err))?;
    }
    info!(count = rows.len(), "Migrated groups");

    // --- clients ---
    let rows = sqlx::query(
        "SELECT id, guid, pbid, name, ip, greeting, login, password, group_bits, auto_login, auth, \
         time_add, time_edit, last_visit FROM clients"
    ).fetch_all(sqlite).await.map_err(|err| e("Read clients", err))?;
    for row in &rows {
        let id: i64 = row.get("id");
        let guid: String = row.get("guid");
        let pbid: String = row.get("pbid");
        let name: String = row.get("name");
        let ip: Option<String> = row.get("ip");
        let greeting: String = row.get("greeting");
        let login: String = row.get("login");
        let password: String = row.get("password");
        let group_bits: i64 = row.get("group_bits");
        let auto_login: i32 = row.get("auto_login");
        let auth: String = row.get("auth");
        let time_add: String = row.get("time_add");
        let time_edit: String = row.get("time_edit");
        let last_visit: Option<String> = row.get("last_visit");
        sqlx::query(
            "INSERT INTO clients (id, guid, pbid, name, ip, greeting, login, password, group_bits, auto_login, auth, time_add, time_edit, last_visit) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(id).bind(&guid).bind(&pbid).bind(&name).bind(&ip)
        .bind(&greeting).bind(&login).bind(&password)
        .bind(group_bits).bind(auto_login as i8).bind(&auth)
        .bind(&time_add).bind(&time_edit).bind(&last_visit)
        .execute(mysql).await.map_err(|err| e(&format!("Write client id={}", id), err))?;
    }
    info!(count = rows.len(), "Migrated clients");

    // --- aliases ---
    let rows = sqlx::query(
        "SELECT id, client_id, alias, num_used, time_add, time_edit FROM aliases"
    ).fetch_all(sqlite).await.map_err(|err| e("Read aliases", err))?;
    for row in &rows {
        let id: i64 = row.get("id");
        let client_id: i64 = row.get("client_id");
        let alias: String = row.get("alias");
        let num_used: i32 = row.get("num_used");
        let time_add: String = row.get("time_add");
        let time_edit: String = row.get("time_edit");
        sqlx::query(
            "INSERT INTO aliases (id, client_id, alias, num_used, time_add, time_edit) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(id).bind(client_id).bind(&alias).bind(num_used)
        .bind(&time_add).bind(&time_edit)
        .execute(mysql).await.map_err(|err| e("Write aliases", err))?;
    }
    info!(count = rows.len(), "Migrated aliases");

    // --- penalties ---
    let rows = sqlx::query(
        "SELECT id, type, client_id, admin_id, duration, reason, keyword, inactive, time_add, time_edit, time_expire FROM penalties"
    ).fetch_all(sqlite).await.map_err(|err| e("Read penalties", err))?;
    for row in &rows {
        let id: i64 = row.get("id");
        let ptype: String = row.get("type");
        let client_id: i64 = row.get("client_id");
        let admin_id: Option<i64> = row.get("admin_id");
        let duration: Option<i64> = row.get("duration");
        let reason: String = row.get("reason");
        let keyword: String = row.get("keyword");
        let inactive: i32 = row.get("inactive");
        let time_add: String = row.get("time_add");
        let time_edit: String = row.get("time_edit");
        let time_expire: Option<String> = row.get("time_expire");
        sqlx::query(
            "INSERT INTO penalties (id, type, client_id, admin_id, duration, reason, keyword, inactive, time_add, time_edit, time_expire) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(id).bind(&ptype).bind(client_id).bind(admin_id).bind(duration)
        .bind(&reason).bind(&keyword).bind(inactive as i8)
        .bind(&time_add).bind(&time_edit).bind(&time_expire)
        .execute(mysql).await.map_err(|err| e("Write penalties", err))?;
    }
    info!(count = rows.len(), "Migrated penalties");

    // --- admin_users ---
    let rows = sqlx::query(
        "SELECT id, username, password_hash, role, created_at, updated_at FROM admin_users"
    ).fetch_all(sqlite).await.map_err(|err| e("Read admin_users", err))?;
    for row in &rows {
        let id: i64 = row.get("id");
        let username: String = row.get("username");
        let password_hash: String = row.get("password_hash");
        let role: String = row.get("role");
        let created_at: String = row.get("created_at");
        let updated_at: String = row.get("updated_at");
        sqlx::query(
            "INSERT INTO admin_users (id, username, password_hash, role, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(id).bind(&username).bind(&password_hash).bind(&role)
        .bind(&created_at).bind(&updated_at)
        .execute(mysql).await.map_err(|err| e("Write admin_users", err))?;
    }
    info!(count = rows.len(), "Migrated admin_users");

    // --- audit_log ---
    let rows = sqlx::query(
        "SELECT id, admin_user_id, action, detail, ip_address, created_at FROM audit_log"
    ).fetch_all(sqlite).await.map_err(|err| e("Read audit_log", err))?;
    for row in &rows {
        let id: i64 = row.get("id");
        let admin_user_id: Option<i64> = row.get("admin_user_id");
        let action: String = row.get("action");
        let detail: String = row.get("detail");
        let ip_address: Option<String> = row.get("ip_address");
        let created_at: String = row.get("created_at");
        sqlx::query(
            "INSERT INTO audit_log (id, admin_user_id, action, detail, ip_address, created_at) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(id).bind(admin_user_id).bind(&action).bind(&detail)
        .bind(&ip_address).bind(&created_at)
        .execute(mysql).await.map_err(|err| e("Write audit_log", err))?;
    }
    info!(count = rows.len(), "Migrated audit_log");

    // --- chat_messages ---
    let rows = sqlx::query(
        "SELECT id, client_id, client_name, channel, message, time_add FROM chat_messages"
    ).fetch_all(sqlite).await.map_err(|err| e("Read chat_messages", err))?;
    for row in &rows {
        let id: i64 = row.get("id");
        let client_id: i64 = row.get("client_id");
        let client_name: String = row.get("client_name");
        let channel: String = row.get("channel");
        let message: String = row.get("message");
        let time_add: String = row.get("time_add");
        sqlx::query(
            "INSERT INTO chat_messages (id, client_id, client_name, channel, message, time_add) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(id).bind(client_id).bind(&client_name).bind(&channel)
        .bind(&message).bind(&time_add)
        .execute(mysql).await.map_err(|err| e("Write chat_messages", err))?;
    }
    info!(count = rows.len(), "Migrated chat_messages");

    // --- vote_history ---
    let rows = sqlx::query(
        "SELECT id, client_id, client_name, vote_type, vote_data, time_add FROM vote_history"
    ).fetch_all(sqlite).await.map_err(|err| e("Read vote_history", err))?;
    for row in &rows {
        let id: i64 = row.get("id");
        let client_id: i64 = row.get("client_id");
        let client_name: String = row.get("client_name");
        let vote_type: String = row.get("vote_type");
        let vote_data: String = row.get("vote_data");
        let time_add: String = row.get("time_add");
        sqlx::query(
            "INSERT INTO vote_history (id, client_id, client_name, vote_type, vote_data, time_add) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(id).bind(client_id).bind(&client_name).bind(&vote_type)
        .bind(&vote_data).bind(&time_add)
        .execute(mysql).await.map_err(|err| e("Write vote_history", err))?;
    }
    info!(count = rows.len(), "Migrated vote_history");

    // --- admin_notes ---
    let rows = sqlx::query(
        "SELECT id, admin_user_id, content, updated_at FROM admin_notes"
    ).fetch_all(sqlite).await.map_err(|err| e("Read admin_notes", err))?;
    for row in &rows {
        let id: i64 = row.get("id");
        let admin_user_id: i64 = row.get("admin_user_id");
        let content: String = row.get("content");
        let updated_at: String = row.get("updated_at");
        sqlx::query(
            "INSERT INTO admin_notes (id, admin_user_id, content, updated_at) VALUES (?, ?, ?, ?)"
        )
        .bind(id).bind(admin_user_id).bind(&content).bind(&updated_at)
        .execute(mysql).await.map_err(|err| e("Write admin_notes", err))?;
    }
    info!(count = rows.len(), "Migrated admin_notes");

    // --- XLR tables (may not exist if xlrstats plugin was never used) ---

    // xlr_playerstats
    if let Ok(rows) = sqlx::query(
        "SELECT id, client_id, kills, deaths, teamkills, teamdeaths, suicides, ratio, skill, \
         assists, assistskill, curstreak, winstreak, losestreak, rounds, \
         smallestratio, biggestratio, smalleststreak, biggeststreak FROM xlr_playerstats"
    ).fetch_all(sqlite).await {
        for row in &rows {
            sqlx::query(
                "INSERT INTO xlr_playerstats (id, client_id, kills, deaths, teamkills, teamdeaths, suicides, \
                 ratio, skill, assists, assistskill, curstreak, winstreak, losestreak, rounds, \
                 smallestratio, biggestratio, smalleststreak, biggeststreak) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(row.get::<i64, _>("id"))
            .bind(row.get::<i64, _>("client_id"))
            .bind(row.get::<i64, _>("kills"))
            .bind(row.get::<i64, _>("deaths"))
            .bind(row.get::<i64, _>("teamkills"))
            .bind(row.get::<i64, _>("teamdeaths"))
            .bind(row.get::<i64, _>("suicides"))
            .bind(row.get::<f64, _>("ratio"))
            .bind(row.get::<f64, _>("skill"))
            .bind(row.get::<i64, _>("assists"))
            .bind(row.get::<f64, _>("assistskill"))
            .bind(row.get::<i64, _>("curstreak"))
            .bind(row.get::<i64, _>("winstreak"))
            .bind(row.get::<i64, _>("losestreak"))
            .bind(row.get::<i64, _>("rounds"))
            .bind(row.get::<f64, _>("smallestratio"))
            .bind(row.get::<f64, _>("biggestratio"))
            .bind(row.get::<i64, _>("smalleststreak"))
            .bind(row.get::<i64, _>("biggeststreak"))
            .execute(mysql).await.map_err(|err| e("Write xlr_playerstats", err))?;
        }
        info!(count = rows.len(), "Migrated xlr_playerstats");
    }

    // xlr_weaponstats
    if let Ok(rows) = sqlx::query(
        "SELECT id, client_id, name, kills, deaths, teamkills, teamdeaths, suicides, headshots FROM xlr_weaponstats"
    ).fetch_all(sqlite).await {
        for row in &rows {
            sqlx::query(
                "INSERT INTO xlr_weaponstats (id, client_id, name, kills, deaths, teamkills, teamdeaths, suicides, headshots) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(row.get::<i64, _>("id"))
            .bind(row.get::<i64, _>("client_id"))
            .bind(row.get::<String, _>("name"))
            .bind(row.get::<i64, _>("kills"))
            .bind(row.get::<i64, _>("deaths"))
            .bind(row.get::<i64, _>("teamkills"))
            .bind(row.get::<i64, _>("teamdeaths"))
            .bind(row.get::<i64, _>("suicides"))
            .bind(row.get::<i64, _>("headshots"))
            .execute(mysql).await.map_err(|err| e("Write xlr_weaponstats", err))?;
        }
        info!(count = rows.len(), "Migrated xlr_weaponstats");
    }

    // xlr_weaponusage
    if let Ok(rows) = sqlx::query(
        "SELECT id, name, kills, deaths, teamkills, teamdeaths, suicides, headshots FROM xlr_weaponusage"
    ).fetch_all(sqlite).await {
        for row in &rows {
            sqlx::query(
                "INSERT INTO xlr_weaponusage (id, name, kills, deaths, teamkills, teamdeaths, suicides, headshots) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(row.get::<i64, _>("id"))
            .bind(row.get::<String, _>("name"))
            .bind(row.get::<i64, _>("kills"))
            .bind(row.get::<i64, _>("deaths"))
            .bind(row.get::<i64, _>("teamkills"))
            .bind(row.get::<i64, _>("teamdeaths"))
            .bind(row.get::<i64, _>("suicides"))
            .bind(row.get::<i64, _>("headshots"))
            .execute(mysql).await.map_err(|err| e("Write xlr_weaponusage", err))?;
        }
        info!(count = rows.len(), "Migrated xlr_weaponusage");
    }

    // xlr_bodyparts
    if let Ok(rows) = sqlx::query(
        "SELECT id, client_id, name, kills, deaths, teamkills, teamdeaths, suicides FROM xlr_bodyparts"
    ).fetch_all(sqlite).await {
        for row in &rows {
            sqlx::query(
                "INSERT INTO xlr_bodyparts (id, client_id, name, kills, deaths, teamkills, teamdeaths, suicides) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(row.get::<i64, _>("id"))
            .bind(row.get::<i64, _>("client_id"))
            .bind(row.get::<String, _>("name"))
            .bind(row.get::<i64, _>("kills"))
            .bind(row.get::<i64, _>("deaths"))
            .bind(row.get::<i64, _>("teamkills"))
            .bind(row.get::<i64, _>("teamdeaths"))
            .bind(row.get::<i64, _>("suicides"))
            .execute(mysql).await.map_err(|err| e("Write xlr_bodyparts", err))?;
        }
        info!(count = rows.len(), "Migrated xlr_bodyparts");
    }

    // xlr_opponents
    if let Ok(rows) = sqlx::query(
        "SELECT id, client_id, target_id, kills, deaths, retals FROM xlr_opponents"
    ).fetch_all(sqlite).await {
        for row in &rows {
            sqlx::query(
                "INSERT INTO xlr_opponents (id, client_id, target_id, kills, deaths, retals) \
                 VALUES (?, ?, ?, ?, ?, ?)"
            )
            .bind(row.get::<i64, _>("id"))
            .bind(row.get::<i64, _>("client_id"))
            .bind(row.get::<i64, _>("target_id"))
            .bind(row.get::<i64, _>("kills"))
            .bind(row.get::<i64, _>("deaths"))
            .bind(row.get::<i64, _>("retals"))
            .execute(mysql).await.map_err(|err| e("Write xlr_opponents", err))?;
        }
        info!(count = rows.len(), "Migrated xlr_opponents");
    }

    // xlr_mapstats
    if let Ok(rows) = sqlx::query(
        "SELECT id, name, kills, suicides, teamkills, rounds FROM xlr_mapstats"
    ).fetch_all(sqlite).await {
        for row in &rows {
            sqlx::query(
                "INSERT INTO xlr_mapstats (id, name, kills, suicides, teamkills, rounds) \
                 VALUES (?, ?, ?, ?, ?, ?)"
            )
            .bind(row.get::<i64, _>("id"))
            .bind(row.get::<String, _>("name"))
            .bind(row.get::<i64, _>("kills"))
            .bind(row.get::<i64, _>("suicides"))
            .bind(row.get::<i64, _>("teamkills"))
            .bind(row.get::<i64, _>("rounds"))
            .execute(mysql).await.map_err(|err| e("Write xlr_mapstats", err))?;
        }
        info!(count = rows.len(), "Migrated xlr_mapstats");
    }

    // xlr_history
    if let Ok(rows) = sqlx::query(
        "SELECT id, client_id, kills, deaths, skill, time_add FROM xlr_history"
    ).fetch_all(sqlite).await {
        for row in &rows {
            sqlx::query(
                "INSERT INTO xlr_history (id, client_id, kills, deaths, skill, time_add) \
                 VALUES (?, ?, ?, ?, ?, ?)"
            )
            .bind(row.get::<i64, _>("id"))
            .bind(row.get::<i64, _>("client_id"))
            .bind(row.get::<i64, _>("kills"))
            .bind(row.get::<i64, _>("deaths"))
            .bind(row.get::<f64, _>("skill"))
            .bind(row.get::<String, _>("time_add"))
            .execute(mysql).await.map_err(|err| e("Write xlr_history", err))?;
        }
        info!(count = rows.len(), "Migrated xlr_history");
    }

    // --- servers (multi-server registrations) ---
    if let Ok(rows) = sqlx::query(
        "SELECT id, name, address, port, status, current_map, player_count, max_clients, \
         last_seen, config_json, config_version, cert_fingerprint, update_channel, update_interval, \
         created_at, updated_at FROM servers"
    ).fetch_all(sqlite).await {
        for row in &rows {
            let id: i64 = row.get("id");
            let name: String = row.get("name");
            let address: String = row.get("address");
            let port: i64 = row.get("port");
            let status: String = row.get("status");
            let current_map: Option<String> = row.get("current_map");
            let player_count: i64 = row.get("player_count");
            let max_clients: i64 = row.get("max_clients");
            let last_seen: Option<String> = row.get("last_seen");
            let config_json: Option<String> = row.get("config_json");
            let config_version: i64 = row.get("config_version");
            let cert_fingerprint: Option<String> = row.get("cert_fingerprint");
            let update_channel: String = row.try_get("update_channel").unwrap_or_else(|_| "beta".to_string());
            let update_interval: i64 = row.try_get("update_interval").unwrap_or(3600i64);
            let created_at: String = row.get("created_at");
            let updated_at: String = row.get("updated_at");
            sqlx::query(
                "INSERT INTO servers (id, name, address, port, status, current_map, player_count, max_clients, \
                 last_seen, config_json, config_version, cert_fingerprint, update_channel, update_interval, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
                 ON DUPLICATE KEY UPDATE name=VALUES(name), address=VALUES(address), port=VALUES(port), \
                 status=VALUES(status), current_map=VALUES(current_map), player_count=VALUES(player_count), \
                 max_clients=VALUES(max_clients), last_seen=VALUES(last_seen), config_json=VALUES(config_json), \
                 config_version=VALUES(config_version), cert_fingerprint=VALUES(cert_fingerprint), \
                 update_channel=VALUES(update_channel), update_interval=VALUES(update_interval), updated_at=VALUES(updated_at)"
            )
            .bind(id).bind(&name).bind(&address).bind(port).bind(&status)
            .bind(&current_map).bind(player_count).bind(max_clients)
            .bind(&last_seen).bind(&config_json).bind(config_version)
            .bind(&cert_fingerprint).bind(&update_channel).bind(update_interval)
            .bind(&created_at).bind(&updated_at)
            .execute(mysql).await.map_err(|err| e(&format!("Write server id={}", id), err))?;
        }
        info!(count = rows.len(), "Migrated servers");
    }

    // --- map_configs ---
    if let Ok(rows) = sqlx::query(
        "SELECT id, map_name, gametype, capturelimit, timelimit, fraglimit, g_gear, g_gravity, \
         g_friendlyfire, g_followstrict, g_waverespawns, g_bombdefusetime, g_bombexplodetime, \
         g_swaproles, g_maxrounds, g_matchmode, g_respawndelay, startmessage, skiprandom, bot, \
         custom_commands, created_at, updated_at FROM map_configs"
    ).fetch_all(sqlite).await {
        for row in &rows {
            sqlx::query(
                "INSERT INTO map_configs (id, map_name, gametype, capturelimit, timelimit, fraglimit, \
                 g_gear, g_gravity, g_friendlyfire, g_followstrict, g_waverespawns, g_bombdefusetime, \
                 g_bombexplodetime, g_swaproles, g_maxrounds, g_matchmode, g_respawndelay, startmessage, \
                 skiprandom, bot, custom_commands, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
                 ON DUPLICATE KEY UPDATE gametype=VALUES(gametype), capturelimit=VALUES(capturelimit), \
                 timelimit=VALUES(timelimit), fraglimit=VALUES(fraglimit), g_gear=VALUES(g_gear), \
                 g_gravity=VALUES(g_gravity), g_friendlyfire=VALUES(g_friendlyfire), \
                 g_followstrict=VALUES(g_followstrict), g_waverespawns=VALUES(g_waverespawns), \
                 g_bombdefusetime=VALUES(g_bombdefusetime), g_bombexplodetime=VALUES(g_bombexplodetime), \
                 g_swaproles=VALUES(g_swaproles), g_maxrounds=VALUES(g_maxrounds), \
                 g_matchmode=VALUES(g_matchmode), g_respawndelay=VALUES(g_respawndelay), \
                 startmessage=VALUES(startmessage), skiprandom=VALUES(skiprandom), bot=VALUES(bot), \
                 custom_commands=VALUES(custom_commands), updated_at=VALUES(updated_at)"
            )
            .bind(row.get::<i64, _>("id"))
            .bind(row.get::<String, _>("map_name"))
            .bind(row.get::<String, _>("gametype"))
            .bind(row.get::<Option<i64>, _>("capturelimit"))
            .bind(row.get::<Option<i64>, _>("timelimit"))
            .bind(row.get::<Option<i64>, _>("fraglimit"))
            .bind(row.get::<String, _>("g_gear"))
            .bind(row.get::<Option<i64>, _>("g_gravity"))
            .bind(row.get::<Option<i64>, _>("g_friendlyfire"))
            .bind(row.get::<Option<i64>, _>("g_followstrict"))
            .bind(row.get::<Option<i64>, _>("g_waverespawns"))
            .bind(row.get::<Option<i64>, _>("g_bombdefusetime"))
            .bind(row.get::<Option<i64>, _>("g_bombexplodetime"))
            .bind(row.get::<Option<i64>, _>("g_swaproles"))
            .bind(row.get::<Option<i64>, _>("g_maxrounds"))
            .bind(row.get::<Option<i64>, _>("g_matchmode"))
            .bind(row.get::<Option<i64>, _>("g_respawndelay"))
            .bind(row.get::<String, _>("startmessage"))
            .bind(row.get::<i64, _>("skiprandom"))
            .bind(row.get::<i64, _>("bot"))
            .bind(row.get::<String, _>("custom_commands"))
            .bind(row.get::<String, _>("created_at"))
            .bind(row.get::<String, _>("updated_at"))
            .execute(mysql).await.map_err(|err| e("Write map_configs", err))?;
        }
        info!(count = rows.len(), "Migrated map_configs");
    }

    Ok(())
}

// ---- Server Config Analyzer ----

#[derive(Deserialize)]
pub struct ServerCfgRequest {
    pub path: String,
}

/// POST /api/v1/config/server-cfg — read and analyze a UrT server.cfg file.
pub async fn analyze_server_cfg(
    AdminOnly(_claims): AdminOnly,
    State(state): State<AppState>,
    Json(body): Json<ServerCfgRequest>,
) -> impl IntoResponse {
    let path = std::path::Path::new(&body.path);

    // Security: only allow reading .cfg files to prevent arbitrary file reads
    match path.extension().and_then(|e| e.to_str()) {
        Some("cfg") => {}
        _ => {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": "Only .cfg files can be read."
            }))).into_response();
        }
    }

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": format!("Cannot read file: {}", e)
            }))).into_response();
        }
    };

    // Parse all "set <key> <value>" and "seta <key> <value>" lines
    let mut settings: Vec<serde_json::Value> = Vec::new();
    let mut commands: Vec<String> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }

        // Match: set/seta key value  or  set/seta key "value"
        if trimmed.starts_with("set ") || trimmed.starts_with("seta ") {
            let rest = if trimmed.starts_with("seta ") {
                &trimmed[5..]
            } else {
                &trimmed[4..]
            };
            let rest = rest.trim();
            if let Some((key, val_raw)) = rest.split_once(char::is_whitespace) {
                let val = val_raw.trim().trim_matches('"');
                settings.push(serde_json::json!({
                    "key": key,
                    "value": val,
                }));
            }
        } else {
            // Non-set commands like "vstr d1", "map ut4_casa", etc.
            commands.push(trimmed.to_string());
        }
    }

    // Build a lookup map for checks
    let setting_map: std::collections::HashMap<&str, &str> = settings.iter()
        .filter_map(|s| {
            let k = s.get("key")?.as_str()?;
            let v = s.get("value")?.as_str()?;
            Some((k, v))
        })
        .collect();

    // Run health checks comparing server.cfg against bot requirements
    let mut checks: Vec<serde_json::Value> = Vec::new();

    // 1. g_log — must be set
    match setting_map.get("g_log") {
        Some(v) if !v.is_empty() => {
            checks.push(serde_json::json!({
                "key": "g_log", "status": "ok",
                "message": format!("Game log enabled: \"{}\"", v),
            }));
        }
        _ => {
            checks.push(serde_json::json!({
                "key": "g_log", "status": "error",
                "message": "g_log is not set. The bot requires game logging to be enabled.",
                "fix_key": "g_log", "fix_value": "games.log",
            }));
        }
    }

    // 2. g_logsync — must be 1
    match setting_map.get("g_logsync") {
        Some(&"1") => {
            checks.push(serde_json::json!({
                "key": "g_logsync", "status": "ok",
                "message": "Log sync is enabled (writes flushed immediately).",
            }));
        }
        Some(v) => {
            checks.push(serde_json::json!({
                "key": "g_logsync", "status": "error",
                "message": format!("g_logsync is \"{}\" but must be \"1\" for the bot to read events in real time.", v),
                "fix_key": "g_logsync", "fix_value": "1",
            }));
        }
        None => {
            checks.push(serde_json::json!({
                "key": "g_logsync", "status": "error",
                "message": "g_logsync is not set. Must be \"1\" for real-time log reading.",
                "fix_key": "g_logsync", "fix_value": "1",
            }));
        }
    }

    // 3. g_logroll — should be 0 (prevent log rotation mid-session)
    match setting_map.get("g_logroll") {
        Some(&"0") | None => {
            checks.push(serde_json::json!({
                "key": "g_logroll", "status": "ok",
                "message": "Log roll is disabled (recommended).",
            }));
        }
        Some(v) => {
            checks.push(serde_json::json!({
                "key": "g_logroll", "status": "warning",
                "message": format!("g_logroll is \"{}\". Recommend \"0\" to prevent log rotation issues.", v),
                "fix_key": "g_logroll", "fix_value": "0",
            }));
        }
    }

    // 4. logfile — should be 2 for full logging
    match setting_map.get("logfile") {
        Some(&"2") => {
            checks.push(serde_json::json!({
                "key": "logfile", "status": "ok",
                "message": "Console logfile level is 2 (full logging).",
            }));
        }
        Some(v) => {
            checks.push(serde_json::json!({
                "key": "logfile", "status": "warning",
                "message": format!("logfile is \"{}\". Recommend \"2\" for full console logging.", v),
                "fix_key": "logfile", "fix_value": "2",
            }));
        }
        None => {
            checks.push(serde_json::json!({
                "key": "logfile", "status": "warning",
                "message": "logfile not set. Recommend \"2\" for full console logging.",
                "fix_key": "logfile", "fix_value": "2",
            }));
        }
    }

    // 5. rconPassword / sv_rconPassword — must match bot config
    let rcon_set = setting_map.get("rconPassword").or_else(|| setting_map.get("sv_rconPassword"));
    match rcon_set {
        Some(v) if !v.is_empty() => {
            if *v == state.config.server.rcon_password {
                checks.push(serde_json::json!({
                    "key": "rconPassword", "status": "ok",
                    "message": "RCON password matches bot configuration.",
                }));
            } else {
                checks.push(serde_json::json!({
                    "key": "rconPassword", "status": "error",
                    "message": "RCON password does NOT match bot configuration. The bot cannot send server commands.",
                }));
            }
        }
        _ => {
            checks.push(serde_json::json!({
                "key": "rconPassword", "status": "error",
                "message": "No RCON password set. The bot requires RCON to manage the server.",
                "fix_key": "rconPassword", "fix_value": "",
            }));
        }
    }

    // 6. sv_strictAuth — recommended for auth tracking
    match setting_map.get("sv_strictAuth") {
        Some(&"1") => {
            checks.push(serde_json::json!({
                "key": "sv_strictAuth", "status": "ok",
                "message": "Strict auth is enabled. Player auth names will be tracked.",
            }));
        }
        Some(v) => {
            checks.push(serde_json::json!({
                "key": "sv_strictAuth", "status": "warning",
                "message": format!("sv_strictAuth is \"{}\". Recommend \"1\" to enable player auth tracking.", v),
                "fix_key": "sv_strictAuth", "fix_value": "1",
            }));
        }
        None => {
            checks.push(serde_json::json!({
                "key": "sv_strictAuth", "status": "warning",
                "message": "sv_strictAuth not set. Recommend \"1\" for player auth tracking.",
                "fix_key": "sv_strictAuth", "fix_value": "1",
            }));
        }
    }

    // 7. net_port — should match bot's configured port
    if let Some(port_str) = setting_map.get("net_port") {
        if let Ok(port) = port_str.parse::<u16>() {
            if port == state.config.server.port {
                checks.push(serde_json::json!({
                    "key": "net_port", "status": "ok",
                    "message": format!("Server port {} matches bot configuration.", port),
                }));
            } else {
                checks.push(serde_json::json!({
                    "key": "net_port", "status": "error",
                    "message": format!("Server port is {} but bot is configured for port {}.", port, state.config.server.port),
                }));
            }
        }
    }

    // 8. g_gametype info
    if let Some(gt) = setting_map.get("g_gametype") {
        let gt_name = match *gt {
            "0" => "Free For All",
            "1" => "Last Man Standing",
            "3" => "Team Death Match",
            "4" => "Team Survivor",
            "5" => "Follow the Leader",
            "6" => "Capture and Hold",
            "7" => "Capture the Flag",
            "8" => "Bomb Mode",
            "9" => "Jump Mode",
            "10" => "Freeze Tag",
            "11" => "Gun Game",
            _ => "Unknown",
        };
        checks.push(serde_json::json!({
            "key": "g_gametype", "status": "info",
            "message": format!("Game type: {} ({})", gt_name, gt),
        }));
    }

    // Collect map rotation
    let mut map_rotation: Vec<String> = Vec::new();
    for s in &settings {
        if let (Some(key), Some(val)) = (s.get("key").and_then(|k| k.as_str()), s.get("value").and_then(|v| v.as_str())) {
            if val.starts_with("map ") || val.starts_with("ut4_") {
                let map_name = val.strip_prefix("map ").unwrap_or(val);
                map_rotation.push(map_name.to_string());
            }
            // Also handle "set d1 "map ut4_name; set d2..."
            if key.starts_with('d') && key[1..].parse::<u32>().is_ok() && val.contains("map ") {
                // Already captured above
            }
        }
    }

    Json(serde_json::json!({
        "settings": settings,
        "commands": commands,
        "checks": checks,
        "map_rotation": map_rotation,
        "raw": content,
    })).into_response()
}

/// POST /api/v1/config/server-cfg/save — write changes back to the server.cfg file.
pub async fn save_server_cfg(
    AdminOnly(claims): AdminOnly,
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let path_str = match body.get("path").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": "Missing 'path' field."
            }))).into_response();
        }
    };

    let path = std::path::Path::new(path_str);
    match path.extension().and_then(|e| e.to_str()) {
        Some("cfg") => {}
        _ => {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": "Only .cfg files can be written."
            }))).into_response();
        }
    }

    // Must be an existing file (no creating arbitrary files)
    if !path.exists() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": "File does not exist."
        }))).into_response();
    }

    let content = match body.get("content").and_then(|v| v.as_str()) {
        Some(c) => c,
        None => {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": "Missing 'content' field."
            }))).into_response();
        }
    };

    if let Err(e) = std::fs::write(path, content) {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "error": format!("Cannot write file: {}", e)
        }))).into_response();
    }

    // Audit log
    let _ = state.storage.save_audit_entry(&crate::core::AuditEntry {
        id: 0,
        admin_user_id: Some(claims.user_id),
        action: "server_cfg_update".to_string(),
        detail: format!("Updated server config: {}", path_str),
        ip_address: None,
        created_at: chrono::Utc::now(),
        server_id: None,
    }).await;

    Json(serde_json::json!({
        "status": "ok",
        "message": "Server config saved. Restart the game server for changes to take effect."
    })).into_response()
}

/// POST /api/v1/config/browse — browse the server's filesystem for .cfg files.
pub async fn browse_files(
    AdminOnly(_claims): AdminOnly,
    State(_state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let path_str = match body.get("path").and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => "/".to_string(),
    };

    let path = std::path::Path::new(&path_str);

    // Must be absolute path
    if !path.is_absolute() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": "Path must be absolute."
        }))).into_response();
    }

    // Canonicalize to resolve symlinks and block .. traversal
    let canonical = match path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": format!("Invalid path: {}", e)
            }))).into_response();
        }
    };

    if !canonical.is_dir() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": "Path is not a directory."
        }))).into_response();
    }

    let dir = match std::fs::read_dir(&canonical) {
        Ok(d) => d,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                "error": format!("Cannot read directory: {}", e)
            }))).into_response();
        }
    };

    let mut entries: Vec<serde_json::Value> = Vec::new();
    for entry in dir.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        // Skip hidden files/dirs
        if name.starts_with('.') {
            continue;
        }
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let is_dir = meta.is_dir();
        // Show directories and .cfg files only
        if is_dir || name.ends_with(".cfg") {
            entries.push(serde_json::json!({
                "name": name,
                "is_dir": is_dir,
                "size": if is_dir { 0 } else { meta.len() },
            }));
        }
    }

    // Sort: directories first, then files, alphabetically within each group
    entries.sort_by(|a, b| {
        let a_dir = a["is_dir"].as_bool().unwrap_or(false);
        let b_dir = b["is_dir"].as_bool().unwrap_or(false);
        match (a_dir, b_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                let a_name = a["name"].as_str().unwrap_or("");
                let b_name = b["name"].as_str().unwrap_or("");
                a_name.to_lowercase().cmp(&b_name.to_lowercase())
            }
        }
    });

    let parent = canonical.parent().map(|p| p.to_string_lossy().to_string());

    Json(serde_json::json!({
        "path": canonical.to_string_lossy(),
        "parent": parent,
        "entries": entries,
    })).into_response()
}

/// POST /api/v1/config/check-game-log — verify the given path on the local
/// filesystem (for standalone or master's own embedded server). Delegates to
/// the same routine the client bots use so feedback is consistent.
pub async fn check_game_log(
    AdminOnly(_claims): AdminOnly,
    State(_state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let path = body
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let resp = crate::sync::handlers::handle_check_game_log(&path).await;
    Json(serde_json::to_value(&resp).unwrap_or_default()).into_response()
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
