-- 013_server_update_interval.sql: Per-slave update check interval (seconds)

ALTER TABLE servers ADD COLUMN update_interval INTEGER NOT NULL DEFAULT 3600;
