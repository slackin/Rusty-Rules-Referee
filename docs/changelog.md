# Changelog

All notable changes to R3 (Rusty Rules Referee) are documented here.

## [2.1.0] - 2026-04-18

### Added

#### Web Dashboard Enhancements
- **Live Scoreboard** — Real-time scoreboard with Red/Blue team grouping, scores, and ping
- **Live Chat Panel** — In-game chat messages streamed via WebSocket with team/all channel badges
- **Vote History Panel** — Track callvotes with player name, vote type, data, and timestamp
- **Personal Notes** — Per-admin notepad persisted in the database
- **Enhanced Stats Cards** — 6 stat cards: players, map, game type, uptime, warnings, total bans
- **Quick Access Panel** — One-click shortcut bar to key management pages
- **Audit Log Page** — Paginated, filterable admin action history (admin-only)
- **Dashboard Summary API** — New `/api/v1/stats/summary` endpoint

#### Backend
- New database migration (004): `chat_messages`, `vote_history`, `admin_notes` tables
- Storage trait extended with 7 new methods for chat, votes, notes, and summary
- Chat messages persisted via chatlogger plugin
- Callvotes persisted via callvote plugin
- 4 new API endpoints: `/chat`, `/votes`, `/notes`, `/audit-log`
- Players API now returns `score` and `ping` fields

#### New Plugins
- **headshotcounter** — Headshot streak tracking and announcements
- **namechecker** — Player name validation and enforcement
- **specchecker** — Spectator monitoring and enforcement

### Changed
- Dashboard page redesigned with 2-column grid layout
- Plugin count increased from 22 to 30

---

## [2.0.0] - 2026-04-17

### Added
- Complete rewrite in Rust (from the original Python B3 bot)
- Async runtime powered by tokio
- Urban Terror 4.3 log parser with 18 regex patterns and 40+ weapon mappings
- 22 plugins at launch
- Plugin trait system with lifecycle management and event dispatching
- Event system with 60+ event types
- SQLite and MySQL database backends with automatic migrations
- UDP RCON client for Quake 3 engine protocol
- Async log file tailer with rotation detection
- TOML-based configuration
- XLRstats extended player statistics
- SvelteKit 2 web dashboard with Tailwind CSS
- JWT authentication and role-based access control
- WebSocket real-time event streaming
- REST API with 20+ endpoints
