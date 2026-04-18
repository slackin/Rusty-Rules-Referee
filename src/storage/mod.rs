pub mod mysql;
pub mod sqlite;

use async_trait::async_trait;
use thiserror::Error;

use crate::core::{Alias, AdminUser, AuditEntry, Client, Group, Penalty, PenaltyType};

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Database connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Query failed: {0}")]
    QueryFailed(String),
    #[error("Record not found")]
    NotFound,
    #[error("Storage error: {0}")]
    Other(#[from] anyhow::Error),
}

/// Supported storage protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageProtocol {
    Sqlite,
    Mysql,
}

/// The Storage trait — abstracts database operations.
///
/// Implementations: SqliteStorage, MysqlStorage.
#[async_trait]
pub trait Storage: Send + Sync {
    fn protocol(&self) -> StorageProtocol;

    async fn connect(&mut self) -> Result<(), StorageError>;
    async fn shutdown(&mut self) -> Result<(), StorageError>;

    // ---- Client operations ----
    async fn get_client(&self, client_id: i64) -> Result<Client, StorageError>;
    async fn get_client_by_guid(&self, guid: &str) -> Result<Client, StorageError>;
    async fn find_clients(&self, query: &str) -> Result<Vec<Client>, StorageError>;
    async fn save_client(&self, client: &Client) -> Result<i64, StorageError>;

    // ---- Penalty operations ----
    async fn get_penalties(&self, client_id: i64, penalty_type: Option<PenaltyType>) -> Result<Vec<Penalty>, StorageError>;
    async fn save_penalty(&self, penalty: &Penalty) -> Result<i64, StorageError>;
    async fn disable_penalties(&self, client_id: i64, penalty_type: PenaltyType) -> Result<(), StorageError>;
    async fn get_last_penalty(&self, client_id: i64, penalty_type: PenaltyType) -> Result<Option<Penalty>, StorageError>;
    async fn count_penalties(&self, client_id: i64, penalty_type: PenaltyType) -> Result<u64, StorageError>;

    // ---- Group operations ----
    async fn get_groups(&self) -> Result<Vec<Group>, StorageError>;
    async fn get_group(&self, group_id: u64) -> Result<Group, StorageError>;

    // ---- Alias operations ----
    async fn save_alias(&self, client_id: i64, alias: &str) -> Result<(), StorageError>;
    async fn get_aliases(&self, client_id: i64) -> Result<Vec<Alias>, StorageError>;
    async fn find_clients_by_alias(&self, query: &str) -> Result<Vec<Client>, StorageError>;

    // ---- Extended penalty operations ----
    async fn get_last_bans(&self, limit: u32) -> Result<Vec<Penalty>, StorageError>;
    async fn disable_last_penalty(&self, client_id: i64, penalty_type: PenaltyType) -> Result<bool, StorageError>;
    async fn disable_all_penalties_of_type(&self, client_id: i64, penalty_type: PenaltyType) -> Result<u64, StorageError>;

    // ---- Client search extensions ----
    async fn get_client_count_by_level(&self, min_level: u32) -> Result<u64, StorageError>;

    // ---- Convenience helpers ----
    /// Check if a client has an active ban or unexpired tempban.
    async fn is_banned(&self, client_id: i64) -> Result<bool, StorageError> {
        let bans = self.get_penalties(client_id, Some(PenaltyType::Ban)).await?;
        if bans.iter().any(|p| !p.inactive) {
            return Ok(true);
        }
        let tempbans = self.get_penalties(client_id, Some(PenaltyType::TempBan)).await?;
        let now = chrono::Utc::now();
        Ok(tempbans.iter().any(|p| !p.inactive && p.time_expire.is_some_and(|exp| exp > now)))
    }

    // ---- Schema / migration ----
    async fn get_tables(&self) -> Result<Vec<String>, StorageError>;

    // ---- Admin user operations (web UI) ----
    async fn get_admin_user(&self, username: &str) -> Result<AdminUser, StorageError>;
    async fn get_admin_user_by_id(&self, id: i64) -> Result<AdminUser, StorageError>;
    async fn get_admin_users(&self) -> Result<Vec<AdminUser>, StorageError>;
    async fn save_admin_user(&self, user: &AdminUser) -> Result<i64, StorageError>;
    async fn delete_admin_user(&self, id: i64) -> Result<(), StorageError>;

    // ---- Audit log operations ----
    async fn save_audit_entry(&self, entry: &AuditEntry) -> Result<i64, StorageError>;
    async fn get_audit_log(&self, limit: u32, offset: u32) -> Result<Vec<AuditEntry>, StorageError>;

    // ---- XLR stats queries ----
    async fn get_xlr_leaderboard(&self, limit: u32, offset: u32) -> Result<Vec<serde_json::Value>, StorageError>;
    async fn get_xlr_player_stats(&self, client_id: i64) -> Result<Option<serde_json::Value>, StorageError>;
    async fn get_xlr_weapon_stats(&self, client_id: Option<i64>) -> Result<Vec<serde_json::Value>, StorageError>;
    async fn get_xlr_map_stats(&self) -> Result<Vec<serde_json::Value>, StorageError>;
}

/// Parse a DSN string like "mysql://user:pass@host:port/db" into components.
pub fn parse_dsn(dsn: &str) -> anyhow::Result<DsnComponents> {
    let url = url_like_parse(dsn)?;
    Ok(url)
}

#[derive(Debug, Clone)]
pub struct DsnComponents {
    pub protocol: StorageProtocol,
    pub host: String,
    pub port: Option<u16>,
    pub user: Option<String>,
    pub password: Option<String>,
    pub database: String,
}

fn url_like_parse(dsn: &str) -> anyhow::Result<DsnComponents> {
    let (protocol_str, rest) = dsn
        .split_once("://")
        .ok_or_else(|| anyhow::anyhow!("Invalid DSN: missing ://"))?;

    let protocol = match protocol_str {
        "sqlite" => StorageProtocol::Sqlite,
        "mysql" => StorageProtocol::Mysql,
        _ => anyhow::bail!("Unsupported storage protocol: {}", protocol_str),
    };

    if protocol == StorageProtocol::Sqlite {
        return Ok(DsnComponents {
            protocol,
            host: String::new(),
            port: None,
            user: None,
            password: None,
            database: rest.to_string(),
        });
    }

    // Parse user:pass@host:port/database
    let (auth, host_db) = if let Some((a, h)) = rest.split_once('@') {
        (Some(a), h)
    } else {
        (None, rest)
    };

    let (user, password) = if let Some(auth) = auth {
        if let Some((u, p)) = auth.split_once(':') {
            (Some(u.to_string()), Some(p.to_string()))
        } else {
            (Some(auth.to_string()), None)
        }
    } else {
        (None, None)
    };

    let (host_port, database) = host_db
        .split_once('/')
        .ok_or_else(|| anyhow::anyhow!("Invalid DSN: missing database name"))?;

    let (host, port) = if let Some((h, p)) = host_port.split_once(':') {
        (h.to_string(), Some(p.parse()?))
    } else {
        (host_port.to_string(), None)
    };

    Ok(DsnComponents {
        protocol,
        host,
        port,
        user,
        password,
        database: database.to_string(),
    })
}

/// Create a storage backend from a DSN string.
/// Supports "sqlite://..." and "mysql://..." protocols.
pub async fn create_storage(dsn: &str) -> anyhow::Result<Box<dyn Storage>> {
    let components = parse_dsn(dsn)?;
    match components.protocol {
        StorageProtocol::Sqlite => {
            let url = format!("sqlite://{}", components.database);
            let storage = sqlite::SqliteStorage::new(&url).await?;
            Ok(Box::new(storage))
        }
        StorageProtocol::Mysql => {
            let storage = mysql::MysqlStorage::new(dsn).await?;
            Ok(Box::new(storage))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dsn_mysql() {
        let dsn = parse_dsn("mysql://r3user:r3pass@localhost:3306/r3db").unwrap();
        assert_eq!(dsn.protocol, StorageProtocol::Mysql);
        assert_eq!(dsn.host, "localhost");
        assert_eq!(dsn.port, Some(3306));
        assert_eq!(dsn.user.as_deref(), Some("r3user"));
        assert_eq!(dsn.password.as_deref(), Some("r3pass"));
        assert_eq!(dsn.database, "r3db");
    }

    #[test]
    fn test_parse_dsn_sqlite() {
        let dsn = parse_dsn("sqlite:///path/to/r3.db").unwrap();
        assert_eq!(dsn.protocol, StorageProtocol::Sqlite);
        assert_eq!(dsn.database, "/path/to/r3.db");
    }
}
