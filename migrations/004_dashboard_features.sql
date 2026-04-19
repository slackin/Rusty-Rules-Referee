-- Chat messages (persisted alongside existing file-based logging)
CREATE TABLE IF NOT EXISTS chat_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    client_id INTEGER NOT NULL,
    client_name TEXT NOT NULL DEFAULT '',
    channel TEXT NOT NULL DEFAULT 'SAY',
    message TEXT NOT NULL DEFAULT '',
    time_add TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (client_id) REFERENCES clients(id)
);

CREATE INDEX IF NOT EXISTS idx_chat_messages_time ON chat_messages(time_add);
CREATE INDEX IF NOT EXISTS idx_chat_messages_client ON chat_messages(client_id);

-- Vote history (persisted from callvote plugin)
CREATE TABLE IF NOT EXISTS vote_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    client_id INTEGER NOT NULL,
    client_name TEXT NOT NULL DEFAULT '',
    vote_type TEXT NOT NULL DEFAULT '',
    vote_data TEXT NOT NULL DEFAULT '',
    time_add TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (client_id) REFERENCES clients(id)
);

CREATE INDEX IF NOT EXISTS idx_vote_history_time ON vote_history(time_add);

-- Personal notes per admin user (dashboard scratchpad)
CREATE TABLE IF NOT EXISTS admin_notes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    admin_user_id INTEGER NOT NULL UNIQUE,
    content TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (admin_user_id) REFERENCES admin_users(id)
);
