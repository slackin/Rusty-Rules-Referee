-- 014_hubs.sql: Hub orchestrator tables (master mode).
--
-- A hub pairs with the master and orchestrates one or more R3 client bots
-- (and optionally Urban Terror game-server installs) on a single host. The
-- master records each hub, the host telemetry it reports, and a periodic
-- ring of metrics samples for charting.

-- Registered hub orchestrators
CREATE TABLE IF NOT EXISTS hubs (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    name            TEXT    NOT NULL,
    address         TEXT    NOT NULL DEFAULT '',
    status          TEXT    NOT NULL DEFAULT 'offline',  -- online, offline, degraded
    last_seen       DATETIME,
    cert_fingerprint TEXT UNIQUE,
    hub_version     TEXT,
    build_hash      TEXT,
    created_at      DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at      DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Static-ish host info (one row per hub, upserted on heartbeat)
CREATE TABLE IF NOT EXISTS hub_host_info (
    hub_id            INTEGER PRIMARY KEY REFERENCES hubs(id) ON DELETE CASCADE,
    hostname          TEXT    NOT NULL DEFAULT '',
    os                TEXT    NOT NULL DEFAULT '',
    kernel            TEXT    NOT NULL DEFAULT '',
    cpu_model         TEXT    NOT NULL DEFAULT '',
    cpu_cores         INTEGER NOT NULL DEFAULT 0,
    total_ram_bytes   INTEGER NOT NULL DEFAULT 0,
    disk_total_bytes  INTEGER NOT NULL DEFAULT 0,
    public_ip         TEXT,
    external_ip       TEXT,
    urt_installs_json TEXT    NOT NULL DEFAULT '[]',
    updated_at        DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Periodic host metric samples (append-only ring; pruned by master)
CREATE TABLE IF NOT EXISTS hub_host_metrics (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    hub_id    INTEGER NOT NULL REFERENCES hubs(id) ON DELETE CASCADE,
    ts        DATETIME NOT NULL,
    cpu_pct   REAL    NOT NULL DEFAULT 0,
    mem_pct   REAL    NOT NULL DEFAULT 0,
    disk_pct  REAL    NOT NULL DEFAULT 0,
    load1     REAL    NOT NULL DEFAULT 0,
    load5     REAL    NOT NULL DEFAULT 0,
    load15    REAL    NOT NULL DEFAULT 0,
    uptime_s  INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_hub_metrics_hub_ts
    ON hub_host_metrics(hub_id, ts);

-- Link client bots back to the hub that owns them, plus the systemd
-- instance slug used on that hub. Both nullable so existing standalone
-- clients keep working.
ALTER TABLE servers ADD COLUMN hub_id INTEGER REFERENCES hubs(id);
ALTER TABLE servers ADD COLUMN slug   TEXT;
