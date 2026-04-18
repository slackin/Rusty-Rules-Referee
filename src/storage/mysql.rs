use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::mysql::{MySqlPool, MySqlPoolOptions, MySqlRow};
use sqlx::Row;
use tracing::info;

use crate::core::{Alias, Client, Group, Penalty, PenaltyType};
use crate::storage::{Storage, StorageError, StorageProtocol};

pub struct MysqlStorage {
    pool: MySqlPool,
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
        // MySQL DDL — adapted from the SQLite migration
        let statements = [
            "CREATE TABLE IF NOT EXISTS `groups` (
                id BIGINT PRIMARY KEY,
                name VARCHAR(255) NOT NULL,
                keyword VARCHAR(255) NOT NULL UNIQUE,
                level INT NOT NULL DEFAULT 0,
                time_add DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                time_edit DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
            )",
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
            )",
            "CREATE TABLE IF NOT EXISTS aliases (
                id BIGINT PRIMARY KEY AUTO_INCREMENT,
                client_id BIGINT NOT NULL,
                alias VARCHAR(255) NOT NULL,
                num_used INT NOT NULL DEFAULT 1,
                time_add DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                time_edit DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                INDEX idx_aliases_client (client_id),
                FOREIGN KEY (client_id) REFERENCES clients(id)
            )",
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
            )",
        ];

        for stmt in &statements {
            sqlx::query(stmt)
                .execute(&self.pool)
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
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
        }

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
    }
}

fn str_to_penalty_type(s: &str) -> PenaltyType {
    match s {
        "Warning" => PenaltyType::Warning,
        "Notice" => PenaltyType::Notice,
        "Kick" => PenaltyType::Kick,
        "Ban" => PenaltyType::Ban,
        "TempBan" => PenaltyType::TempBan,
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

    async fn save_client(&self, client: &Client) -> Result<i64, StorageError> {
        let ip_str = client.ip.map(|ip| ip.to_string());
        let last_visit_ndt = client.last_visit.map(|dt| dt.naive_utc());

        if client.id > 0 {
            sqlx::query(
                "UPDATE clients SET name = ?, ip = ?, greeting = ?, login = ?, password = ?, \
                 group_bits = ?, auto_login = ?, last_visit = ?, pbid = ? WHERE id = ?"
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
            .bind(client.id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::QueryFailed(e.to_string()))?;
            Ok(client.id)
        } else {
            let result = sqlx::query(
                "INSERT INTO clients (guid, pbid, name, ip, greeting, login, password, \
                 group_bits, auto_login, last_visit) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
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
}
