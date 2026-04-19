# Rusty Rules Referee

A high-performance game server administration bot written in Rust, inspired by [Big Brother Bot](https://github.com/BigBrotherBot/big-brother-bot).

Rusty Rules Referee monitors your **Urban Terror 4.3** server in real-time — enforcing rules automatically (offensive language, team killing, spam) and providing admin commands (`!kick`, `!ban`, `!warn`, and more). It includes a modern web dashboard for server management and live monitoring.

## Features

- **Blazing fast** — Rust-native log parsing and async event handling via [tokio](https://tokio.rs)
- **Memory safe** — No GC pauses, no memory leaks in a 24/7 service
- **30 plugins** — Moderation, statistics, anti-abuse, chat logging, and more
- **Plugin system** — Easy-to-implement `Plugin` trait for custom extensions
- **Dual database support** — SQLite for simplicity, MySQL for scale
- **RCON integration** — Full UDP RCON client for the Quake 3 engine
- **Real-time log tailing** — Async file tailer with log rotation detection
- **XLRstats** — Extended player statistics and skill tracking
- **Web dashboard** — SvelteKit-based admin panel with live scoreboard, chat, and server management
- **WebSocket events** — Real-time event streaming to the dashboard
- **Configurable** — TOML-based configuration with per-plugin settings

## Supported Game

- **Urban Terror 4.3** (Quake 3 engine, UDP RCON)

## Quick Start

### Building

```sh
cargo build --release
```

### Running

```sh
# Copy and edit the example config
cp referee.example.toml referee.toml
# Edit referee.toml with your server details (RCON password, game log path, etc.)

# Run
cargo run --release -- referee.toml
```

## Configuration

Rusty Rules Referee uses TOML configuration. See [`referee.example.toml`](referee.example.toml) for a complete example with all available options.

```toml
[referee]
bot_name = "Referee"
bot_prefix = "^2RRR:^3"
database = "sqlite://referee.db"
logfile = "referee.log"
log_level = "info"

[server]
public_ip = "192.168.1.100"
port = 27960
rcon_password = "your_rcon_password_here"
game_log = "/path/to/server/games.log"
delay = 0.33
```

Key settings:
- **RCON** — Server IP, port, and RCON password
- **Storage** — SQLite (`sqlite://referee.db`) or MySQL (`mysql://user:pass@host/db`)
- **Log path** — Path to the Urban Terror `games.log` file
- **Plugins** — Enable/disable and configure each plugin individually

## Plugins

| Plugin | Description |
|---|---|
| **admin** | Core admin commands (`!kick`, `!ban`, `!warn`, `!find`, `!leveltest`, etc.) |
| **poweradminurt** | Urban Terror-specific admin (team balance, radio spam protection) |
| **censor** | Bad word filtering with configurable word lists |
| **censorurt** | Urban Terror-specific name censoring |
| **spamcontrol** | Chat flood protection |
| **tk** | Team kill monitoring and auto-penalties |
| **welcome** | New and returning player greeting messages |
| **chatlogger** | Daily rotating chat log files + database persistence |
| **stats** | Kill/death/ratio statistics tracking |
| **xlrstats** | Extended player statistics with skill ratings |
| **pingwatch** | High-ping detection and enforcement |
| **countryfilter** | GeoIP-based access control |
| **vpncheck** | VPN/proxy detection |
| **afk** | AFK player detection and management |
| **spree** | Kill spree announcements |
| **spawnkill** | Spawn kill detection and penalties |
| **firstkill** | First kill of the round announcements |
| **flagannounce** | CTF flag event announcements |
| **headshotcounter** | Headshot streak tracking and announcements |
| **adv** | Timed advertisement messages |
| **scheduler** | Scheduled server tasks |
| **mapconfig** | Per-map configuration loading |
| **makeroom** | Reserved slots for admins |
| **nickreg** | Nickname registration and protection |
| **namechecker** | Name validation and enforcement |
| **callvote** | Vote control, restrictions, and vote history logging |
| **customcommands** | Custom chat commands |
| **login** | Admin login system |
| **follow** | Player follow/watch system |
| **specchecker** | Spectator monitoring and enforcement |

## Architecture

```
src/
├── main.rs              # Entry point & main event loop
├── lib.rs               # Library root
├── config/              # TOML configuration loading
├── core/                # Core domain types
│   ├── client.rs        # Player data model (identity, session, permissions)
│   ├── clients.rs       # Connected-player manager (thread-safe)
│   ├── context.rs       # BotContext — shared state passed to all plugins
│   ├── game.rs          # Current game/map/round state
│   ├── log_tailer.rs    # Async log file tailer with rotation detection
│   └── types.rs         # Group, Penalty, Alias, ChatMessage, VoteRecord types
├── events/              # Event registry and typed event system (60+ event types)
├── parsers/             # Game log parser interface
│   ├── traits.rs        # GameParser trait
│   └── urbanterror/     # Urban Terror 4.3 parser (18 regex patterns, 40+ weapons)
├── plugins/             # Plugin system
│   ├── traits.rs        # Plugin trait (lifecycle + event handling)
│   ├── registry.rs      # Plugin lifecycle manager and event dispatcher
│   └── */               # 30 plugin implementations
├── rcon/                # RCON UDP client (Quake 3 engine protocol)
├── storage/             # Database abstraction layer
│   ├── sqlite.rs        # SQLite backend
│   └── mysql.rs         # MySQL backend
└── web/                 # Web dashboard & API
    ├── mod.rs           # Axum router, static file serving (rust_embed)
    ├── auth.rs          # JWT authentication & role-based extractors
    ├── state.rs         # Shared application state
    ├── ws.rs            # WebSocket event broadcasting
    └── api/             # REST API endpoints
        ├── players.rs   # Player management
        ├── penalties.rs # Penalty CRUD
        ├── stats.rs     # XLRstats + dashboard summary
        ├── chat.rs      # Chat message history
        ├── votes.rs     # Vote history
        ├── notes.rs     # Personal admin notes
        ├── audit.rs     # Audit log (admin-only)
        ├── config.rs    # Server configuration
        ├── server.rs    # Server status & RCON console
        └── users.rs     # Admin user management

ui/                      # SvelteKit frontend (built & embedded via rust_embed)
├── src/
│   ├── lib/
│   │   ├── api.svelte.js   # REST API client
│   │   ├── live.svelte.js  # Reactive live data store (WebSocket + polling)
│   │   └── ws.js           # WebSocket connection manager
│   └── routes/(app)/
│       ├── +page.svelte         # Dashboard (scoreboard, chat, stats, notes)
│       ├── players/             # Player list & detail pages
│       ├── penalties/           # Penalty management
│       ├── stats/               # XLRstats leaderboards
│       ├── console/             # RCON console
│       ├── config/              # Configuration editor
│       ├── audit-log/           # Admin audit trail
│       └── admin-users/         # Admin user management
```

### Web Dashboard

The built-in web dashboard provides a modern admin interface:

- **Live Scoreboard** — Real-time player list with teams, scores, and ping
- **Live Chat** — Server chat messages streamed via WebSocket
- **Dashboard Stats** — Player count, warnings, bans, map info at a glance
- **Vote History** — Track callvotes with who called what and when
- **Personal Notes** — Per-admin notepad persisted in the database
- **Quick Access** — One-click links to key management pages
- **Audit Log** — Full admin action trail (admin-only)
- **RCON Console** — Execute server commands from the browser
- **Player Management** — Search, kick, ban, change groups
- **XLRstats** — Leaderboards, weapon stats, map stats

The frontend is built with **SvelteKit 2** (Svelte 5 runes), **Tailwind CSS**, and compiled to static files that are embedded directly into the Rust binary via `rust_embed` — no separate web server needed.

#### Building the Frontend

```sh
cd ui
npm install
npm run build
```

The build output in `ui/build/` is automatically embedded when compiling the Rust binary.

### Event Flow

1. **Log tailer** reads new lines from the game server log
2. **Parser** converts log lines into typed events (kill, chat, connect, etc.)
3. **Event handler** processes client authentication and state management
4. **Plugin registry** dispatches events to all enabled plugins
5. **Plugins** react to events — issuing RCON commands, updating the database, etc.

## Adding a Plugin

Implement the `Plugin` trait:

```rust
use async_trait::async_trait;
use crate::core::context::BotContext;
use crate::events::Event;
use crate::plugins::{Plugin, PluginInfo};

pub struct MyPlugin { enabled: bool }

#[async_trait]
impl Plugin for MyPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "myplugin",
            description: "My custom plugin",
            requires_config: false,
            requires_plugins: &["admin"],
            requires_parsers: &[],
            requires_storage: &[],
            load_after: &[],
        }
    }

    async fn on_event(&self, event: &Event, ctx: &BotContext) -> anyhow::Result<()> {
        // Handle events here
        Ok(())
    }

    fn is_enabled(&self) -> bool { self.enabled }
}
```

Then register it in `main.rs`:

```rust
plugins.register(Box::new(MyPlugin { enabled: true }))?;
```

## Database

Rusty Rules Referee supports **SQLite** and **MySQL** backends. Database migrations run automatically on startup.

### Schema

- **clients** — Player records (GUID, name, IP, permissions, greeting)
- **groups** — Permission levels (Guest, User, Regular, Mod, Admin, SuperAdmin)
- **aliases** — Player name history
- **penalties** — Bans, kicks, warnings with duration and expiry
- **xlr_*** — Extended statistics tables (player stats, weapon stats, map stats, history)
- **admin_users** — Web dashboard admin accounts (bcrypt passwords, roles)
- **audit_log** — Admin action history for accountability
- **chat_messages** — Persisted in-game chat for the live chat panel
- **vote_history** — Callvote records for the vote history panel
- **admin_notes** — Per-admin personal notes

## Credits

Rusty Rules Referee is inspired by [Big Brother Bot](https://github.com/BigBrotherBot/big-brother-bot) originally created by Michael "ThorN" Thornton.

## License

This project is licensed under the [GNU General Public License v2.0 or later](LICENSE) — the same license as the original Big Brother Bot.
