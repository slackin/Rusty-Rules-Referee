# Web Dashboard

R3 includes a built-in web dashboard for server management and live monitoring. The frontend is a SvelteKit application embedded into the Rust binary — no separate web server needed.

## Enabling the Dashboard

Add a `[web]` section to your `referee.toml`:

```toml
[web]
enabled = true
bind_address = "0.0.0.0"
port = 8080
# jwt_secret = "your-secret-key"
```

The dashboard will be available at `http://your-server-ip:8080`.

## Default Credentials

- **Username:** `admin`
- **Password:** `changeme`

::: warning
Change the default password immediately after first login via the Admin Users page.
:::

## Features

### Live Scoreboard
Real-time player list with Red/Blue team grouping, scores, and ping. Updates via WebSocket.

### Live Chat
In-game chat messages streamed in real-time with team/all channel badges.

### Dashboard Stats
Six stat cards showing: online players, current map, game type, server uptime, active warnings, and total bans.

### Vote History
Track callvotes with player name, vote type, data, and timestamp.

### Personal Notes
Per-admin notepad persisted in the database. Accessible from the dashboard sidebar.

### RCON Console
Send RCON commands directly from the dashboard. All commands are audit-logged.

### Player Management
- View all connected players with their groups, IPs, and GUIDs
- Click a player for detailed info: aliases, penalties, XLR stats, gear loadout
- Kick, ban, or change a player's group from the UI

### XLRstats Leaderboards
View top players by skill rating, with weapon and map statistics.

### Audit Log
Complete history of all admin actions taken through the dashboard (RCON commands, kicks, bans, config changes).

### Admin Users
Manage dashboard user accounts. Create, edit, and delete admin users. Change passwords and roles.

### Configuration
View and edit the R3 configuration directly from the dashboard (admin-only). Changes are written to disk and applied on restart.

## Architecture

The web dashboard consists of:

- **Backend:** Axum HTTP server running inside the R3 process
- **Frontend:** SvelteKit 2 (Svelte 5) with Tailwind CSS, compiled to static files
- **Real-time:** WebSocket connection for live events (player joins, kills, chat, votes)
- **Auth:** JWT-based authentication with role-based access control

The frontend is embedded into the Rust binary via `rust_embed` at compile time. No separate web server or Node.js runtime is required in production.

## Security Notes

- All API endpoints require JWT authentication
- Admin-only endpoints have additional role checks
- RCON commands and config changes are recorded in the audit log
- Set a fixed `jwt_secret` in production to persist sessions across restarts
- Consider placing the dashboard behind a reverse proxy with TLS for production use
