pub mod mysql;
pub mod sqlite;

use async_trait::async_trait;
use thiserror::Error;

use crate::core::{Alias, AdminNote, AdminUser, AuditEntry, ChatMessage, Client, DashboardSummary, GameServer, Group, Hub, HubHostInfo, HubMetricSample, MapConfig, MapConfigDefault, MapRepoEntry, Penalty, PenaltyType, ServerMap, ServerMapScanStatus, SyncQueueEntry, VoteRecord};

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
    async fn list_clients(&self, limit: u32, offset: u32, search: Option<&str>, sort_by: &str, order: &str) -> Result<(Vec<Client>, u64), StorageError>;

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
    /// Audit entries scoped to a specific server. Default impl filters the full
    /// list in memory; backends are encouraged to override for efficiency.
    async fn get_audit_log_by_server(
        &self,
        server_id: i64,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<AuditEntry>, StorageError> {
        let all = self.get_audit_log(limit * 20, offset).await?;
        Ok(all
            .into_iter()
            .filter(|e| e.server_id == Some(server_id))
            .take(limit as usize)
            .collect())
    }

    /// Penalties scoped to a specific server.
    async fn get_penalties_by_server(
        &self,
        server_id: i64,
        limit: u32,
    ) -> Result<Vec<Penalty>, StorageError> {
        // Use get_last_bans() as a seed and filter by server_id — backends
        // should override with proper `WHERE server_id = ?` queries.
        let all = self.get_last_bans(limit * 20).await?;
        Ok(all
            .into_iter()
            .filter(|p| p.server_id == Some(server_id))
            .take(limit as usize)
            .collect())
    }

    /// Chat messages scoped to a specific server.
    async fn get_chat_messages_by_server(
        &self,
        server_id: i64,
        limit: u32,
        before_id: Option<i64>,
    ) -> Result<Vec<ChatMessage>, StorageError> {
        let all = self.get_chat_messages(limit * 20, before_id).await?;
        Ok(all
            .into_iter()
            .filter(|m| m.server_id == Some(server_id))
            .take(limit as usize)
            .collect())
    }

    // ---- XLR stats queries ----
    async fn get_xlr_leaderboard(&self, limit: u32, offset: u32) -> Result<Vec<serde_json::Value>, StorageError>;
    async fn get_xlr_player_stats(&self, client_id: i64) -> Result<Option<serde_json::Value>, StorageError>;
    async fn get_xlr_weapon_stats(&self, client_id: Option<i64>) -> Result<Vec<serde_json::Value>, StorageError>;
    async fn get_xlr_map_stats(&self) -> Result<Vec<serde_json::Value>, StorageError>;

    // ---- Chat message operations ----
    async fn save_chat_message(&self, msg: &ChatMessage) -> Result<i64, StorageError>;
    async fn get_chat_messages(&self, limit: u32, before_id: Option<i64>) -> Result<Vec<ChatMessage>, StorageError>;
    async fn search_chat_messages(&self, query: Option<&str>, client_id: Option<i64>, limit: u32, before_id: Option<i64>) -> Result<Vec<ChatMessage>, StorageError>;

    // ---- Vote history operations ----
    async fn save_vote(&self, vote: &VoteRecord) -> Result<i64, StorageError>;
    async fn get_recent_votes(&self, limit: u32) -> Result<Vec<VoteRecord>, StorageError>;

    // ---- Admin notes operations ----
    async fn get_admin_note(&self, admin_user_id: i64) -> Result<Option<AdminNote>, StorageError>;
    async fn save_admin_note(&self, admin_user_id: i64, content: &str) -> Result<(), StorageError>;

    // ---- Map configuration ----
    async fn get_map_configs(&self) -> Result<Vec<MapConfig>, StorageError>;
    async fn get_map_config(&self, map_name: &str) -> Result<Option<MapConfig>, StorageError>;
    async fn get_map_config_by_id(&self, id: i64) -> Result<MapConfig, StorageError>;
    async fn save_map_config(&self, config: &MapConfig) -> Result<i64, StorageError>;
    async fn delete_map_config(&self, id: i64) -> Result<(), StorageError>;
    /// Return an existing `MapConfig` for `map_name`, creating it from
    /// (map_config_defaults row → built-in defaults table → blank fallback)
    /// when absent. The created row is tagged with `source='auto'`.
    async fn ensure_map_config(&self, map_name: &str) -> Result<MapConfig, StorageError>;

    // ---- Map configuration defaults (master-only global template) ----
    async fn get_map_config_defaults(&self) -> Result<Vec<MapConfigDefault>, StorageError>;
    async fn get_map_config_default(&self, map_name: &str) -> Result<Option<MapConfigDefault>, StorageError>;
    async fn save_map_config_default(&self, def: &MapConfigDefault) -> Result<(), StorageError>;
    async fn delete_map_config_default(&self, map_name: &str) -> Result<(), StorageError>;

    // ---- Map repository cache (master-side) ----
    /// Upsert a batch of repo entries. Uses `filename` as primary key.
    async fn upsert_map_repo_entries(&self, entries: &[MapRepoEntry]) -> Result<u64, StorageError>;
    /// Case-insensitive substring search over `filename`. `query` empty
    /// returns the newest entries. Returns `(entries, total_matching)`.
    async fn search_map_repo(
        &self,
        query: &str,
        limit: u32,
        offset: u32,
    ) -> Result<(Vec<MapRepoEntry>, u64), StorageError>;
    async fn get_map_repo_entry(&self, filename: &str) -> Result<Option<MapRepoEntry>, StorageError>;
    /// Remove entries whose `last_seen_at` is older than the cutoff.
    /// Returns the number of rows deleted.
    async fn prune_map_repo_entries(&self, before: chrono::DateTime<chrono::Utc>) -> Result<u64, StorageError>;
    /// Total count of cached entries.
    async fn count_map_repo_entries(&self) -> Result<u64, StorageError>;
    /// Most recent `last_seen_at` across all entries, if any.
    async fn latest_map_repo_refresh(&self) -> Result<Option<chrono::DateTime<chrono::Utc>>, StorageError>;

    // ---- Per-server installed-map cache (master-side) ----
    /// Replace the full set of installed maps for a server with the given
    /// batch in a single transaction. Rows with `pending_restart = 1` are
    /// preserved even if absent from `maps`, so freshly-imported maps that
    /// the game engine hasn't re-scanned yet don't vanish from the UI.
    /// Returns the number of rows in the post-update set.
    async fn replace_server_maps(
        &self,
        server_id: i64,
        maps: &[ServerMap],
        scanned_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, StorageError>;
    /// Mark a single map as pending (imported but game engine has not yet
    /// reloaded its filesystem). Inserts if missing.
    async fn mark_server_map_pending(
        &self,
        server_id: i64,
        map_name: &str,
        pk3_filename: Option<&str>,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), StorageError>;
    /// List all cached installed maps for a server, ordered by `map_name`.
    async fn list_server_maps(&self, server_id: i64) -> Result<Vec<ServerMap>, StorageError>;
    /// Fetch the last scan status row for a server, if any.
    async fn get_server_map_scan(
        &self,
        server_id: i64,
    ) -> Result<Option<ServerMapScanStatus>, StorageError>;
    /// Record the outcome of a scan (success or failure).
    async fn record_server_map_scan(
        &self,
        server_id: i64,
        ok: bool,
        error: Option<&str>,
        map_count: i64,
        at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), StorageError>;
    /// Remove all cached maps and scan status for a server (used on
    /// `delete_server`).
    async fn delete_server_maps(&self, server_id: i64) -> Result<(), StorageError>;

    // ---- Dashboard summary ----
    async fn get_dashboard_summary(&self) -> Result<DashboardSummary, StorageError>;

    // ---- Server management (master/client mode) ----
    async fn get_servers(&self) -> Result<Vec<GameServer>, StorageError>;
    async fn get_server(&self, server_id: i64) -> Result<GameServer, StorageError>;
    async fn get_server_by_fingerprint(&self, fingerprint: &str) -> Result<Option<GameServer>, StorageError>;
    async fn save_server(&self, server: &GameServer) -> Result<i64, StorageError>;
    async fn update_server_status(&self, server_id: i64, status: &str, map: Option<&str>, players: u32, max_clients: u32) -> Result<(), StorageError>;
    /// Update the update-channel string for a server (master-controlled).
    async fn set_server_update_channel(&self, server_id: i64, channel: &str) -> Result<(), StorageError>;
    /// Update the auto-update check interval (seconds) for a server (master-controlled).
    async fn set_server_update_interval(&self, server_id: i64, interval_secs: u64) -> Result<(), StorageError>;
    async fn delete_server(&self, server_id: i64) -> Result<(), StorageError>;

    // ---- Hub orchestrators (master mode) ----
    async fn get_hubs(&self) -> Result<Vec<Hub>, StorageError>;
    async fn get_hub(&self, hub_id: i64) -> Result<Hub, StorageError>;
    async fn get_hub_by_fingerprint(&self, fingerprint: &str) -> Result<Option<Hub>, StorageError>;
    async fn save_hub(&self, hub: &Hub) -> Result<i64, StorageError>;
    /// Update the update-channel string for a hub (master-controlled).
    async fn set_hub_update_channel(&self, hub_id: i64, channel: &str) -> Result<(), StorageError>;
    async fn delete_hub(&self, hub_id: i64) -> Result<(), StorageError>;
    async fn upsert_host_info(&self, info: &HubHostInfo) -> Result<(), StorageError>;
    async fn get_host_info(&self, hub_id: i64) -> Result<Option<HubHostInfo>, StorageError>;
    async fn record_host_metric(&self, sample: &HubMetricSample) -> Result<(), StorageError>;
    async fn get_host_metrics(
        &self,
        hub_id: i64,
        since: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<HubMetricSample>, StorageError>;
    async fn prune_host_metrics(
        &self,
        older_than: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, StorageError>;
    /// List all `servers` rows owned by the given hub.
    async fn list_clients_for_hub(&self, hub_id: i64) -> Result<Vec<GameServer>, StorageError>;

    // ---- Sync queue (client-side offline queue) ----
    async fn enqueue_sync(&self, entity_type: &str, entity_id: Option<i64>, action: &str, payload: &str, server_id: Option<i64>) -> Result<i64, StorageError>;
    async fn dequeue_sync(&self, limit: u32) -> Result<Vec<SyncQueueEntry>, StorageError>;
    async fn mark_synced(&self, ids: &[i64]) -> Result<(), StorageError>;
    async fn retry_sync(&self, id: i64) -> Result<(), StorageError>;
    async fn prune_synced(&self, older_than_days: u32) -> Result<u64, StorageError>;
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
