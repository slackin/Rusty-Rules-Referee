# Changelog

All notable changes to Rusty Rules Referee will be documented in this file.

## [Unreleased]

### Changed — Release Channels (BREAKING)

The auto-update system now uses release channels. Artifacts on the update server
are organized by channel (`production`, `beta`, `alpha`, `dev`). Automated builds
(`deploy.sh` / `push-update.sh`) always publish to the **dev** channel. Use the
new `promote.sh` script on the update server to move builds between channels
(`dev → alpha → beta → production`).

- **New** `[update].channel` config field (default: `beta`). Valid values:
  `production`, `beta`, `alpha`, `dev`. Invalid values cause startup failure.
- **New** `promote.sh` at repo root — promotes builds between channels.
- Client now fetches manifest at `{url}/{channel}/latest.json` instead of
  `{url}/latest.json`.
- **BREAKING**: The legacy `/api/updates/latest.json` endpoint has been removed
  from the publish pipeline. Clients running versions prior to this release will
  stop auto-updating until reinstalled with a channel-aware build.
- `production` channel directory is reserved for future use; no automation
  currently writes to it.

## [2.1.0] - 2026-04-18

### Added

#### Web Dashboard Enhancements
- **Live Scoreboard** — Real-time scoreboard with Red/Blue team grouping, scores, and ping
- **Live Chat Panel** — In-game chat messages streamed via WebSocket with team/all channel badges
- **Vote History Panel** — Track callvotes with player name, vote type, data, and timestamp
- **Personal Notes** — Per-admin notepad persisted in the database, accessible from the dashboard
- **Enhanced Stats Cards** — Dashboard now shows 6 stat cards (players, map, game type, uptime, warnings, total bans)
- **Quick Access Panel** — One-click shortcut bar to key management pages
- **Audit Log Page** — Dedicated page with paginated, filterable admin action history (admin-only)
- **Dashboard Summary API** — New `/api/v1/stats/summary` endpoint for aggregate counts

#### Backend
- New database migration (004): `chat_messages`, `vote_history`, `admin_notes` tables with indexes
- Storage trait extended with 7 new methods: chat message CRUD, vote CRUD, admin notes, dashboard summary
- SQLite and MySQL implementations for all new storage methods
- Chat messages persisted to database via chatlogger plugin
- Callvotes persisted to database via callvote plugin
- 4 new API endpoints: `/chat`, `/votes`, `/notes` (GET + PUT), `/audit-log`
- Players API now returns `score` and `ping` fields for live scoreboard

#### Frontend
- Reactive live store now tracks `recentChat` and `recentVotes` arrays
- WebSocket events for say/team-say and callvote pushed to live stores in real-time
- API client extended with methods for chat, votes, notes, audit log, and dashboard summary
- Audit Log nav item added to sidebar

#### New Plugins
- **headshotcounter** — Headshot streak tracking and announcements
- **namechecker** — Player name validation and enforcement
- **specchecker** — Spectator monitoring and enforcement

### Changed
- Dashboard page fully redesigned with 2-column grid layout featuring scoreboard, chat, votes, notes, and activity feed
- Plugin count increased from 22 to 30

## [2.0.0] - 2026-04-17

### Added

- Complete rewrite in Rust (from the original Python bot)
- Async runtime powered by tokio
- Urban Terror 4.3 log parser with 18 regex patterns and 40+ weapon mappings
- 22 plugins: admin, poweradminurt, censor, censorurt, spamcontrol, tk, welcome, chatlogger, stats, xlrstats, pingwatch, countryfilter, vpncheck, afk, spree, spawnkill, firstkill, flagannounce, adv, scheduler, mapconfig, makeroom, nickreg, callvote, customcommands, login, follow
- Plugin trait system with lifecycle management and event dispatching
- Event system with 60+ event types
- SQLite and MySQL database backends with automatic migrations
- UDP RCON client for Quake 3 engine protocol
- Async log file tailer with rotation detection
- TOML-based configuration
- XLRstats extended player statistics
