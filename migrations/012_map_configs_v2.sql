-- 012_map_configs_v2.sql: richer per-map configuration.
--
-- Adds:
--   * supported_gametypes  (CSV of gametype ids; empty = all allowed)
--   * default_gametype     (gametype to switch to when current is not supported)
--   * g_suddendeath        (0/1 — nullable)
--   * g_teamdamage         (0/1 — nullable; distinct from g_friendlyfire)
--   * source               ('user' | 'auto' | 'default_seed') — flags
--                            whether a row has been edited by an admin.
--
-- Also creates `map_config_defaults` — a master-only table of "global"
-- defaults per map_name that the master can propagate down to every
-- server's map_configs row. It mirrors the map_configs columns, minus
-- server_id, with map_name as PK.

-- ---- map_configs column additions (best-effort; ignored on 2nd run) ----
ALTER TABLE map_configs ADD COLUMN supported_gametypes VARCHAR(64) NOT NULL DEFAULT '';
ALTER TABLE map_configs ADD COLUMN default_gametype VARCHAR(16);
ALTER TABLE map_configs ADD COLUMN g_suddendeath INTEGER;
ALTER TABLE map_configs ADD COLUMN g_teamdamage INTEGER;
ALTER TABLE map_configs ADD COLUMN source VARCHAR(16) NOT NULL DEFAULT 'user';

-- ---- map_config_defaults (global template, master-only) ----
CREATE TABLE IF NOT EXISTS map_config_defaults (
    map_name VARCHAR(128) PRIMARY KEY,
    gametype VARCHAR(16) NOT NULL DEFAULT '',
    supported_gametypes VARCHAR(64) NOT NULL DEFAULT '',
    default_gametype VARCHAR(16),
    capturelimit INTEGER,
    timelimit INTEGER,
    fraglimit INTEGER,
    g_gear VARCHAR(64) NOT NULL DEFAULT '',
    g_gravity INTEGER,
    g_friendlyfire INTEGER,
    g_teamdamage INTEGER,
    g_suddendeath INTEGER,
    g_followstrict INTEGER,
    g_waverespawns INTEGER,
    g_bombdefusetime INTEGER,
    g_bombexplodetime INTEGER,
    g_swaproles INTEGER,
    g_maxrounds INTEGER,
    g_matchmode INTEGER,
    g_respawndelay INTEGER,
    startmessage VARCHAR(255) NOT NULL DEFAULT '',
    skiprandom INTEGER NOT NULL DEFAULT 0,
    bot INTEGER NOT NULL DEFAULT 0,
    custom_commands TEXT NOT NULL DEFAULT '',
    source VARCHAR(16) NOT NULL DEFAULT 'user',
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Best-effort ALTER for deployments where the table was created by an
-- earlier revision of this migration (pre-source-column). Ignored on
-- fresh installs where the column is already present.
ALTER TABLE map_config_defaults ADD COLUMN source VARCHAR(16) NOT NULL DEFAULT 'user';

-- Seeded rows for the well-known stock Urban Terror 4.3 maps.
-- supported_gametypes is a CSV of gametype ids:
--   0=FFA 1=LMS 3=TDM 4=TS 5=FTL 6=CAH 7=CTF 8=Bomb 9=Jump 10=FT 11=GunGame
-- default_gametype is typically the one the level was designed around.
-- INSERT OR IGNORE is SQLite; MySQL supports "INSERT IGNORE" but the
-- runtime migration handler tolerates either dialect's failure on 2nd run.

INSERT OR IGNORE INTO map_config_defaults (map_name, gametype, supported_gametypes, default_gametype, source) VALUES
    ('ut4_abbey',         '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_abaddon_rc8',   '8', '3,4,7,8',        '8', 'default_seed'),
    ('ut4_algiers',       '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_austria',       '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_bohemia',       '8', '3,4,7,8',        '8', 'default_seed'),
    ('ut4_casa',          '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_cascade',       '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_docks',         '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_dressingroom',  '3', '0,3,4',          '3', 'default_seed'),
    ('ut4_eagle',         '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_elgin',         '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_firingrange',   '3', '0,3,4',          '3', 'default_seed'),
    ('ut4_ghosttown',     '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_herring',       '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_imperial_x13',  '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_jumpents',      '9', '9',              '9', 'default_seed'),
    ('ut4_killroom',      '0', '0,3,4',          '0', 'default_seed'),
    ('ut4_kingdom',       '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_kingpin',       '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_mandolin',      '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_mykonos_a17',   '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_oildepot',      '8', '3,4,7,8',        '8', 'default_seed'),
    ('ut4_paris',         '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_pipeline_b5',   '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_prague',        '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_prominence',    '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_raiders',       '8', '3,4,7,8',        '8', 'default_seed'),
    ('ut4_ramelle',       '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_ricochet',      '3', '0,3,4',          '3', 'default_seed'),
    ('ut4_riyadh',        '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_sanc',          '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_suburbs',       '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_subway',        '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_swim',          '3', '0,3,4',          '3', 'default_seed'),
    ('ut4_thingley',      '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_tohunga_b8',    '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_tombs',         '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_turnpike',      '7', '0,3,4,7,8',      '7', 'default_seed'),
    ('ut4_uptown',        '7', '0,3,4,7,8',      '7', 'default_seed');
