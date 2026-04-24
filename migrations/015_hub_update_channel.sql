-- 015_hub_update_channel.sql: Per-hub update channel.
--
-- Mirrors servers.update_channel (migration 008). Lets the master tell each
-- hub which release stream (production | beta | alpha | dev) its binary
-- should pull updates from. Defaults to 'beta' so existing hubs keep
-- following the same channel they do today.

ALTER TABLE hubs ADD COLUMN update_channel TEXT NOT NULL DEFAULT 'beta';
