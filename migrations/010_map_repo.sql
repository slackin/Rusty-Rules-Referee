-- 010_map_repo.sql: cache of .pk3 map files available on external repositories
-- (e.g. https://maps.pugbot.net/q3ut4/). Populated by the master's background
-- refresh task; queried by the web UI's map browser. Client installs run
-- through the sync layer and do not touch this table.

CREATE TABLE IF NOT EXISTS map_repo_entries (
    filename TEXT PRIMARY KEY,
    size BIGINT,
    mtime TEXT,
    source_url TEXT NOT NULL,
    last_seen_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_map_repo_entries_filename_lower
    ON map_repo_entries(LOWER(filename));
CREATE INDEX IF NOT EXISTS idx_map_repo_entries_last_seen
    ON map_repo_entries(last_seen_at);
