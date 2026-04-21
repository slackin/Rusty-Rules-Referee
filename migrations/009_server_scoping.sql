-- 009_server_scoping.sql: per-server scoping of penalties, chat_messages,
-- audit_log, map_configs, and XLR statistics.
--
-- This migration is applied both on the master (where server_id identifies
-- the originating client bot) and on clients (where server_id is NULL and
-- the columns are effectively unused). NULL always means "legacy / unscoped".

-- penalties.server_id already added in 006_multiserver.sql.
-- chat_messages.server_id already added in 006_multiserver.sql.

-- Add server_id to audit_log so admin actions can be filtered per-server.
ALTER TABLE audit_log ADD COLUMN server_id INTEGER REFERENCES servers(id);
CREATE INDEX IF NOT EXISTS idx_audit_log_server ON audit_log(server_id);

-- map_configs: add server_id and replace UNIQUE(map_name) with (server_id, map_name).
-- SQLite doesn't support DROP CONSTRAINT, so for SQLite we just add a new
-- composite unique index; the old UNIQUE(map_name) remains but is overridden
-- in practice when server_id is populated. MySQL runtime code drops+recreates
-- the key explicitly.
ALTER TABLE map_configs ADD COLUMN server_id INTEGER REFERENCES servers(id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_map_configs_server_map
    ON map_configs(server_id, map_name);
CREATE INDEX IF NOT EXISTS idx_map_configs_server ON map_configs(server_id);

-- XLR statistics per-server. NULL server_id = global / pre-scoping history.
ALTER TABLE xlr_playerstats ADD COLUMN server_id INTEGER REFERENCES servers(id);
CREATE INDEX IF NOT EXISTS idx_xlr_playerstats_server ON xlr_playerstats(server_id);

ALTER TABLE xlr_weaponstats ADD COLUMN server_id INTEGER REFERENCES servers(id);
CREATE INDEX IF NOT EXISTS idx_xlr_weaponstats_server ON xlr_weaponstats(server_id);

ALTER TABLE xlr_mapstats ADD COLUMN server_id INTEGER REFERENCES servers(id);
CREATE INDEX IF NOT EXISTS idx_xlr_mapstats_server ON xlr_mapstats(server_id);
