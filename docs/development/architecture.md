# Architecture

R3 is a modular, async game server administration bot written in Rust. This page describes the internal architecture and data flow.

## High-Level Overview

```
┌─────────────┐     ┌──────────┐     ┌──────────────┐
│ Game Server  │────▶│ Log File │────▶│  Log Tailer  │
│ (Urban Terror│     └──────────┘     └──────┬───────┘
│  4.3)        │                             │ raw lines
│              │◀────┐                       ▼
└─────────────┘     │               ┌──────────────┐
                    │               │    Parser     │
                    │               │ (UrbanTerror) │
                    │               └──────┬───────┘
                    │                      │ events
                    │                      ▼
              ┌─────┴────┐         ┌──────────────┐
              │   RCON    │◀───────│   Plugins    │
              │  Client   │        │  (30 total)  │
              └──────────┘         └──────┬───────┘
                                          │
                                          ▼
                                   ┌──────────────┐
                                   │   Storage    │
                                   │ SQLite/MySQL │
                                   └──────────────┘
```

## Module Structure

### `src/main.rs`
Entry point. Loads config, initializes storage, registers all plugins, starts the log tailer, and optionally starts the web server.

### `src/config/`
TOML configuration parsing with serde. Defines all config structs (`Config`, `ServerConfig`, `WebConfig`, `PluginConfig`).

### `src/core/`
The bot's runtime core:

- **`client.rs`** — Single connected player state (name, GUID, IP, group, score, team, etc.)
- **`clients.rs`** — Thread-safe client manager (`Arc<RwLock<HashMap>>`)
- **`context.rs`** — Shared bot context passed to plugins: clients, storage, config, RCON sender
- **`game.rs`** — Current game state (map, game type, scores)
- **`log_tailer.rs`** — Async file tailer with inotify/polling and log rotation detection
- **`types.rs`** — Shared type definitions (groups, penalties, stats records, etc.)

### `src/events/`
Event type definitions. Over 60 event variants covering kills, chat, connections, game state changes, admin actions, and more.

### `src/parsers/`
Log line parsing:

- **`traits.rs`** — `Parser` trait definition
- **`urbanterror/mod.rs`** — Urban Terror 4.3 parser with 18 regex patterns and 40+ weapon mappings

### `src/plugins/`
Plugin system:

- **`traits.rs`** — `Plugin` trait (name, enable/disable, handle_event, settings, commands)
- **`registry.rs`** — Plugin registry for ordering and dispatching events
- **`mod.rs`** — Re-exports all 30 plugins

Each plugin lives in its own subdirectory (e.g., `plugins/admin/mod.rs`).

### `src/rcon/`
UDP RCON client for the Quake 3 engine protocol. Sends commands to the game server and parses responses. Supports the `\xff\xff\xff\xff` packet prefix.

### `src/storage/`
Database abstraction:

- **`mod.rs`** — `Storage` trait with all database operations
- **`sqlite.rs`** — SQLite backend (via `sqlx`)
- **`mysql.rs`** — MySQL backend (via `sqlx`)

Both backends support automatic migrations from the `migrations/` directory.

### `src/web/`
Web dashboard and API:

- **`mod.rs`** — Axum router setup, static file serving via `rust_embed`
- **`auth.rs`** — JWT authentication middleware and extractors
- **`state.rs`** — Shared application state for web handlers
- **`ws.rs`** — WebSocket handler for real-time events
- **`api/`** — REST API endpoint handlers (13 modules)

## Event Flow

1. **Game server** writes log lines to the game log file
2. **Log Tailer** reads new lines asynchronously (with rotation handling)
3. **Parser** converts raw log lines into typed `Event` values
4. **Event Dispatcher** sends each event to all enabled plugins in priority order
5. **Plugins** process events and may:
   - Send RCON commands to the game server
   - Read/write to the database
   - Broadcast WebSocket messages to the dashboard
6. **Web Dashboard** receives real-time updates and serves the admin UI

## Async Runtime

R3 uses `tokio` as its async runtime. Key async operations:

- File I/O (log tailing)
- UDP socket (RCON)
- Database queries (sqlx)
- HTTP server (axum)
- WebSocket connections

## Plugin Lifecycle

1. **Construction** — Plugin is created with its config settings
2. **Enable** — `on_enable()` called, plugin initializes its state
3. **Event Handling** — `handle_event()` called for each game event
4. **Disable** — `on_disable()` called during shutdown
