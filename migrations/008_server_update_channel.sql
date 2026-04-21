-- 008_server_update_channel.sql: Per-slave update channel

ALTER TABLE servers ADD COLUMN update_channel TEXT NOT NULL DEFAULT 'beta';
