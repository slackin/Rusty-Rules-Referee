-- Player Groups: named collections of player permission records that can be
-- shared across multiple servers. Servers inherit the union of all assigned
-- groups' records plus their own local client entries (highest group_bits wins).

CREATE TABLE IF NOT EXISTS player_groups (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT    NOT NULL UNIQUE,
    description TEXT    NOT NULL DEFAULT '',
    created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);

-- Members of a player group: one row per (group, player guid).
-- group_bits mirrors the encoding used in the clients table.
-- note is an optional free-text field (e.g. "TFG founding admin").
CREATE TABLE IF NOT EXISTS player_group_members (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    player_group_id INTEGER NOT NULL REFERENCES player_groups(id) ON DELETE CASCADE,
    client_guid    TEXT    NOT NULL,
    group_bits     INTEGER NOT NULL DEFAULT 0,
    note           TEXT    NOT NULL DEFAULT '',
    created_at     TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE(player_group_id, client_guid)
);

-- Which player groups a server belongs to, with explicit ordering priority.
-- Lower priority value = applied first; higher priority value wins on conflict.
CREATE TABLE IF NOT EXISTS server_player_groups (
    server_id       INTEGER NOT NULL REFERENCES servers(id) ON DELETE CASCADE,
    player_group_id INTEGER NOT NULL REFERENCES player_groups(id) ON DELETE CASCADE,
    priority        INTEGER NOT NULL DEFAULT 100,
    PRIMARY KEY (server_id, player_group_id)
);

CREATE INDEX IF NOT EXISTS idx_player_group_members_group ON player_group_members(player_group_id);
CREATE INDEX IF NOT EXISTS idx_player_group_members_guid  ON player_group_members(client_guid);
CREATE INDEX IF NOT EXISTS idx_server_player_groups_server ON server_player_groups(server_id);
