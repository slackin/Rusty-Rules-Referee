-- 016_hub_update_interval.sql: Per-hub auto-update check interval (seconds).
--
-- Mirrors servers.update_interval (migration 013). Lets the master
-- change how often each hub polls the update manifest without a
-- restart. Defaults to 3600 (1 hour) so existing hubs keep their
-- current behavior.

ALTER TABLE hubs ADD COLUMN update_interval INTEGER NOT NULL DEFAULT 3600;
