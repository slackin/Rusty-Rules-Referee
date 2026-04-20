use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions, MySqlRow};
use sqlx::Row;
use tracing::info;

use crate::core::{Alias, AdminNote, AdminUser, AuditEntry, ChatMessage, Client, DashboardSummary, GameServer, Group, MapConfig, Penalty, PenaltyType, SyncQueueEntry, VoteRecord};
use crate::storage::{Storage, StorageError, StorageProtocol};

pub struct MysqlStorage {
    pool: MySqlPool,
}

impl MysqlStorage {
    /// Return a reference to the underlying connection pool (used for bulk migration).
    pub fn pool(&self) -> &MySqlPool {
        &self.pool
    }
}

impl MysqlStorage {
    pub async fn new(database_url: &str) -> Result<Self, StorageError> {
        let pool = MySqlPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
            .map_err(|e| StorageError::ConnectionFailed(e.to_string()))?;

        let storage = Self { pool };
        storage.run_migrations().await?;
        info!(url = %database_url, "MySQL storage connected");
        Ok(storage)
    }

    async fn run_migrations(&self) -> Result<(), StorageError> {
        // Run all migrations on a single connection to ensure session settings persist
        let mut conn = self.pool.acquire().await
            .map_err(|e| StorageError::QueryFailed(format!("MySQL migration error: {}", e)))?;

        // Disable FK checks during migration so tables can be created in any order
        // and avoid "Failed to open the referenced table" errors.
        sqlx::query("SET FOREIGN_KEY_CHECKS=0")
            .execute(&mut *conn)
            .await
            .map_err(|e| StorageError::QueryFailed(format!("MySQL migration error: {}", e)))?;

        // Ensure InnoDB is used for all tables (required for foreign keys)
        sqlx::query("SET default_storage_engine=InnoDB")
            .execute(&mut *conn)
            .await
            .map_err(|e| StorageError::QueryFailed(format!("MySQL migration error: {}", e)))?;

        // MySQL DDL — adapted from the SQLite migration
        let statements = [
            "CREATE TABLE IF NOT EXISTS `groups` (
                id BIGINT PRIMARY KEY,
                name VARCHAR(255) NOT NULL,
                keyword VARCHAR(255) NOT NULL UNIQUE,
                level INT NOT NULL DEFAULT 0,
                time_add DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                time_edit DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            "CREATE TABLE IF NOT EXISTS clients (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                guid VARCHAR(255) NOT NULL UNIQUE,
                pbid VARCHAR(255) NOT NULL DEFAULT '',
                name VARCHAR(255) NOT NULL DEFAULT '',
                ip VARCHAR(45),
                greeting TEXT NOT NULL,
                login VARCHAR(255) NOT NULL DEFAULT '',
                password VARCHAR(255) NOT NULL DEFAULT '',
                group_bits BIGINT NOT NULL DEFAULT 0,
                auto_login TINYINT NOT NULL DEFAULT 1,
                time_add DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                time_edit DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                last_visit DATETIME,
                INDEX idx_clients_guid (guid),
                INDEX idx_clients_name (name)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            "CREATE TABLE IF NOT EXISTS aliases (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                client_id BIGINT NOT NULL,
                alias VARCHAR(255) NOT NULL,
                num_used INT NOT NULL DEFAULT 1,
                time_add DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                time_edit DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                INDEX idx_aliases_client (client_id),
                FOREIGN KEY (client_id) REFERENCES clients(id)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            "CREATE TABLE IF NOT EXISTS penalties (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                type VARCHAR(32) NOT NULL,
                client_id BIGINT NOT NULL,
                admin_id BIGINT,
                duration BIGINT,
                reason TEXT NOT NULL,
                keyword VARCHAR(255) NOT NULL DEFAULT '',
                inactive TINYINT NOT NULL DEFAULT 0,
                time_add DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                time_edit DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                time_expire DATETIME,
                INDEX idx_penalties_client (client_id),
                INDEX idx_penalties_type (type),
                FOREIGN KEY (client_id) REFERENCES clients(id)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
        ];

        for stmt in &statements {
            sqlx::query(stmt)
                .execute(&mut *conn)
                .await
                .map_err(|e| StorageError::QueryFailed(format!("MySQL migration error: {}", e)))?;
        }

        // Insert default groups (ignore duplicates)
        let groups = [
            (0, "Guest", "guest", 0),
            (1, "User", "user", 1),
            (2, "Regular", "reg", 2),
            (8, "Moderator", "mod", 20),
            (16, "Admin", "admin", 40),
            (32, "Full Admin", "fulladmin", 60),
            (64, "Senior Admin", "senioradmin", 80),
            (128, "Super Admin", "superadmin", 100),
        ];
        for (id, name, keyword, level) in &groups {
            sqlx::query(
                "INSERT IGNORE INTO `groups` (id, name, keyword, level) VALUES (?, ?, ?, ?)"
            )
            .bind(id)
            .bind(name)
            .bind(keyword)
            .bind(level)
            .execute(&mut *conn)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        }

        // XLR stats tables
        let xlr_statements = [
            "CREATE TABLE IF NOT EXISTS xlr_playerstats (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                client_id BIGINT NOT NULL UNIQUE,
                kills INT NOT NULL DEFAULT 0,
                deaths INT NOT NULL DEFAULT 0,
                teamkills INT NOT NULL DEFAULT 0,
                teamdeaths INT NOT NULL DEFAULT 0,
                suicides INT NOT NULL DEFAULT 0,
                ratio DOUBLE NOT NULL DEFAULT 0.0,
                skill DOUBLE NOT NULL DEFAULT 1000.0,
                assists INT NOT NULL DEFAULT 0,
                assistskill DOUBLE NOT NULL DEFAULT 0.0,
                curstreak INT NOT NULL DEFAULT 0,
                winstreak INT NOT NULL DEFAULT 0,
                losestreak INT NOT NULL DEFAULT 0,
                rounds INT NOT NULL DEFAULT 0,
                smallestratio DOUBLE NOT NULL DEFAULT 0.0,
                biggestratio DOUBLE NOT NULL DEFAULT 0.0,
                smalleststreak INT NOT NULL DEFAULT 0,
                biggeststreak INT NOT NULL DEFAULT 0,
                INDEX idx_xlr_ps_client (client_id),
                INDEX idx_xlr_ps_skill (skill),
                FOREIGN KEY (client_id) REFERENCES clients(id)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            "CREATE TABLE IF NOT EXISTS xlr_weaponstats (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                client_id BIGINT NOT NULL,
                name VARCHAR(64) NOT NULL,
                kills INT NOT NULL DEFAULT 0,
                deaths INT NOT NULL DEFAULT 0,
                teamkills INT NOT NULL DEFAULT 0,
                teamdeaths INT NOT NULL DEFAULT 0,
                suicides INT NOT NULL DEFAULT 0,
                headshots INT NOT NULL DEFAULT 0,
                UNIQUE KEY uq_ws_client_name (client_id, name),
                FOREIGN KEY (client_id) REFERENCES clients(id)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            "CREATE TABLE IF NOT EXISTS xlr_weaponusage (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                name VARCHAR(64) NOT NULL UNIQUE,
                kills INT NOT NULL DEFAULT 0,
                deaths INT NOT NULL DEFAULT 0,
                teamkills INT NOT NULL DEFAULT 0,
                teamdeaths INT NOT NULL DEFAULT 0,
                suicides INT NOT NULL DEFAULT 0,
                headshots INT NOT NULL DEFAULT 0
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            "CREATE TABLE IF NOT EXISTS xlr_bodyparts (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                client_id BIGINT NOT NULL,
                name VARCHAR(64) NOT NULL,
                kills INT NOT NULL DEFAULT 0,
                deaths INT NOT NULL DEFAULT 0,
                teamkills INT NOT NULL DEFAULT 0,
                teamdeaths INT NOT NULL DEFAULT 0,
                suicides INT NOT NULL DEFAULT 0,
                UNIQUE KEY uq_bp_client_name (client_id, name),
                FOREIGN KEY (client_id) REFERENCES clients(id)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            "CREATE TABLE IF NOT EXISTS xlr_opponents (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                client_id BIGINT NOT NULL,
                target_id BIGINT NOT NULL,
                kills INT NOT NULL DEFAULT 0,
                deaths INT NOT NULL DEFAULT 0,
                retals INT NOT NULL DEFAULT 0,
                UNIQUE KEY uq_opp_client_target (client_id, target_id),
                FOREIGN KEY (client_id) REFERENCES clients(id),
                FOREIGN KEY (target_id) REFERENCES clients(id)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            "CREATE TABLE IF NOT EXISTS xlr_mapstats (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                name VARCHAR(64) NOT NULL UNIQUE,
                kills INT NOT NULL DEFAULT 0,
                suicides INT NOT NULL DEFAULT 0,
                teamkills INT NOT NULL DEFAULT 0,
                rounds INT NOT NULL DEFAULT 0
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            "CREATE TABLE IF NOT EXISTS xlr_history (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                client_id BIGINT NOT NULL,
                kills INT NOT NULL DEFAULT 0,
                deaths INT NOT NULL DEFAULT 0,
                skill DOUBLE NOT NULL DEFAULT 0.0,
                time_add DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_xlr_hist_client (client_id),
                FOREIGN KEY (client_id) REFERENCES clients(id)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            // Dashboard / admin tables
            "CREATE TABLE IF NOT EXISTS admin_users (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                username VARCHAR(255) NOT NULL UNIQUE,
                password_hash VARCHAR(255) NOT NULL,
                role VARCHAR(50) NOT NULL DEFAULT 'admin',
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            "CREATE TABLE IF NOT EXISTS audit_log (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                admin_user_id BIGINT,
                action VARCHAR(255) NOT NULL,
                detail TEXT NOT NULL,
                ip_address VARCHAR(45),
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (admin_user_id) REFERENCES admin_users(id)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            "CREATE TABLE IF NOT EXISTS chat_messages (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                client_id BIGINT NOT NULL,
                client_name VARCHAR(255) NOT NULL DEFAULT '',
                channel VARCHAR(50) NOT NULL DEFAULT '',
                message TEXT NOT NULL,
                time_add DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_chat_client (client_id),
                FOREIGN KEY (client_id) REFERENCES clients(id)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            "CREATE TABLE IF NOT EXISTS vote_history (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                client_id BIGINT NOT NULL,
                client_name VARCHAR(255) NOT NULL DEFAULT '',
                vote_type VARCHAR(50) NOT NULL DEFAULT '',
                vote_data TEXT NOT NULL,
                time_add DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_vote_client (client_id),
                FOREIGN KEY (client_id) REFERENCES clients(id)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
            "CREATE TABLE IF NOT EXISTS admin_notes (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                admin_user_id BIGINT NOT NULL UNIQUE,
                content TEXT NOT NULL,
                updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                FOREIGN KEY (admin_user_id) REFERENCES admin_users(id)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
        ];

        for stmt in &xlr_statements {
            sqlx::query(stmt)
                .execute(&mut *conn)
                .await
                .map_err(|e| StorageError::QueryFailed(format!("MySQL migration error: {}", e)))?;
        }

        // Add auth column if not present
        let _ = sqlx::query("ALTER TABLE clients ADD COLUMN auth VARCHAR(255) NOT NULL DEFAULT ''")
            .execute(&mut *conn)
            .await;

        // Map configs table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS map_configs (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                map_name VARCHAR(128) NOT NULL UNIQUE,
                gametype VARCHAR(16) NOT NULL DEFAULT '',
                capturelimit INT,
                timelimit INT,
                fraglimit INT,
                g_gear VARCHAR(64) NOT NULL DEFAULT '',
                g_gravity INT,
                g_friendlyfire INT,
                g_followstrict INT,
                g_waverespawns INT,
                g_bombdefusetime INT,
                g_bombexplodetime INT,
                g_swaproles INT,
                g_maxrounds INT,
                g_matchmode INT,
                g_respawndelay INT,
                startmessage VARCHAR(255) NOT NULL DEFAULT '',
                skiprandom INT NOT NULL DEFAULT 0,
                bot INT NOT NULL DEFAULT 0,
                custom_commands TEXT NOT NULL,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"
        )
        .execute(&mut *conn)
        .await
        .map_err(|e| StorageError::QueryFailed(format!("MySQL migration error: {}", e)))?;

        // Multi-server tables (006_multiserver)
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS servers (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                name VARCHAR(255) NOT NULL,
                address VARCHAR(255) NOT NULL,
                port INT NOT NULL DEFAULT 27960,
                status VARCHAR(32) NOT NULL DEFAULT 'offline',
                current_map VARCHAR(128),
                player_count INT NOT NULL DEFAULT 0,
                max_clients INT NOT NULL DEFAULT 0,
                last_seen DATETIME,
                config_json TEXT,
                config_version BIGINT NOT NULL DEFAULT 0,
                cert_fingerprint VARCHAR(128),
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                INDEX idx_servers_status (status)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"
        )
        .execute(&mut *conn)
        .await
        .map_err(|e| StorageError::QueryFailed(format!("MySQL migration error: {}", e)))?;

        // Add multi-server columns to penalties/chat_messages if not present
        let _ = sqlx::query("ALTER TABLE penalties ADD COLUMN server_id BIGINT REFERENCES servers(id)")
            .execute(&mut *conn)
            .await;
        let _ = sqlx::query("ALTER TABLE penalties ADD COLUMN scope VARCHAR(32) NOT NULL DEFAULT 'local'")
            .execute(&mut *conn)
            .await;
        let _ = sqlx::query("ALTER TABLE chat_messages ADD COLUMN server_id BIGINT REFERENCES servers(id)")
            .execute(&mut *conn)
            .await;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS sync_queue (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                entity_type VARCHAR(64) NOT NULL,
                entity_id BIGINT,
                action VARCHAR(32) NOT NULL,
                payload TEXT NOT NULL,
                server_id BIGINT,
                retry_count INT NOT NULL DEFAULT 0,
                created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                synced_at DATETIME,
                INDEX idx_sync_queue_entity (entity_type, action)
            ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"
        )
        .execute(&mut *conn)
        .await
        .map_err(|e| StorageError::QueryFailed(format!("MySQL migration error: {}", e)))?;

        // Re-enable foreign key checks
        sqlx::query("SET FOREIGN_KEY_CHECKS=1")
            .execute(&mut *conn)
            .await
            .map_err(|e| StorageError::QueryFailed(format!("MySQL migration error: {}", e)))?;

        info!("MySQL migrations complete");
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

fn parse_dt(ndt: Option<NaiveDateTime>) -> DateTime<Utc> {
    ndt.map(|n| n.and_utc()).unwrap_or_default()
}

fn row_to_client(row: &MySqlRow) -> Client {
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
    client.auto_login = row.get::<i8, _>("auto_login") != 0;
    client.auth = row.get::<Option<String>, _>("auth").unwrap_or_default();
    client.time_add = parse_dt(row.get("time_add"));
    client.time_edit = parse_dt(row.get("time_edit"));

    let ip_str: Option<String> = row.get("ip");
    client.ip = ip_str.and_then(|s| s.parse().ok());

    let lv: Option<NaiveDateTime> = row.get("last_visit");
    client.last_visit = lv.map(|n| n.and_utc());

    client
}

fn row_to_penalty(row: &MySqlRow) -> Penalty {
    let te: Option<NaiveDateTime> = row.get("time_expire");
    Penalty {
        id: row.get("id"),
        penalty_type: str_to_penalty_type(row.get("type")),
        client_id: row.get("client_id"),
        admin_id: row.get("admin_id"),
        duration: row.get("duration"),
        reason: row.get("reason"),
        keyword: row.get("keyword"),
        inactive: row.get::<i8, _>("inactive") != 0,
        time_add: parse_dt(row.get("time_add")),
        time_edit: parse_dt(row.get("time_edit")),
        time_expire: te.map(|n| n.and_utc()),
    }
}

fn row_to_group(row: &MySqlRow) -> Group {
    Group {
        id: row.get::<i64, _>("id") as u64,
        name: row.get("name"),
        keyword: row.get("keyword"),
        level: row.get::<i32, _>("level") as u32,
        time_add: parse_dt(row.get("time_add")),
        time_edit: parse_dt(row.get("time_edit")),
    }
}

fn row_to_alias(row: &MySqlRow) -> Alias {
    Alias {
        id: row.get("id"),
        client_id: row.get("client_id"),
        alias: row.get("alias"),
        num_used: row.get::<i32, _>("num_used") as u32,
        time_add: parse_dt(row.get("time_add")),
        time_edit: parse_dt(row.get("time_edit")),
    }
}

fn row_to_admin_user(row: &MySqlRow) -> AdminUser {
    AdminUser {
        id: row.get("id"),
        username: row.get("username"),
        password_hash: row.get("password_hash"),
        role: row.get("role"),
        created_at: parse_dt(row.get("created_at")),
        updated_at: parse_dt(row.get("updated_at")),
    }
}

fn row_to_audit_entry(row: &MySqlRow) -> AuditEntry {
    AuditEntry {
        id: row.get("id"),
        admin_user_id: row.get("admin_user_id"),
        action: row.get("action"),
        detail: row.get("detail"),
        ip_address: row.get("ip_address"),
        created_at: parse_dt(row.get("created_at")),
    }
}

fn row_to_chat_message(row: &MySqlRow) -> ChatMessage {
    ChatMessage {
        id: row.get("id"),
        client_id: row.get("client_id"),
        client_name: row.get("client_name"),
        channel: row.get("channel"),
        message: row.get("message"),
        time_add: parse_dt(row.get("time_add")),
    }
}

fn row_to_vote_record(row: &MySqlRow) -> VoteRecord {
    VoteRecord {
        id: row.get("id"),
        client_id: row.get("client_id"),
        client_name: row.get("client_name"),
        vote_type: row.get("vote_type"),
        vote_data: row.get("vote_data"),
        time_add: parse_dt(row.get("time_add")),
    }
}

fn row_to_admin_note(row: &MySqlRow) -> AdminNote {
    AdminNote {
        id: row.get("id"),
        admin_user_id: row.get("admin_user_id"),
        content: row.get("content"),
        updated_at: parse_dt(row.get("updated_at")),
    }
}

fn row_to_map_config(row: &MySqlRow) -> MapConfig {
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
impl Storage for MysqlStorage {
    fn protocol(&self) -> StorageProtocol {
        StorageProtocol::Mysql
    }

    async fn connect(&mut self) -> Result<(), StorageError> {
        Ok(())
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
            _ => "last_visit",
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
                .bind(&pattern).bind(&pattern).bind(&pattern).bind(limit).bind(offset)
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
                .bind(limit).bind(offset)
                .fetch_all(&self.pool).await
                .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
            (rows, total as u64)
        };

        Ok((rows.iter().map(row_to_client).collect(), total))
    }

    async fn save_client(&self, client: &Client) -> Result<i64, StorageError> {
        let ip_str = client.ip.map(|ip| ip.to_string());
        let last_visit_ndt = client.last_visit.map(|dt| dt.naive_utc());

        if client.id > 0 {
            sqlx::query(
                "UPDATE clients SET name = ?, ip = ?, greeting = ?, login = ?, password = ?, \
                 group_bits = ?, auto_login = ?, last_visit = ?, pbid = ?, auth = ? WHERE id = ?"
            )
            .bind(&client.name)
            .bind(&ip_str)
            .bind(&client.greeting)
            .bind(&client.login)
            .bind(&client.password)
            .bind(client.group_bits as i64)
            .bind(client.auto_login as i8)
            .bind(last_visit_ndt)
            .bind(&client.pbid)
            .bind(&client.auth)
            .bind(client.id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
            Ok(client.id)
        } else {
            let result = sqlx::query(
                "INSERT INTO clients (guid, pbid, name, ip, greeting, login, password, \
                 group_bits, auto_login, last_visit, auth) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&client.guid)
            .bind(&client.pbid)
            .bind(&client.name)
            .bind(&ip_str)
            .bind(&client.greeting)
            .bind(&client.login)
            .bind(&client.password)
            .bind(client.group_bits as i64)
            .bind(client.auto_login as i8)
            .bind(last_visit_ndt)
            .bind(&client.auth)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
            Ok(result.last_insert_id() as i64)
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
        let expire_ndt = penalty.time_expire.map(|dt| dt.naive_utc());

        let result = sqlx::query(
            "INSERT INTO penalties (type, client_id, admin_id, duration, reason, keyword, \
             inactive, time_expire) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(penalty_type_to_str(penalty.penalty_type))
        .bind(penalty.client_id)
        .bind(penalty.admin_id)
        .bind(penalty.duration)
        .bind(&penalty.reason)
        .bind(&penalty.keyword)
        .bind(penalty.inactive as i8)
        .bind(expire_ndt)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;

        Ok(result.last_insert_id() as i64)
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
        let rows = sqlx::query("SELECT * FROM `groups` ORDER BY level ASC")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(rows.iter().map(row_to_group).collect())
    }

    async fn get_group(&self, group_id: u64) -> Result<Group, StorageError> {
        sqlx::query("SELECT * FROM `groups` WHERE id = ?")
            .bind(group_id as i64)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?
            .map(|row| row_to_group(&row))
            .ok_or(StorageError::NotFound)
    }

    async fn get_tables(&self) -> Result<Vec<String>, StorageError> {
        let rows = sqlx::query("SHOW TABLES")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(rows.iter().map(|r| r.get::<String, usize>(0)).collect())
    }

    async fn save_alias(&self, client_id: i64, alias: &str) -> Result<(), StorageError> {
        // Try to increment existing alias
        let result = sqlx::query(
            "UPDATE aliases SET num_used = num_used + 1 WHERE client_id = ? AND alias = ?"
        )
        .bind(client_id)
        .bind(alias)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;

        if result.rows_affected() == 0 {
            sqlx::query(
                "INSERT INTO aliases (client_id, alias, num_used) VALUES (?, ?, 1)"
            )
            .bind(client_id)
            .bind(alias)
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
        if user.id > 0 {
            sqlx::query(
                "UPDATE admin_users SET username = ?, password_hash = ?, role = ? WHERE id = ?"
            )
            .bind(&user.username)
            .bind(&user.password_hash)
            .bind(&user.role)
            .bind(user.id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
            Ok(user.id)
        } else {
            let result = sqlx::query(
                "INSERT INTO admin_users (username, password_hash, role) VALUES (?, ?, ?)"
            )
            .bind(&user.username)
            .bind(&user.password_hash)
            .bind(&user.role)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
            Ok(result.last_insert_id() as i64)
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

    async fn save_audit_entry(&self, entry: &AuditEntry) -> Result<i64, StorageError> {
        let result = sqlx::query(
            "INSERT INTO audit_log (admin_user_id, action, detail, ip_address) VALUES (?, ?, ?, ?)"
        )
        .bind(entry.admin_user_id)
        .bind(&entry.action)
        .bind(&entry.detail)
        .bind(&entry.ip_address)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(result.last_insert_id() as i64)
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
                "ratio": row.get::<f64, _>("ratio"),
                "skill": row.get::<f64, _>("skill"),
                "rounds": row.get::<i64, _>("rounds"),
            })
        }).collect())
    }

    async fn get_xlr_player_stats(&self, client_id: i64) -> Result<Option<serde_json::Value>, StorageError> {
        let row = sqlx::query(
            "SELECT s.*, c.name FROM xlr_playerstats s \
             INNER JOIN clients c ON s.client_id = c.id WHERE s.client_id = ?"
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
                "ratio": row.get::<f64, _>("ratio"),
                "skill": row.get::<f64, _>("skill"),
            })
        }))
    }

    async fn get_xlr_weapon_stats(&self, client_id: Option<i64>) -> Result<Vec<serde_json::Value>, StorageError> {
        let rows = if let Some(cid) = client_id {
            sqlx::query("SELECT * FROM xlr_weaponstats WHERE client_id = ? ORDER BY kills DESC")
                .bind(cid)
                .fetch_all(&self.pool)
                .await
                .unwrap_or_default()
        } else {
            sqlx::query("SELECT * FROM xlr_weaponusage ORDER BY kills DESC LIMIT 50")
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
        let rows = sqlx::query("SELECT * FROM xlr_mapstats ORDER BY rounds DESC LIMIT 50")
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default();
        Ok(rows.iter().map(|row| {
            serde_json::json!({
                "name": row.get::<String, _>("name"),
                "kills": row.get::<i64, _>("kills"),
                "rounds": row.get::<i64, _>("rounds"),
            })
        }).collect())
    }

    // ---- Chat messages ----

    async fn save_chat_message(&self, msg: &ChatMessage) -> Result<i64, StorageError> {
        let result = sqlx::query(
            "INSERT INTO chat_messages (client_id, client_name, channel, message) VALUES (?, ?, ?, ?)"
        )
        .bind(msg.client_id)
        .bind(&msg.client_name)
        .bind(&msg.channel)
        .bind(&msg.message)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(result.last_insert_id() as i64)
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
        let result = sqlx::query(
            "INSERT INTO vote_history (client_id, client_name, vote_type, vote_data) VALUES (?, ?, ?, ?)"
        )
        .bind(vote.client_id)
        .bind(&vote.client_name)
        .bind(&vote.vote_type)
        .bind(&vote.vote_data)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(result.last_insert_id() as i64)
    }

    async fn get_recent_votes(&self, limit: u32) -> Result<Vec<VoteRecord>, StorageError> {
        let rows = sqlx::query("SELECT * FROM vote_history ORDER BY id DESC LIMIT ?")
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
        let result = sqlx::query(
            "UPDATE admin_notes SET content = ? WHERE admin_user_id = ?"
        )
        .bind(content)
        .bind(admin_user_id)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;

        if result.rows_affected() == 0 {
            sqlx::query(
                "INSERT INTO admin_notes (admin_user_id, content) VALUES (?, ?)"
            )
            .bind(admin_user_id)
            .bind(content)
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
        if config.id > 0 {
            sqlx::query(
                "UPDATE map_configs SET map_name=?, gametype=?, capturelimit=?, timelimit=?, fraglimit=?, \
                 g_gear=?, g_gravity=?, g_friendlyfire=?, g_followstrict=?, g_waverespawns=?, \
                 g_bombdefusetime=?, g_bombexplodetime=?, g_swaproles=?, g_maxrounds=?, g_matchmode=?, \
                 g_respawndelay=?, startmessage=?, skiprandom=?, bot=?, custom_commands=? \
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
                 g_respawndelay, startmessage, skiprandom, bot, custom_commands) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
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
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
            Ok(result.last_insert_id() as i64)
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
                 updated_at = NOW() WHERE id = ?"
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
            Ok(result.last_insert_id() as i64)
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
             max_clients = ?, last_seen = NOW(), updated_at = NOW() \
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
        Ok(result.last_insert_id() as i64)
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
            "UPDATE sync_queue SET synced_at = NOW() WHERE id IN ({})",
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
             AND synced_at < DATE_SUB(NOW(), INTERVAL ? DAY)"
        )
        .bind(older_than_days as i32)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        Ok(result.rows_affected())
    }
}
