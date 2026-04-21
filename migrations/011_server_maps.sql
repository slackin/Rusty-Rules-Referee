-- 011_server_maps.sql: per-server cache of installed maps, as reported by
-- the game engine via RCON `fdir *.bsp`. Populated by a background scanner
-- that runs every `map_repo.scan_interval_hours` and on server reconnect.
-- Lets the UI render "available maps" without a live RCON round-trip and
-- surfaces freshly-imported maps with a `pending_restart` flag until the
-- game engine re-scans its filesystem.

CREATE TABLE IF NOT EXISTS server_maps (
    server_id INTEGER NOT NULL,
    map_name TEXT NOT NULL,
    pk3_filename TEXT,
    first_seen_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    pending_restart INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (server_id, map_name)
);

CREATE INDEX IF NOT EXISTS idx_server_maps_server
    ON server_maps(server_id);
CREATE INDEX IF NOT EXISTS idx_server_maps_name_lower
    ON server_maps(LOWER(map_name));

CREATE TABLE IF NOT EXISTS server_map_scans (
    server_id INTEGER PRIMARY KEY,
    last_scan_at TEXT,
    last_scan_ok INTEGER NOT NULL DEFAULT 0,
    last_scan_error TEXT,
    map_count INTEGER NOT NULL DEFAULT 0
);
