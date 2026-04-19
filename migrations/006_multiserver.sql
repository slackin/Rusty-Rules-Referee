-- 006_multiserver.sql: Multi-server (client/server mode) schema additions

-- Registered game server bots (managed by master)
CREATE TABLE IF NOT EXISTS servers (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT    NOT NULL,
    address     TEXT    NOT NULL,
    port        INTEGER NOT NULL DEFAULT 27960,
    status      TEXT    NOT NULL DEFAULT 'offline',  -- online, offline, degraded
    current_map TEXT,
    player_count INTEGER NOT NULL DEFAULT 0,
    max_clients  INTEGER NOT NULL DEFAULT 0,
    last_seen   DATETIME,
    config_json TEXT,            -- full TOML/JSON config for this server
    config_version INTEGER NOT NULL DEFAULT 0,
    cert_fingerprint TEXT,       -- SHA-256 fingerprint of the client cert
    created_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Track which server a penalty came from and whether it's global
ALTER TABLE penalties ADD COLUMN server_id INTEGER REFERENCES servers(id);
ALTER TABLE penalties ADD COLUMN scope TEXT NOT NULL DEFAULT 'local';  -- 'local' or 'global'

-- Track which server a chat message came from
ALTER TABLE chat_messages ADD COLUMN server_id INTEGER REFERENCES servers(id);

-- Persistent queue for offline sync (used by client bots)
CREATE TABLE IF NOT EXISTS sync_queue (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_type TEXT    NOT NULL,  -- 'event', 'penalty', 'player', 'config', 'chat', 'stats'
    entity_id   INTEGER,
    action      TEXT    NOT NULL,  -- 'create', 'update', 'delete', 'sync'
    payload     TEXT    NOT NULL,  -- JSON serialized data
    server_id   INTEGER,
    retry_count INTEGER NOT NULL DEFAULT 0,
    created_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    synced_at   DATETIME
);

CREATE INDEX IF NOT EXISTS idx_sync_queue_unsynced ON sync_queue(synced_at) WHERE synced_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_sync_queue_entity ON sync_queue(entity_type, action);
CREATE INDEX IF NOT EXISTS idx_servers_status ON servers(status);
CREATE INDEX IF NOT EXISTS idx_penalties_server ON penalties(server_id);
CREATE INDEX IF NOT EXISTS idx_penalties_scope ON penalties(scope);
CREATE INDEX IF NOT EXISTS idx_chat_messages_server ON chat_messages(server_id);
