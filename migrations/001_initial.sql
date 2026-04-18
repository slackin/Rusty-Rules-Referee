-- B3 initial schema

CREATE TABLE IF NOT EXISTS groups (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    keyword TEXT NOT NULL UNIQUE,
    level INTEGER NOT NULL DEFAULT 0,
    time_add TEXT NOT NULL DEFAULT (datetime('now')),
    time_edit TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS clients (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    guid TEXT NOT NULL UNIQUE,
    pbid TEXT NOT NULL DEFAULT '',
    name TEXT NOT NULL DEFAULT '',
    ip TEXT,
    greeting TEXT NOT NULL DEFAULT '',
    login TEXT NOT NULL DEFAULT '',
    password TEXT NOT NULL DEFAULT '',
    group_bits INTEGER NOT NULL DEFAULT 0,
    auto_login INTEGER NOT NULL DEFAULT 1,
    time_add TEXT NOT NULL DEFAULT (datetime('now')),
    time_edit TEXT NOT NULL DEFAULT (datetime('now')),
    last_visit TEXT
);

CREATE INDEX IF NOT EXISTS idx_clients_guid ON clients(guid);
CREATE INDEX IF NOT EXISTS idx_clients_name ON clients(name);

CREATE TABLE IF NOT EXISTS aliases (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    client_id INTEGER NOT NULL REFERENCES clients(id),
    alias TEXT NOT NULL,
    num_used INTEGER NOT NULL DEFAULT 1,
    time_add TEXT NOT NULL DEFAULT (datetime('now')),
    time_edit TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_aliases_client ON aliases(client_id);

CREATE TABLE IF NOT EXISTS penalties (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    type TEXT NOT NULL,
    client_id INTEGER NOT NULL REFERENCES clients(id),
    admin_id INTEGER,
    duration INTEGER,
    reason TEXT NOT NULL DEFAULT '',
    keyword TEXT NOT NULL DEFAULT '',
    inactive INTEGER NOT NULL DEFAULT 0,
    time_add TEXT NOT NULL DEFAULT (datetime('now')),
    time_edit TEXT NOT NULL DEFAULT (datetime('now')),
    time_expire TEXT
);

CREATE INDEX IF NOT EXISTS idx_penalties_client ON penalties(client_id);
CREATE INDEX IF NOT EXISTS idx_penalties_type ON penalties(type);

-- Default groups matching B3's standard permission levels
INSERT OR IGNORE INTO groups (id, name, keyword, level) VALUES (0,  'Guest',      'guest',     0);
INSERT OR IGNORE INTO groups (id, name, keyword, level) VALUES (1,  'User',       'user',      1);
INSERT OR IGNORE INTO groups (id, name, keyword, level) VALUES (2,  'Regular',    'reg',       2);
INSERT OR IGNORE INTO groups (id, name, keyword, level) VALUES (8,  'Moderator',  'mod',       20);
INSERT OR IGNORE INTO groups (id, name, keyword, level) VALUES (16, 'Admin',      'admin',     40);
INSERT OR IGNORE INTO groups (id, name, keyword, level) VALUES (32, 'Full Admin', 'fulladmin', 60);
INSERT OR IGNORE INTO groups (id, name, keyword, level) VALUES (64, 'Senior Admin','senioradmin',80);
INSERT OR IGNORE INTO groups (id, name, keyword, level) VALUES (128,'Super Admin','superadmin',100);
