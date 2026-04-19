# Introduction

**Rusty Rules Referee (R3)** is a high-performance game server administration bot written in Rust. Inspired by [Big Brother Bot](https://www.bigbrotherbot.net/), R3 monitors your Urban Terror 4.3 server in real-time — enforcing rules automatically and providing admin commands for server management.

## What Does R3 Do?

R3 connects to your game server by tailing its log file and communicating via RCON. It:

- **Enforces rules automatically** — offensive language filtering, anti-spam, team kill limits, spawn kill detection, AFK kicking, and more
- **Provides admin commands** — `!kick`, `!ban`, `!warn`, `!tempban`, team management, and dozens more
- **Tracks statistics** — kill/death ratios, XLR skill ratings, weapon stats, map stats, and leaderboards
- **Offers a web dashboard** — live scoreboard, chat monitoring, RCON console, player management, and audit logging

## Features

| Feature | Description |
|---------|-------------|
| **30 Plugins** | Moderation, statistics, anti-abuse, chat logging, server management |
| **Plugin System** | Easy-to-implement `Plugin` trait for custom extensions |
| **Dual Database** | SQLite for simplicity, MySQL for scale |
| **RCON Integration** | Full UDP RCON client for the Quake 3 engine |
| **Real-Time Log Tailing** | Async file tailer with log rotation detection |
| **XLRstats** | Extended player statistics with ELO-based skill tracking |
| **Web Dashboard** | SvelteKit admin panel with live data via WebSocket |
| **Configurable** | TOML-based configuration with per-plugin settings |

## Supported Games

- **Urban Terror 4.3** (Quake 3 engine, UDP RCON)

## Architecture Overview

```
┌─────────────────┐    ┌──────────────┐    ┌───────────────┐
│  Game Server     │───▶│  Log Tailer  │───▶│  Log Parser   │
│  (Urban Terror)  │    │  (async)     │    │  (UrT 4.3)    │
└─────────────────┘    └──────────────┘    └───────┬───────┘
                                                    │
                                                    ▼
┌─────────────────┐    ┌──────────────┐    ┌───────────────┐
│  RCON Client    │◀───│  Plugins     │◀───│  Event System │
│  (UDP)          │    │  (30 total)  │    │  (60+ events) │
└─────────────────┘    └──────────────┘    └───────────────┘
                              │
                              ▼
                       ┌──────────────┐
                       │  Database    │
                       │  (SQLite/    │
                       │   MySQL)     │
                       └──────────────┘
```

1. The **Log Tailer** continuously reads new lines from the game server's log file
2. The **Parser** converts log lines into typed events (kills, chat, connections, etc.)
3. The **Event System** dispatches events to all enabled plugins
4. **Plugins** react by issuing RCON commands, updating the database, or both
5. The **RCON Client** sends commands back to the game server

## Next Steps

- [Installation](/guide/installation) — Build R3 from source
- [Quick Start](/guide/quick-start) — Get R3 running on your server
- [Configuration](/guide/configuration) — Full configuration reference
- [Plugins](/plugins/) — Learn about all 30 plugins
