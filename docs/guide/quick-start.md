# Quick Start

This guide walks you through getting R3 running on your Urban Terror 4.3 server.

## 1. Prepare Your Game Server

Ensure your Urban Terror server has RCON enabled and log syncing turned on. In your `server.cfg`:

```cfg
set rcon_password "your_secret_rcon_password"
set g_log "games.log"
set g_logsync 1
```

The `g_logsync 1` setting is **required** — it forces the server to flush log lines immediately so R3 can read them in real-time.

## 2. Create Configuration

Copy the example configuration and edit it:

```bash
cp referee.example.toml referee.toml
```

Edit `referee.toml` with your server details:

```toml
[referee]
bot_name = "R3"
bot_prefix = "^2R3:^3"
database = "sqlite://r3.db"
logfile = "r3.log"
log_level = "info"

[server]
public_ip = "192.168.1.100"      # Your server's IP
port = 27960                       # Your server's port
rcon_password = "your_secret_rcon_password"
game_log = "/path/to/games.log"   # Full path to the game server's log
delay = 0.33                       # Log poll interval in seconds
```

## 3. Enable Plugins

Add plugins to your config. Start with the essentials:

```toml
[[plugins]]
name = "admin"
enabled = true

[[plugins]]
name = "welcome"
enabled = true

[[plugins]]
name = "spamcontrol"
enabled = true

[[plugins]]
name = "tk"
enabled = true

[[plugins]]
name = "stats"
enabled = true
```

See [Plugin Overview](/plugins/) for the full list of 30 available plugins.

## 4. Run R3

```bash
./rusty-rules-referee referee.toml
```

R3 will:
1. Connect to the database (creating it if using SQLite)
2. Run any pending migrations
3. Start tailing the game server log
4. Begin parsing events and dispatching to plugins

You should see output like:

```
INFO  Starting Rusty Rules Referee v2.0.0
INFO  Database connected: sqlite://r3.db
INFO  Loaded plugin: admin
INFO  Loaded plugin: welcome
INFO  Loaded plugin: spamcontrol
INFO  Loaded plugin: tk
INFO  Loaded plugin: stats
INFO  Tailing log: /path/to/games.log
```

## 5. Verify It Works

Join your game server and type in chat:

```
!help
```

R3 should respond with a list of available commands for your permission level.

To promote yourself to Super Admin (first time only):

```
!iamgod
```

This command only works when no Super Admins exist in the database.

## 6. Enable the Web Dashboard (Optional)

Add a `[web]` section to your config:

```toml
[web]
enabled = true
bind_address = "0.0.0.0"
port = 8080
```

The dashboard will be available at `http://your-server-ip:8080`. Default login: `admin` / `changeme`.

::: warning
Change the default password immediately after first login.
:::

## Running as a Background Service

### Using screen (simple)

```bash
screen -dmS r3 ./rusty-rules-referee referee.toml
```

Reattach with `screen -r r3`.

### Using systemd (recommended for production)

Create `/etc/systemd/system/r3.service`:

```ini
[Unit]
Description=Rusty Rules Referee
After=network.target

[Service]
Type=simple
User=gameserver
WorkingDirectory=/home/gameserver/r3
ExecStart=/home/gameserver/r3/rusty-rules-referee referee.toml
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl enable r3
sudo systemctl start r3
```

## Next Steps

- [Configuration Reference](/guide/configuration) — All configuration options explained
- [Plugins](/plugins/) — Explore all 30 plugins
- [Command Reference](/commands/) — Every available command
