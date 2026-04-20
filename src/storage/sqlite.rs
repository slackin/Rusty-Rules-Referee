use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions, SqliteRow};
use sqlx::Row;
use tracing::info;

use crate::core::{Alias, AdminNote, AdminUser, AuditEntry, ChatMessage, Client, DashboardSummary, GameServer, Group, MapConfig, Penalty, PenaltyType, SyncQueueEntry, VoteRecord};
use crate::storage::{Storage, StorageError, StorageProtocol};

pub struct SqliteStorage {
    pool: SqlitePool,
}

impl SqliteStorage {
    pub async fn new(database_url: &str) -> Result<Self, StorageError> {
        let url = if database_url.starts_with("sqlite://") || database_url.starts_with("sqlite:") {
            database_url.to_string()
        } else {
            format!("sqlite://{}", database_url)
        };

        // For in-memory databases, limit to 1 connection so all queries
        // share the same database instance.
        let max_conn = if url.contains(":memory:") { 1 } else { 5 };

        let connect_options: SqliteConnectOptions = url
            .parse::<SqliteConnectOptions>()
            .map_err(|e| StorageError::ConnectionFailed(e.to_string()))?
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(max_conn)
            .connect_with(connect_options)
            .await
            .map_err(|e| StorageError::ConnectionFailed(e.to_string()))?;

        let storage = Self { pool };
        storage.run_migrations().await?;
        info!(url = %url, "SQLite storage connected");
        Ok(storage)
    }

    async fn run_migrations(&self) -> Result<(), StorageError> {
        let migrations = [
            include_str!("../../migrations/001_initial.sql"),
            include_str!("../../migrations/003_admin_users.sql"),
            include_str!("../../migrations/004_dashboard_features.sql"),
            include_str!("../../migrations/005_auth_column.sql"),
            include_str!("../../migrations/007_map_configs.sql"),
        ];
        for schema in migrations {
            // Strip SQL comment lines before splitting into statements
            let cleaned: String = schema
                .lines()
                .filter(|line| !line.trim_start().starts_with("--"))
                .collect::<Vec<_>>()
                .join("\n");
            for statement in cleaned.split(';') {
                let trimmed = statement.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let result = sqlx::query(trimmed)
                    .execute(&self.pool)
                    .await;
                match result {
                    Ok(_) => {},
                    Err(e) => {
                        // Ignore "duplicate column" errors from ALTER TABLE
                        let msg = e.to_string();
                        if msg.contains("duplicate column") {
                            continue;
                        }
                        return Err(StorageError::QueryFailed(format!("Migration error: {}", e)));
                    }
                }
            }
        }
        info!("SQLite migrations complete");
        Ok(())
    }
}

fn penalty_type_to_str(pt: PenaltyType) -> &'static str {
    match pt {
        PenaltyType::Warning => "Warning",
        PenaltyType::Notice => "Notice",
        PenaltyType::Kick => "Kick",
        PenaltyType::Ban => "Ban",
        PenaltyType::TempBan => "TempBan",
        PenaltyType::Mute => "Mute",
    }
}

fn str_to_penalty_type(s: &str) -> PenaltyType {
    match s {
        "Warning" => PenaltyType::Warning,
        "Notice" => PenaltyType::Notice,
        "Kick" => PenaltyType::Kick,
        "Ban" => PenaltyType::Ban,
        "TempBan" => PenaltyType::TempBan,
        "Mute" => PenaltyType::Mute,
        _ => PenaltyType::Warning,
    }
}

fn parse_dt(s: &str) -> DateTime<Utc> {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .map(|ndt| ndt.and_utc())
        .unwrap_or_default()
}

fn row_to_client(row: &SqliteRow) -> Client {
    let mut client = Client::new(
        row.get::<String, _>("guid").as_str(),
        row.get::<String, _>("name").as_str(),
    );
    client.id = row.get("id");
    client.pbid = row.get("pbid");
    client.greeting = row.get("greeting");
    client.login = row.get("login");
    client.password = row.get("password");
    client.group_bits = row.get::<i64, _>("group_bits") as u64;
    client.auto_login = row.get::<i32, _>("auto_login") != 0;
    client.auth = row.get::<Option<String>, _>("auth").unwrap_or_default();
    client.time_add = parse_dt(row.get("time_add"));
    client.time_edit = parse_dt(row.get("time_edit"));

    let ip_str: Option<String> = row.get("ip");
    client.ip = ip_str.and_then(|s| s.parse().ok());

    let lv: Option<String> = row.get("last_visit");
    client.last_visit = lv.map(|s| parse_dt(&s));

    client
}

fn row_to_penalty(row: &SqliteRow) -> Penalty {
    let te: Option<String> = row.get("time_expire");
    Penalty {
        id: row.get("id"),
        penalty_type: str_to_penalty_type(row.get("type")),
        client_id: row.get("client_id"),
        admin_id: row.get("admin_id"),
        duration: row.get("duration"),
        reason: row.get("reason"),
        keyword: row.get("keyword"),
        inactive: row.get::<i32, _>("inactive") != 0,
        time_add: parse_dt(row.get("time_add")),
        time_edit: parse_dt(row.get("time_edit")),
        time_expire: te.map(|s| parse_dt(&s)),
    }
}

fn row_to_group(row: &SqliteRow) -> Group {
    Group {
        id: row.get::<i64, _>("id") as u64,
        name: row.get("name"),
        keyword: row.get("keyword"),
        level: row.get::<i32, _>("level") as u32,
        time_add: parse_dt(row.get("time_add")),
        time_edit: parse_dt(row.get("time_edit")),
    }
}

fn row_to_alias(row: &SqliteRow) -> Alias {
    Alias {
        id: row.get("id"),
        client_id: row.get("client_id"),
        alias: row.get("alias"),
        num_used: row.get::<i32, _>("num_used") as u32,
        time_add: parse_dt(row.get("time_add")),
        time_edit: parse_dt(row.get("time_edit")),
    }
}

fn row_to_admin_user(row: &SqliteRow) -> AdminUser {
    AdminUser {
        id: row.get("id"),
        username: row.get("username"),
        password_hash: row.get("password_hash"),
        role: row.get("role"),
        created_at: parse_dt(row.get("created_at")),
        updated_at: parse_dt(row.get("updated_at")),
    }
}

fn row_to_audit_entry(row: &SqliteRow) -> AuditEntry {
    AuditEntry {
        id: row.get("id"),
        admin_user_id: row.get("admin_user_id"),
        action: row.get("action"),
        detail: row.get("detail"),
        ip_address: row.get("ip_address"),
        created_at: parse_dt(row.get("created_at")),
    }
}

fn row_to_chat_message(row: &SqliteRow) -> ChatMessage {
    ChatMessage {
        id: row.get("id"),
        client_id: row.get("client_id"),
        client_name: row.get("client_name"),
        channel: row.get("channel"),
        message: row.get("message"),
        time_add: parse_dt(row.get("time_add")),
    }
}

fn row_to_vote_record(row: &SqliteRow) -> VoteRecord {
    VoteRecord {
        id: row.get("id"),
        client_id: row.get("client_id"),
        client_name: row.get("client_name"),
        vote_type: row.get("vote_type"),
        vote_data: row.get("vote_data"),
        time_add: parse_dt(row.get("time_add")),
    }
}

fn row_to_admin_note(row: &SqliteRow) -> AdminNote {
    AdminNote {
        id: row.get("id"),
        admin_user_id: row.get("admin_user_id"),
        content: row.get("content"),
        updated_at: parse_dt(row.get("updated_at")),
    }
}

fn row_to_map_config(row: &SqliteRow) -> MapConfig {
    MapConfig {
        id: row.get("id"),
        map_name: row.get("map_name"),
        gametype: row.get("gametype"),
        capturelimit: row.get("capturelimit"),
        timelimit: row.get("timelimit"),
        fraglimit: row.get("fraglimit"),
        g_gear: row.get("g_gear"),
        g_gravity: row.get("g_gravity"),
        g_friendlyfire: row.get("g_friendlyfire"),
        g_followstrict: row.get("g_followstrict"),
        g_waverespawns: row.get("g_waverespawns"),
        g_bombdefusetime: row.get("g_bombdefusetime"),
        g_bombexplodetime: row.get("g_bombexplodetime"),
        g_swaproles: row.get("g_swaproles"),
        g_maxrounds: row.get("g_maxrounds"),
        g_matchmode: row.get("g_matchmode"),
        g_respawndelay: row.get("g_respawndelay"),
        startmessage: row.get("startmessage"),
        skiprandom: row.get("skiprandom"),
        bot: row.get("bot"),
        custom_commands: row.get("custom_commands"),
        created_at: parse_dt(row.get("created_at")),
        updated_at: parse_dt(row.get("updated_at")),
    }
}

#[async_trait]
impl Storage for SqliteStorage {
    fn protocol(&self) -> StorageProtocol {
        StorageProtocol::Sqlite
    }

    async fn connect(&mut self) -> Result<(), StorageError> {
        Ok(()) // already connected via pool
    }

    async fn shutdown(&mut self) -> Result<(), StorageError> {
        self.pool.close().await;
        Ok(())
    }

    async fn get_client(&self, client_id: i64) -> Result<Client, StorageError> {
        sqlx::query("SELECT * FROM clients WHERE id = ?")
            .bind(client_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?
            .map(|row| row_to_client(&row))
            .ok_or(StorageError::NotFound)
    }

    async fn get_client_by_guid(&self, guid: &str) -> Result<Client, StorageError> {
        sqlx::query("SELECT * FROM clients WHERE guid = ?")
            .bind(guid)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?
            .map(|row| row_to_client(&row))
            .ok_or(StorageError::NotFound)
    }

    async fn find_clients(&self, query: &str) -> Result<Vec<Client>, StorageError> {
        let pattern = format!("%{}%", query);
        let rows = sqlx::query("SELECT * FROM clients WHERE name LIKE ? OR guid LIKE ? LIMIT 50")
            .bind(&pattern)
            .bind(&pattern)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(rows.iter().map(row_to_client).collect())
    }

    async fn list_clients(&self, limit: u32, offset: u32, search: Option<&str>, sort_by: &str, order: &str) -> Result<(Vec<Client>, u64), StorageError> {
        let sort_col = match sort_by {
            "name" => "name",
            "time_add" => "time_add",
            "id" => "id",
            _ => "COALESCE(last_visit, '1970-01-01')",
        };
        let sort_dir = if order.eq_ignore_ascii_case("asc") { "ASC" } else { "DESC" };

        let (rows, total) = if let Some(q) = search.filter(|s| !s.is_empty()) {
            let pattern = format!("%{}%", q);
            let count_sql = "SELECT COUNT(*) as cnt FROM clients WHERE name LIKE ? OR guid LIKE ? OR ip LIKE ?";
            let total: i64 = sqlx::query_scalar(count_sql)
                .bind(&pattern).bind(&pattern).bind(&pattern)
                .fetch_one(&self.pool).await
                .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
            let query_sql = format!(
                "SELECT * FROM clients WHERE name LIKE ? OR guid LIKE ? OR ip LIKE ? ORDER BY {} {} LIMIT ? OFFSET ?",
                sort_col, sort_dir
            );
            let rows = sqlx::query(&query_sql)
                .bind(&pattern).bind(&pattern).bind(&pattern)
                .bind(limit as i64).bind(offset as i64)
                .fetch_all(&self.pool).await
                .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
            (rows, total as u64)
        } else {
            let total: i64 = sqlx::query_scalar("SELECT COUNT(*) as cnt FROM clients")
                .fetch_one(&self.pool).await
                .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
            let query_sql = format!(
                "SELECT * FROM clients ORDER BY {} {} LIMIT ? OFFSET ?",
                sort_col, sort_dir
            );
            let rows = sqlx::query(&query_sql)
                .bind(limit as i64).bind(offset as i64)
                .fetch_all(&self.pool).await
                .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
            (rows, total as u64)
        };

        Ok((rows.iter().map(row_to_client).collect(), total))
    }

    async fn save_client(&self, client: &Client) -> Result<i64, StorageError> {
        let ip_str = client.ip.map(|ip| ip.to_string());
        let last_visit_str = client.last_visit.map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string());
        let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        if client.id > 0 {
            // Update existing
            sqlx::query(
                "UPDATE clients SET name = ?, ip = ?, greeting = ?, login = ?, password = ?, \
                 group_bits = ?, auto_login = ?, time_edit = ?, last_visit = ?, pbid = ?, auth = ? \
                 WHERE id = ?"
            )
            .bind(&client.name)
            .bind(&ip_str)
            .bind(&client.greeting)
            .bind(&client.login)
            .bind(&client.password)
            .bind(client.group_bits as i64)
            .bind(client.auto_login as i32)
            .bind(&now)
            .bind(&last_visit_str)
            .bind(&client.pbid)
            .bind(&client.auth)
            .bind(client.id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
            Ok(client.id)
        } else {
            // Insert new
            let result = sqlx::query(
                "INSERT INTO clients (guid, pbid, name, ip, greeting, login, password, \
                 group_bits, auto_login, time_add, time_edit, last_visit, auth) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&client.guid)
            .bind(&client.pbid)
            .bind(&client.name)
            .bind(&ip_str)
            .bind(&client.greeting)
            .bind(&client.login)
            .bind(&client.password)
            .bind(client.group_bits as i64)
            .bind(client.auto_login as i32)
            .bind(&now)
            .bind(&now)
            .bind(&last_visit_str)
            .bind(&client.auth)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
            Ok(result.last_insert_rowid())
        }
    }

    async fn get_penalties(
        &self,
        client_id: i64,
        penalty_type: Option<PenaltyType>,
    ) -> Result<Vec<Penalty>, StorageError> {
        let rows = if let Some(pt) = penalty_type {
            sqlx::query(
                "SELECT * FROM penalties WHERE client_id = ? AND type = ? ORDER BY time_add DESC"
            )
            .bind(client_id)
            .bind(penalty_type_to_str(pt))
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query("SELECT * FROM penalties WHERE client_id = ? ORDER BY time_add DESC")
                .bind(client_id)
                .fetch_all(&self.pool)
                .await
        }
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;

        Ok(rows.iter().map(row_to_penalty).collect())
    }

    async fn save_penalty(&self, penalty: &Penalty) -> Result<i64, StorageError> {
        let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let expire_str = penalty.time_expire.map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string());

        let result = sqlx::query(
            "INSERT INTO penalties (type, client_id, admin_id, duration, reason, keyword, \
             inactive, time_add, time_edit, time_expire) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(penalty_type_to_str(penalty.penalty_type))
        .bind(penalty.client_id)
        .bind(penalty.admin_id)
        .bind(penalty.duration)
        .bind(&penalty.reason)
        .bind(&penalty.keyword)
        .bind(penalty.inactive as i32)
        .bind(&now)
        .bind(&now)
        .bind(&expire_str)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;

        Ok(result.last_insert_rowid())
    }

    async fn disable_penalties(
        &self,
        client_id: i64,
        penalty_type: PenaltyType,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE penalties SET inactive = 1 WHERE client_id = ? AND type = ? AND inactive = 0")
            .bind(client_id)
            .bind(penalty_type_to_str(penalty_type))
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(())
    }

    async fn get_last_penalty(
        &self,
        client_id: i64,
        penalty_type: PenaltyType,
    ) -> Result<Option<Penalty>, StorageError> {
        let row = sqlx::query(
            "SELECT * FROM penalties WHERE client_id = ? AND type = ? ORDER BY time_add DESC LIMIT 1"
        )
        .bind(client_id)
        .bind(penalty_type_to_str(penalty_type))
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;

        Ok(row.map(|r| row_to_penalty(&r)))
    }

    async fn count_penalties(
        &self,
        client_id: i64,
        penalty_type: PenaltyType,
    ) -> Result<u64, StorageError> {
        let row = sqlx::query(
            "SELECT COUNT(*) as cnt FROM penalties WHERE client_id = ? AND type = ? AND inactive = 0"
        )
        .bind(client_id)
        .bind(penalty_type_to_str(penalty_type))
        .fetch_one(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;

        Ok(row.get::<i64, _>("cnt") as u64)
    }

    async fn get_groups(&self) -> Result<Vec<Group>, StorageError> {
        let rows = sqlx::query("SELECT * FROM groups ORDER BY level ASC")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(rows.iter().map(row_to_group).collect())
    }

    async fn get_group(&self, group_id: u64) -> Result<Group, StorageError> {
        sqlx::query("SELECT * FROM groups WHERE id = ?")
            .bind(group_id as i64)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?
            .map(|row| row_to_group(&row))
            .ok_or(StorageError::NotFound)
    }

    async fn get_tables(&self) -> Result<Vec<String>, StorageError> {
        let rows = sqlx::query("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(rows.iter().map(|r| r.get("name")).collect())
    }

    async fn save_alias(&self, client_id: i64, alias: &str) -> Result<(), StorageError> {
        let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        // Try to increment existing alias
        let result = sqlx::query(
            "UPDATE aliases SET num_used = num_used + 1, time_edit = ? \
             WHERE client_id = ? AND alias = ?"
        )
        .bind(&now)
        .bind(client_id)
        .bind(alias)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;

        if result.rows_affected() == 0 {
            // Insert new alias
            sqlx::query(
                "INSERT INTO aliases (client_id, alias, num_used, time_add, time_edit) \
                 VALUES (?, ?, 1, ?, ?)"
            )
            .bind(client_id)
            .bind(alias)
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        }
        Ok(())
    }

    async fn get_aliases(&self, client_id: i64) -> Result<Vec<Alias>, StorageError> {
        let rows = sqlx::query(
            "SELECT * FROM aliases WHERE client_id = ? ORDER BY num_used DESC"
        )
        .bind(client_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(rows.iter().map(row_to_alias).collect())
    }

    async fn find_clients_by_alias(&self, query: &str) -> Result<Vec<Client>, StorageError> {
        let pattern = format!("%{}%", query);
        let rows = sqlx::query(
            "SELECT DISTINCT c.* FROM clients c \
             INNER JOIN aliases a ON c.id = a.client_id \
             WHERE a.alias LIKE ? LIMIT 50"
        )
        .bind(&pattern)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(rows.iter().map(row_to_client).collect())
    }

    async fn get_last_bans(&self, limit: u32) -> Result<Vec<Penalty>, StorageError> {
        let rows = sqlx::query(
            "SELECT * FROM penalties WHERE type IN ('Ban', 'TempBan') \
             ORDER BY time_add DESC LIMIT ?"
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(rows.iter().map(row_to_penalty).collect())
    }

    async fn disable_last_penalty(&self, client_id: i64, penalty_type: PenaltyType) -> Result<bool, StorageError> {
        let row = sqlx::query(
            "SELECT id FROM penalties WHERE client_id = ? AND type = ? AND inactive = 0 \
             ORDER BY time_add DESC LIMIT 1"
        )
        .bind(client_id)
        .bind(penalty_type_to_str(penalty_type))
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;

        if let Some(row) = row {
            let id: i64 = row.get("id");
            sqlx::query("UPDATE penalties SET inactive = 1 WHERE id = ?")
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn disable_all_penalties_of_type(&self, client_id: i64, penalty_type: PenaltyType) -> Result<u64, StorageError> {
        let result = sqlx::query(
            "UPDATE penalties SET inactive = 1 WHERE client_id = ? AND type = ? AND inactive = 0"
        )
        .bind(client_id)
        .bind(penalty_type_to_str(penalty_type))
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(result.rows_affected())
    }

    async fn get_client_count_by_level(&self, min_level: u32) -> Result<u64, StorageError> {
        // group_bits encodes level as bit position, so level N means bit N is set
        // For a simple check, count clients where group_bits >= (1 << min_level)
        let min_bits = if min_level == 0 { 0i64 } else { 1i64 << min_level };
        let row = sqlx::query(
            "SELECT COUNT(*) as cnt FROM clients WHERE group_bits >= ?"
        )
        .bind(min_bits)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(row.get::<i64, _>("cnt") as u64)
    }

    // ---- Admin user operations ----

    async fn get_admin_user(&self, username: &str) -> Result<AdminUser, StorageError> {
        sqlx::query("SELECT * FROM admin_users WHERE username = ?")
            .bind(username)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?
            .map(|row| row_to_admin_user(&row))
            .ok_or(StorageError::NotFound)
    }

    async fn get_admin_user_by_id(&self, id: i64) -> Result<AdminUser, StorageError> {
        sqlx::query("SELECT * FROM admin_users WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?
            .map(|row| row_to_admin_user(&row))
            .ok_or(StorageError::NotFound)
    }

    async fn get_admin_users(&self) -> Result<Vec<AdminUser>, StorageError> {
        let rows = sqlx::query("SELECT * FROM admin_users ORDER BY id ASC")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(rows.iter().map(row_to_admin_user).collect())
    }

    async fn save_admin_user(&self, user: &AdminUser) -> Result<i64, StorageError> {
        let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        if user.id > 0 {
            sqlx::query(
                "UPDATE admin_users SET username = ?, password_hash = ?, role = ?, updated_at = ? WHERE id = ?"
            )
            .bind(&user.username)
            .bind(&user.password_hash)
            .bind(&user.role)
            .bind(&now)
            .bind(user.id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
            Ok(user.id)
        } else {
            let result = sqlx::query(
                "INSERT INTO admin_users (username, password_hash, role, created_at, updated_at) VALUES (?, ?, ?, ?, ?)"
            )
            .bind(&user.username)
            .bind(&user.password_hash)
            .bind(&user.role)
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
            Ok(result.last_insert_rowid())
        }
    }

    async fn delete_admin_user(&self, id: i64) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM admin_users WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(())
    }

    // ---- Audit log ----

    async fn save_audit_entry(&self, entry: &AuditEntry) -> Result<i64, StorageError> {
        let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let result = sqlx::query(
            "INSERT INTO audit_log (admin_user_id, action, detail, ip_address, created_at) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(entry.admin_user_id)
        .bind(&entry.action)
        .bind(&entry.detail)
        .bind(&entry.ip_address)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(result.last_insert_rowid())
    }

    async fn get_audit_log(&self, limit: u32, offset: u32) -> Result<Vec<AuditEntry>, StorageError> {
        let rows = sqlx::query(
            "SELECT * FROM audit_log ORDER BY created_at DESC LIMIT ? OFFSET ?"
        )
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(rows.iter().map(row_to_audit_entry).collect())
    }

    // ---- XLR stats queries ----

    async fn get_xlr_leaderboard(&self, limit: u32, offset: u32) -> Result<Vec<serde_json::Value>, StorageError> {
        let rows = sqlx::query(
            "SELECT s.*, c.name, c.guid FROM xlr_playerstats s \
             INNER JOIN clients c ON s.client_id = c.id \
             WHERE s.kills >= 10 \
             ORDER BY s.skill DESC LIMIT ? OFFSET ?"
        )
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        Ok(rows.iter().map(|row| {
            serde_json::json!({
                "client_id": row.get::<i64, _>("client_id"),
                "name": row.get::<String, _>("name"),
                "kills": row.get::<i64, _>("kills"),
                "deaths": row.get::<i64, _>("deaths"),
                "teamkills": row.get::<i64, _>("teamkills"),
                "teamdeaths": row.get::<i64, _>("teamdeaths"),
                "suicides": row.get::<i64, _>("suicides"),
                "ratio": row.get::<f64, _>("ratio"),
                "skill": row.get::<f64, _>("skill"),
                "assists": row.get::<i64, _>("assists"),
                "curstreak": row.get::<i64, _>("curstreak"),
                "winstreak": row.get::<i64, _>("winstreak"),
                "losestreak": row.get::<i64, _>("losestreak"),
                "rounds": row.get::<i64, _>("rounds"),
            })
        }).collect())
    }

    async fn get_xlr_player_stats(&self, client_id: i64) -> Result<Option<serde_json::Value>, StorageError> {
        let row = sqlx::query(
            "SELECT s.*, c.name, c.guid FROM xlr_playerstats s \
             INNER JOIN clients c ON s.client_id = c.id \
             WHERE s.client_id = ?"
        )
        .bind(client_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;

        Ok(row.map(|row| {
            serde_json::json!({
                "client_id": row.get::<i64, _>("client_id"),
                "name": row.get::<String, _>("name"),
                "kills": row.get::<i64, _>("kills"),
                "deaths": row.get::<i64, _>("deaths"),
                "teamkills": row.get::<i64, _>("teamkills"),
                "teamdeaths": row.get::<i64, _>("teamdeaths"),
                "suicides": row.get::<i64, _>("suicides"),
                "ratio": row.get::<f64, _>("ratio"),
                "skill": row.get::<f64, _>("skill"),
                "assists": row.get::<i64, _>("assists"),
                "rounds": row.get::<i64, _>("rounds"),
            })
        }))
    }

    async fn get_xlr_weapon_stats(&self, client_id: Option<i64>) -> Result<Vec<serde_json::Value>, StorageError> {
        let rows = if let Some(cid) = client_id {
            sqlx::query(
                "SELECT * FROM xlr_weaponstats WHERE client_id = ? ORDER BY kills DESC"
            )
            .bind(cid)
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default()
        } else {
            sqlx::query(
                "SELECT * FROM xlr_weaponusage ORDER BY kills DESC LIMIT 50"
            )
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default()
        };

        Ok(rows.iter().map(|row| {
            serde_json::json!({
                "name": row.get::<String, _>("name"),
                "kills": row.get::<i64, _>("kills"),
                "deaths": row.get::<i64, _>("deaths"),
            })
        }).collect())
    }

    async fn get_xlr_map_stats(&self) -> Result<Vec<serde_json::Value>, StorageError> {
        let rows = sqlx::query(
            "SELECT * FROM xlr_mapstats ORDER BY rounds DESC LIMIT 50"
        )
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        Ok(rows.iter().map(|row| {
            serde_json::json!({
                "name": row.get::<String, _>("name"),
                "kills": row.get::<i64, _>("kills"),
                "rounds": row.get::<i64, _>("rounds"),
                "suicides": row.get::<i64, _>("suicides"),
            })
        }).collect())
    }

    // ---- Chat messages ----

    async fn save_chat_message(&self, msg: &ChatMessage) -> Result<i64, StorageError> {
        let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let result = sqlx::query(
            "INSERT INTO chat_messages (client_id, client_name, channel, message, time_add) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(msg.client_id)
        .bind(&msg.client_name)
        .bind(&msg.channel)
        .bind(&msg.message)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(result.last_insert_rowid())
    }

    async fn get_chat_messages(&self, limit: u32, before_id: Option<i64>) -> Result<Vec<ChatMessage>, StorageError> {
        self.search_chat_messages(None, None, limit, before_id).await
    }

    async fn search_chat_messages(&self, query: Option<&str>, client_id: Option<i64>, limit: u32, before_id: Option<i64>) -> Result<Vec<ChatMessage>, StorageError> {
        let mut sql = String::from("SELECT * FROM chat_messages WHERE 1=1");
        if before_id.is_some() {
            sql.push_str(" AND id < ?");
        }
        if client_id.is_some() {
            sql.push_str(" AND client_id = ?");
        }
        if query.is_some() {
            sql.push_str(" AND message LIKE ?");
        }
        sql.push_str(" ORDER BY id DESC LIMIT ?");

        let mut q = sqlx::query(&sql);
        if let Some(bid) = before_id {
            q = q.bind(bid);
        }
        if let Some(cid) = client_id {
            q = q.bind(cid);
        }
        if let Some(search) = query {
            q = q.bind(format!("%{}%", search));
        }
        q = q.bind(limit as i64);

        let rows = q.fetch_all(&self.pool).await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(rows.iter().map(row_to_chat_message).collect())
    }

    // ---- Vote history ----

    async fn save_vote(&self, vote: &VoteRecord) -> Result<i64, StorageError> {
        let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let result = sqlx::query(
            "INSERT INTO vote_history (client_id, client_name, vote_type, vote_data, time_add) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(vote.client_id)
        .bind(&vote.client_name)
        .bind(&vote.vote_type)
        .bind(&vote.vote_data)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(result.last_insert_rowid())
    }

    async fn get_recent_votes(&self, limit: u32) -> Result<Vec<VoteRecord>, StorageError> {
        let rows = sqlx::query(
            "SELECT * FROM vote_history ORDER BY id DESC LIMIT ?"
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(rows.iter().map(row_to_vote_record).collect())
    }

    // ---- Admin notes ----

    async fn get_admin_note(&self, admin_user_id: i64) -> Result<Option<AdminNote>, StorageError> {
        let row = sqlx::query("SELECT * FROM admin_notes WHERE admin_user_id = ?")
            .bind(admin_user_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(row.map(|r| row_to_admin_note(&r)))
    }

    async fn save_admin_note(&self, admin_user_id: i64, content: &str) -> Result<(), StorageError> {
        let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let result = sqlx::query(
            "UPDATE admin_notes SET content = ?, updated_at = ? WHERE admin_user_id = ?"
        )
        .bind(content)
        .bind(&now)
        .bind(admin_user_id)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;

        if result.rows_affected() == 0 {
            sqlx::query(
                "INSERT INTO admin_notes (admin_user_id, content, updated_at) VALUES (?, ?, ?)"
            )
            .bind(admin_user_id)
            .bind(content)
            .bind(&now)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        }
        Ok(())
    }

    // ---- Map configuration ----

    async fn get_map_configs(&self) -> Result<Vec<MapConfig>, StorageError> {
        let rows = sqlx::query("SELECT * FROM map_configs ORDER BY map_name")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(rows.iter().map(row_to_map_config).collect())
    }

    async fn get_map_config(&self, map_name: &str) -> Result<Option<MapConfig>, StorageError> {
        let row = sqlx::query("SELECT * FROM map_configs WHERE map_name = ?")
            .bind(map_name)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(row.as_ref().map(row_to_map_config))
    }

    async fn get_map_config_by_id(&self, id: i64) -> Result<MapConfig, StorageError> {
        sqlx::query("SELECT * FROM map_configs WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?
            .as_ref()
            .map(row_to_map_config)
            .ok_or(StorageError::NotFound)
    }

    async fn save_map_config(&self, config: &MapConfig) -> Result<i64, StorageError> {
        let now = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        if config.id > 0 {
            sqlx::query(
                "UPDATE map_configs SET map_name=?, gametype=?, capturelimit=?, timelimit=?, fraglimit=?, \
                 g_gear=?, g_gravity=?, g_friendlyfire=?, g_followstrict=?, g_waverespawns=?, \
                 g_bombdefusetime=?, g_bombexplodetime=?, g_swaproles=?, g_maxrounds=?, g_matchmode=?, \
                 g_respawndelay=?, startmessage=?, skiprandom=?, bot=?, custom_commands=?, updated_at=? \
                 WHERE id=?"
            )
            .bind(&config.map_name)
            .bind(&config.gametype)
            .bind(config.capturelimit)
            .bind(config.timelimit)
            .bind(config.fraglimit)
            .bind(&config.g_gear)
            .bind(config.g_gravity)
            .bind(config.g_friendlyfire)
            .bind(config.g_followstrict)
            .bind(config.g_waverespawns)
            .bind(config.g_bombdefusetime)
            .bind(config.g_bombexplodetime)
            .bind(config.g_swaproles)
            .bind(config.g_maxrounds)
            .bind(config.g_matchmode)
            .bind(config.g_respawndelay)
            .bind(&config.startmessage)
            .bind(config.skiprandom)
            .bind(config.bot)
            .bind(&config.custom_commands)
            .bind(&now)
            .bind(config.id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
            Ok(config.id)
        } else {
            let result = sqlx::query(
                "INSERT INTO map_configs (map_name, gametype, capturelimit, timelimit, fraglimit, \
                 g_gear, g_gravity, g_friendlyfire, g_followstrict, g_waverespawns, \
                 g_bombdefusetime, g_bombexplodetime, g_swaproles, g_maxrounds, g_matchmode, \
                 g_respawndelay, startmessage, skiprandom, bot, custom_commands, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&config.map_name)
            .bind(&config.gametype)
            .bind(config.capturelimit)
            .bind(config.timelimit)
            .bind(config.fraglimit)
            .bind(&config.g_gear)
            .bind(config.g_gravity)
            .bind(config.g_friendlyfire)
            .bind(config.g_followstrict)
            .bind(config.g_waverespawns)
            .bind(config.g_bombdefusetime)
            .bind(config.g_bombexplodetime)
            .bind(config.g_swaproles)
            .bind(config.g_maxrounds)
            .bind(config.g_matchmode)
            .bind(config.g_respawndelay)
            .bind(&config.startmessage)
            .bind(config.skiprandom)
            .bind(config.bot)
            .bind(&config.custom_commands)
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
            Ok(result.last_insert_rowid())
        }
    }

    async fn delete_map_config(&self, id: i64) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM map_configs WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(())
    }

    // ---- Dashboard summary ----

    async fn get_dashboard_summary(&self) -> Result<DashboardSummary, StorageError> {
        let row = sqlx::query(
            "SELECT \
             (SELECT COUNT(*) FROM clients) as total_clients, \
             (SELECT COUNT(*) FROM penalties WHERE type = 'Warning' AND inactive = 0) as total_warnings, \
             (SELECT COUNT(*) FROM penalties WHERE type = 'TempBan' AND inactive = 0) as total_tempbans, \
             (SELECT COUNT(*) FROM penalties WHERE type = 'Ban' AND inactive = 0) as total_bans"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;

        Ok(DashboardSummary {
            total_clients: row.get::<i64, _>("total_clients") as u64,
            total_warnings: row.get::<i64, _>("total_warnings") as u64,
            total_tempbans: row.get::<i64, _>("total_tempbans") as u64,
            total_bans: row.get::<i64, _>("total_bans") as u64,
        })
    }

    // ---- Server management (master/client mode) ----

    async fn get_servers(&self) -> Result<Vec<GameServer>, StorageError> {
        let rows = sqlx::query("SELECT * FROM servers ORDER BY name")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;

        Ok(rows.iter().map(|r| GameServer {
            id: r.get("id"),
            name: r.get("name"),
            address: r.get("address"),
            port: r.get::<i32, _>("port") as u16,
            status: r.get("status"),
            current_map: r.get("current_map"),
            player_count: r.get::<i32, _>("player_count") as u32,
            max_clients: r.get::<i32, _>("max_clients") as u32,
            last_seen: r.get("last_seen"),
            config_json: r.get("config_json"),
            config_version: r.get("config_version"),
            cert_fingerprint: r.get("cert_fingerprint"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }).collect())
    }

    async fn get_server(&self, server_id: i64) -> Result<GameServer, StorageError> {
        let row = sqlx::query("SELECT * FROM servers WHERE id = ?")
            .bind(server_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;

        Ok(GameServer {
            id: row.get("id"),
            name: row.get("name"),
            address: row.get("address"),
            port: row.get::<i32, _>("port") as u16,
            status: row.get("status"),
            current_map: row.get("current_map"),
            player_count: row.get::<i32, _>("player_count") as u32,
            max_clients: row.get::<i32, _>("max_clients") as u32,
            last_seen: row.get("last_seen"),
            config_json: row.get("config_json"),
            config_version: row.get("config_version"),
            cert_fingerprint: row.get("cert_fingerprint"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    async fn get_server_by_fingerprint(&self, fingerprint: &str) -> Result<Option<GameServer>, StorageError> {
        let row = sqlx::query("SELECT * FROM servers WHERE cert_fingerprint = ?")
            .bind(fingerprint)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;

        Ok(row.map(|r| GameServer {
            id: r.get("id"),
            name: r.get("name"),
            address: r.get("address"),
            port: r.get::<i32, _>("port") as u16,
            status: r.get("status"),
            current_map: r.get("current_map"),
            player_count: r.get::<i32, _>("player_count") as u32,
            max_clients: r.get::<i32, _>("max_clients") as u32,
            last_seen: r.get("last_seen"),
            config_json: r.get("config_json"),
            config_version: r.get("config_version"),
            cert_fingerprint: r.get("cert_fingerprint"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }))
    }

    async fn save_server(&self, server: &GameServer) -> Result<i64, StorageError> {
        if server.id > 0 {
            sqlx::query(
                "UPDATE servers SET name = ?, address = ?, port = ?, status = ?, \
                 current_map = ?, player_count = ?, max_clients = ?, last_seen = ?, \
                 config_json = ?, config_version = ?, cert_fingerprint = ?, \
                 updated_at = CURRENT_TIMESTAMP WHERE id = ?"
            )
            .bind(&server.name)
            .bind(&server.address)
            .bind(server.port as i32)
            .bind(&server.status)
            .bind(&server.current_map)
            .bind(server.player_count as i32)
            .bind(server.max_clients as i32)
            .bind(&server.last_seen)
            .bind(&server.config_json)
            .bind(server.config_version)
            .bind(&server.cert_fingerprint)
            .bind(server.id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
            Ok(server.id)
        } else {
            let result = sqlx::query(
                "INSERT INTO servers (name, address, port, status, current_map, player_count, \
                 max_clients, last_seen, config_json, config_version, cert_fingerprint) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&server.name)
            .bind(&server.address)
            .bind(server.port as i32)
            .bind(&server.status)
            .bind(&server.current_map)
            .bind(server.player_count as i32)
            .bind(server.max_clients as i32)
            .bind(&server.last_seen)
            .bind(&server.config_json)
            .bind(server.config_version)
            .bind(&server.cert_fingerprint)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
            Ok(result.last_insert_rowid())
        }
    }

    async fn update_server_status(
        &self,
        server_id: i64,
        status: &str,
        map: Option<&str>,
        players: u32,
        max_clients: u32,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE servers SET status = ?, current_map = ?, player_count = ?, \
             max_clients = ?, last_seen = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP \
             WHERE id = ?"
        )
        .bind(status)
        .bind(map)
        .bind(players as i32)
        .bind(max_clients as i32)
        .bind(server_id)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(())
    }

    async fn delete_server(&self, server_id: i64) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM servers WHERE id = ?")
            .bind(server_id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(())
    }

    // ---- Sync queue (client-side offline queue) ----

    async fn enqueue_sync(
        &self,
        entity_type: &str,
        entity_id: Option<i64>,
        action: &str,
        payload: &str,
        server_id: Option<i64>,
    ) -> Result<i64, StorageError> {
        let result = sqlx::query(
            "INSERT INTO sync_queue (entity_type, entity_id, action, payload, server_id) \
             VALUES (?, ?, ?, ?, ?)"
        )
        .bind(entity_type)
        .bind(entity_id)
        .bind(action)
        .bind(payload)
        .bind(server_id)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(result.last_insert_rowid())
    }

    async fn dequeue_sync(&self, limit: u32) -> Result<Vec<SyncQueueEntry>, StorageError> {
        let rows = sqlx::query(
            "SELECT * FROM sync_queue WHERE synced_at IS NULL \
             ORDER BY created_at ASC LIMIT ?"
        )
        .bind(limit as i32)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;

        Ok(rows.iter().map(|r| SyncQueueEntry {
            id: r.get("id"),
            entity_type: r.get("entity_type"),
            entity_id: r.get("entity_id"),
            action: r.get("action"),
            payload: r.get("payload"),
            server_id: r.get("server_id"),
            retry_count: r.get("retry_count"),
            created_at: r.get("created_at"),
            synced_at: r.get("synced_at"),
        }).collect())
    }

    async fn mark_synced(&self, ids: &[i64]) -> Result<(), StorageError> {
        if ids.is_empty() {
            return Ok(());
        }
        let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "UPDATE sync_queue SET synced_at = CURRENT_TIMESTAMP WHERE id IN ({})",
            placeholders.join(",")
        );
        let mut query = sqlx::query(&sql);
        for id in ids {
            query = query.bind(id);
        }
        query.execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(())
    }

    async fn retry_sync(&self, id: i64) -> Result<(), StorageError> {
        sqlx::query("UPDATE sync_queue SET retry_count = retry_count + 1 WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(())
    }

    async fn prune_synced(&self, older_than_days: u32) -> Result<u64, StorageError> {
        let result = sqlx::query(
            "DELETE FROM sync_queue WHERE synced_at IS NOT NULL \
             AND synced_at < datetime('now', ?)"
        )
        .bind(format!("-{} days", older_than_days))
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(result.rows_affected())
    }
}
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sqlite_client_crud() {
        let storage = SqliteStorage::new("sqlite::memory:").await.unwrap();

        // Insert a new client
        let mut client = Client::new("abc123guid", "TestPlayer");
        client.ip = Some("192.168.1.1".parse().unwrap());
        client.group_bits = 1;

        let id = storage.save_client(&client).await.unwrap();
        assert!(id > 0);

        // Read back
        let loaded = storage.get_client(id).await.unwrap();
        assert_eq!(loaded.guid, "abc123guid");
        assert_eq!(loaded.name, "TestPlayer");
        assert_eq!(loaded.group_bits, 1);
        assert_eq!(loaded.ip.unwrap().to_string(), "192.168.1.1");

        // Update
        let mut updated = loaded;
        updated.name = "RenamedPlayer".to_string();
        updated.group_bits = 8;
        storage.save_client(&updated).await.unwrap();

        let reloaded = storage.get_client(id).await.unwrap();
        assert_eq!(reloaded.name, "RenamedPlayer");
        assert_eq!(reloaded.group_bits, 8);

        // Find by GUID
        let found = storage.get_client_by_guid("abc123guid").await.unwrap();
        assert_eq!(found.id, id);

        // Search
        let results = storage.find_clients("Renamed").await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_sqlite_penalty_crud() {
        let storage = SqliteStorage::new("sqlite::memory:").await.unwrap();

        // Create a client first
        let client = Client::new("pen_guid", "PenaltyPlayer");
        let client_id = storage.save_client(&client).await.unwrap();

        // Save a warning
        let penalty = Penalty {
            id: 0,
            penalty_type: PenaltyType::Warning,
            client_id,
            admin_id: None,
            duration: None,
            reason: "Bad language".to_string(),
            keyword: "badlang".to_string(),
            inactive: false,
            time_add: Utc::now(),
            time_edit: Utc::now(),
            time_expire: None,
        };
        let pen_id = storage.save_penalty(&penalty).await.unwrap();
        assert!(pen_id > 0);

        // Count warnings
        let count = storage.count_penalties(client_id, PenaltyType::Warning).await.unwrap();
        assert_eq!(count, 1);

        // Get last penalty
        let last = storage.get_last_penalty(client_id, PenaltyType::Warning).await.unwrap();
        assert!(last.is_some());
        assert_eq!(last.unwrap().reason, "Bad language");

        // Disable warnings
        storage.disable_penalties(client_id, PenaltyType::Warning).await.unwrap();
        let count = storage.count_penalties(client_id, PenaltyType::Warning).await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_sqlite_groups() {
        let storage = SqliteStorage::new("sqlite::memory:").await.unwrap();

        let groups = storage.get_groups().await.unwrap();
        assert!(groups.len() >= 8); // Default groups from migration

        let guest = storage.get_group(0).await.unwrap();
        assert_eq!(guest.name, "Guest");
        assert_eq!(guest.keyword, "guest");
        assert_eq!(guest.level, 0);

        let admin = storage.get_group(16).await.unwrap();
        assert_eq!(admin.name, "Admin");
        assert_eq!(admin.level, 40);
    }

    #[tokio::test]
    async fn test_sqlite_tables() {
        let storage = SqliteStorage::new("sqlite::memory:").await.unwrap();
        let tables = storage.get_tables().await.unwrap();
        assert!(tables.contains(&"clients".to_string()));
        assert!(tables.contains(&"penalties".to_string()));
        assert!(tables.contains(&"groups".to_string()));
        assert!(tables.contains(&"aliases".to_string()));
    }

    #[tokio::test]
    async fn test_sqlite_alias_crud() {
        let storage = SqliteStorage::new("sqlite::memory:").await.unwrap();

        // Create a client
        let client = Client::new("alias_guid", "OriginalName");
        let client_id = storage.save_client(&client).await.unwrap();

        // Save alias — first time inserts with num_used=1
        storage.save_alias(client_id, "OriginalName").await.unwrap();
        let aliases = storage.get_aliases(client_id).await.unwrap();
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].alias, "OriginalName");
        assert_eq!(aliases[0].num_used, 1);

        // Save same alias again — should increment num_used
        storage.save_alias(client_id, "OriginalName").await.unwrap();
        let aliases = storage.get_aliases(client_id).await.unwrap();
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].num_used, 2);

        // Save a different alias
        storage.save_alias(client_id, "NewName").await.unwrap();
        let aliases = storage.get_aliases(client_id).await.unwrap();
        assert_eq!(aliases.len(), 2);
        // Ordered by num_used DESC, so OriginalName (2) comes first
        assert_eq!(aliases[0].alias, "OriginalName");
        assert_eq!(aliases[0].num_used, 2);
        assert_eq!(aliases[1].alias, "NewName");
        assert_eq!(aliases[1].num_used, 1);
    }

    #[tokio::test]
    async fn test_sqlite_is_banned() {
        let storage = SqliteStorage::new("sqlite::memory:").await.unwrap();

        // Create a client
        let client = Client::new("ban_guid", "BannedPlayer");
        let client_id = storage.save_client(&client).await.unwrap();

        // Not banned initially
        assert!(!storage.is_banned(client_id).await.unwrap());

        // Add a permanent ban
        let ban = Penalty {
            id: 0,
            penalty_type: PenaltyType::Ban,
            client_id,
            admin_id: None,
            duration: None,
            reason: "Cheating".to_string(),
            keyword: "cheating".to_string(),
            inactive: false,
            time_add: Utc::now(),
            time_edit: Utc::now(),
            time_expire: None,
        };
        storage.save_penalty(&ban).await.unwrap();

        // Now banned
        assert!(storage.is_banned(client_id).await.unwrap());

        // Disable the ban
        storage.disable_penalties(client_id, PenaltyType::Ban).await.unwrap();
        assert!(!storage.is_banned(client_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_sqlite_is_banned_tempban() {
        let storage = SqliteStorage::new("sqlite::memory:").await.unwrap();

        let client = Client::new("tempban_guid", "TempBannedPlayer");
        let client_id = storage.save_client(&client).await.unwrap();

        // Add a tempban that expires in 1 hour
        let tempban = Penalty {
            id: 0,
            penalty_type: PenaltyType::TempBan,
            client_id,
            admin_id: None,
            duration: Some(60),
            reason: "Spamming".to_string(),
            keyword: "spam".to_string(),
            inactive: false,
            time_add: Utc::now(),
            time_edit: Utc::now(),
            time_expire: Some(Utc::now() + chrono::Duration::hours(1)),
        };
        storage.save_penalty(&tempban).await.unwrap();

        // Should be banned (tempban not expired)
        assert!(storage.is_banned(client_id).await.unwrap());

        // Add a tempban that already expired
        let client2 = Client::new("expired_guid", "ExpiredPlayer");
        let client2_id = storage.save_client(&client2).await.unwrap();
        let expired = Penalty {
            id: 0,
            penalty_type: PenaltyType::TempBan,
            client_id: client2_id,
            admin_id: None,
            duration: Some(60),
            reason: "Old ban".to_string(),
            keyword: "old".to_string(),
            inactive: false,
            time_add: Utc::now() - chrono::Duration::hours(2),
            time_edit: Utc::now() - chrono::Duration::hours(2),
            time_expire: Some(Utc::now() - chrono::Duration::hours(1)),
        };
        storage.save_penalty(&expired).await.unwrap();

        // Should NOT be banned (tempban expired)
        assert!(!storage.is_banned(client2_id).await.unwrap());
    }
}
