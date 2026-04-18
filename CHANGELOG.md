# Changelog

All notable changes to Rusty Rules Referee will be documented in this file.

## [2.0.0] - 2026-04-17

### Added

- Complete rewrite in Rust (from the original Python Big Brother Bot)
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
