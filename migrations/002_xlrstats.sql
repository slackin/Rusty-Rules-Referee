-- XLRstats tables for Extended Live Rankings and Statistics

CREATE TABLE IF NOT EXISTS xlr_playerstats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    client_id INTEGER NOT NULL UNIQUE,
    kills INTEGER NOT NULL DEFAULT 0,
    deaths INTEGER NOT NULL DEFAULT 0,
    teamkills INTEGER NOT NULL DEFAULT 0,
    teamdeaths INTEGER NOT NULL DEFAULT 0,
    suicides INTEGER NOT NULL DEFAULT 0,
    ratio REAL NOT NULL DEFAULT 0.0,
    skill REAL NOT NULL DEFAULT 1000.0,
    assists INTEGER NOT NULL DEFAULT 0,
    assistskill REAL NOT NULL DEFAULT 0.0,
    curstreak INTEGER NOT NULL DEFAULT 0,
    winstreak INTEGER NOT NULL DEFAULT 0,
    losestreak INTEGER NOT NULL DEFAULT 0,
    rounds INTEGER NOT NULL DEFAULT 0,
    smallestratio REAL NOT NULL DEFAULT 0.0,
    biggestratio REAL NOT NULL DEFAULT 0.0,
    smalleststreak INTEGER NOT NULL DEFAULT 0,
    biggeststreak INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (client_id) REFERENCES clients(id)
);

CREATE INDEX IF NOT EXISTS idx_xlr_playerstats_client_id ON xlr_playerstats(client_id);
CREATE INDEX IF NOT EXISTS idx_xlr_playerstats_skill ON xlr_playerstats(skill);

CREATE TABLE IF NOT EXISTS xlr_weaponstats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    client_id INTEGER NOT NULL,
    name VARCHAR(64) NOT NULL,
    kills INTEGER NOT NULL DEFAULT 0,
    deaths INTEGER NOT NULL DEFAULT 0,
    teamkills INTEGER NOT NULL DEFAULT 0,
    teamdeaths INTEGER NOT NULL DEFAULT 0,
    suicides INTEGER NOT NULL DEFAULT 0,
    headshots INTEGER NOT NULL DEFAULT 0,
    UNIQUE(client_id, name),
    FOREIGN KEY (client_id) REFERENCES clients(id)
);

CREATE TABLE IF NOT EXISTS xlr_weaponusage (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name VARCHAR(64) NOT NULL UNIQUE,
    kills INTEGER NOT NULL DEFAULT 0,
    deaths INTEGER NOT NULL DEFAULT 0,
    teamkills INTEGER NOT NULL DEFAULT 0,
    teamdeaths INTEGER NOT NULL DEFAULT 0,
    suicides INTEGER NOT NULL DEFAULT 0,
    headshots INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS xlr_bodyparts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    client_id INTEGER NOT NULL,
    name VARCHAR(64) NOT NULL,
    kills INTEGER NOT NULL DEFAULT 0,
    deaths INTEGER NOT NULL DEFAULT 0,
    teamkills INTEGER NOT NULL DEFAULT 0,
    teamdeaths INTEGER NOT NULL DEFAULT 0,
    suicides INTEGER NOT NULL DEFAULT 0,
    UNIQUE(client_id, name),
    FOREIGN KEY (client_id) REFERENCES clients(id)
);

CREATE TABLE IF NOT EXISTS xlr_opponents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    client_id INTEGER NOT NULL,
    target_id INTEGER NOT NULL,
    kills INTEGER NOT NULL DEFAULT 0,
    deaths INTEGER NOT NULL DEFAULT 0,
    retals INTEGER NOT NULL DEFAULT 0,
    UNIQUE(client_id, target_id),
    FOREIGN KEY (client_id) REFERENCES clients(id),
    FOREIGN KEY (target_id) REFERENCES clients(id)
);

CREATE TABLE IF NOT EXISTS xlr_mapstats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name VARCHAR(64) NOT NULL UNIQUE,
    kills INTEGER NOT NULL DEFAULT 0,
    suicides INTEGER NOT NULL DEFAULT 0,
    teamkills INTEGER NOT NULL DEFAULT 0,
    rounds INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS xlr_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    client_id INTEGER NOT NULL,
    kills INTEGER NOT NULL DEFAULT 0,
    deaths INTEGER NOT NULL DEFAULT 0,
    skill REAL NOT NULL DEFAULT 0.0,
    time_add DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (client_id) REFERENCES clients(id)
);

CREATE INDEX IF NOT EXISTS idx_xlr_history_client_id ON xlr_history(client_id);
