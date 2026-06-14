-- Per-server "seen" association so the Users tab can list every player that
-- has actually connected to a given server (not just those with permissions).
--
-- The `clients` table is global (one canonical row per GUID across all
-- servers). This junction records which servers each client has been seen on,
-- along with the most recent name/ip/auth observed there, so the master can
-- render a per-server known-users list. Group-based permissions still apply
-- globally wherever the shared group is assigned (handled in query logic).
--
-- Applied on the master (server_id identifies the originating client bot).
CREATE TABLE IF NOT EXISTS server_clients (
    server_id   INTEGER NOT NULL,
    client_guid TEXT    NOT NULL,
    last_name   TEXT,
    last_ip     TEXT,
    last_auth   TEXT,
    first_seen  TEXT    NOT NULL DEFAULT (datetime('now')),
    last_seen   TEXT    NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (server_id, client_guid)
);

CREATE INDEX IF NOT EXISTS idx_server_clients_server ON server_clients(server_id);
CREATE INDEX IF NOT EXISTS idx_server_clients_guid ON server_clients(client_guid);
