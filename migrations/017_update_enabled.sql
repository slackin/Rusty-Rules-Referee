-- 017_update_enabled.sql: Per-server and per-hub auto-update enable flag.
--
-- Lets the master toggle auto-update on/off for each managed hub and
-- client bot from the web UI. The fleet-wide default is ON (1) so that
-- legacy rows, which lack this flag, inherit the new default when the
-- migration runs. Operators can pin a specific build by flipping this to
-- 0 from the Hub or Server detail page.

ALTER TABLE servers ADD COLUMN update_enabled INTEGER NOT NULL DEFAULT 1;
ALTER TABLE hubs    ADD COLUMN update_enabled INTEGER NOT NULL DEFAULT 1;
