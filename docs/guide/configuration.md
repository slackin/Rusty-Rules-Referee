# Configuration Reference

R3 uses a TOML configuration file. All settings are documented below.

## `[referee]` Section

Core bot settings.

```toml
[referee]
bot_name = "R3"
bot_prefix = "^2R3:^3"
database = "sqlite://r3.db"
logfile = "r3.log"
log_level = "info"
```

| Setting | Type | Required | Default | Description |
|---------|------|----------|---------|-------------|
| `bot_name` | string | Yes | — | Bot's display name in-game |
| `bot_prefix` | string | No | `"^2RRR:^3"` | Color-coded prefix for bot messages (uses Quake 3 color codes) |
| `database` | string | Yes | — | Database connection string (see below) |
| `logfile` | string | Yes | — | Path to R3's own log file |
| `log_level` | string | No | `"info"` | Log verbosity: `error`, `warn`, `info`, `debug`, `trace` |

### Database Connection Strings

**SQLite** (recommended for single-server setups):
```toml
database = "sqlite://r3.db"           # Relative path
database = "sqlite:///var/lib/r3/r3.db"  # Absolute path
```

**MySQL** (for multi-server or high-volume setups):
```toml
database = "mysql://user:password@localhost:3306/r3"
```

### Quake 3 Color Codes

| Code | Color |
|------|-------|
| `^0` | Black |
| `^1` | Red |
| `^2` | Green |
| `^3` | Yellow |
| `^4` | Blue |
| `^5` | Cyan |
| `^6` | Magenta |
| `^7` | White |

## `[server]` Section

Game server connection settings.

```toml
[server]
public_ip = "192.168.1.100"
port = 27960
# rcon_ip = "192.168.1.100"    # Optional: separate RCON IP
# rcon_port = 27960             # Optional: separate RCON port
rcon_password = "your_rcon_password"
game_log = "/home/gameserver/.q3a/q3ut4/games.log"
delay = 0.33
```

| Setting | Type | Required | Default | Description |
|---------|------|----------|---------|-------------|
| `public_ip` | string | Yes | — | Game server's IP address |
| `port` | integer | Yes | — | Game server's port |
| `rcon_ip` | string | No | `public_ip` | RCON IP (if different from game server) |
| `rcon_port` | integer | No | `port` | RCON port (if different from game port) |
| `rcon_password` | string | Yes | — | RCON password (must match `rcon_password` in server.cfg) |
| `game_log` | string | Yes | — | Full path to the game server's `games.log` file |
| `delay` | float | No | `0.33` | Log polling interval in seconds |

::: tip
Set `delay` lower (e.g., `0.1`) for faster response times, or higher (e.g., `1.0`) to reduce CPU usage. The default `0.33` (3 checks/second) is a good balance.
:::

## `[web]` Section

Web dashboard configuration. Omit this section entirely to disable the dashboard.

```toml
[web]
enabled = true
bind_address = "0.0.0.0"
port = 8080
# jwt_secret = "your-secret-key"
```

| Setting | Type | Required | Default | Description |
|---------|------|----------|---------|-------------|
| `enabled` | bool | No | `false` | Enable/disable the web dashboard |
| `bind_address` | string | No | `"0.0.0.0"` | Address to bind the web server to |
| `port` | integer | No | `8080` | Web server port |
| `jwt_secret` | string | No | auto-generated | Secret key for JWT token signing |

::: warning
If you don't set `jwt_secret`, a random one is generated on each startup, which will invalidate all existing sessions. Set a fixed value for production use.
:::

## `[[plugins]]` Array

Each plugin is configured as a TOML array entry. All plugins share the same base structure:

```toml
[[plugins]]
name = "plugin_name"
enabled = true
# config_file = "path/to/external/config.toml"  # Optional

[plugins.settings]
# Plugin-specific settings go here
```

| Setting | Type | Required | Default | Description |
|---------|------|----------|---------|-------------|
| `name` | string | Yes | — | Plugin identifier (see [Plugin Overview](/plugins/)) |
| `enabled` | bool | No | `true` | Enable or disable this plugin |
| `config_file` | string | No | — | Path to an external config file for this plugin |

### Plugin Settings

Each plugin has its own `[plugins.settings]` table. See individual plugin pages for their settings:

- [Admin](/plugins/admin) — warn reasons, max warnings, spam messages, rules
- [PowerAdminUrt](/plugins/poweradminurt) — team balance, radio spam protection
- [Censor](/plugins/censor) — bad words/names regex patterns
- [SpamControl](/plugins/spamcontrol) — flood detection thresholds
- [TK](/plugins/tk) — team kill/damage limits
- And [25 more...](/plugins/)

## Complete Example

```toml
[referee]
bot_name = "R3"
bot_prefix = "^2R3:^3"
database = "sqlite://r3.db"
logfile = "r3.log"
log_level = "info"

[server]
public_ip = "10.10.0.2"
port = 25000
rcon_password = "your_rcon_password"
game_log = "/home/gameserver/.q3a/q3ut4/games.log"
delay = 0.33

[web]
enabled = true
bind_address = "0.0.0.0"
port = 8080

[[plugins]]
name = "admin"
enabled = true
[plugins.settings]
warn_reason = "Server Rule Violation"
max_warnings = 3

[[plugins]]
name = "welcome"
enabled = true

[[plugins]]
name = "spamcontrol"
enabled = true
[plugins.settings]
max_messages = 5
time_window_secs = 10

[[plugins]]
name = "tk"
enabled = true
[plugins.settings]
max_team_kills = 5

[[plugins]]
name = "stats"
enabled = true

[[plugins]]
name = "xlrstats"
enabled = true

[[plugins]]
name = "pingwatch"
enabled = true
[plugins.settings]
max_ping = 250
max_warnings = 3
```
