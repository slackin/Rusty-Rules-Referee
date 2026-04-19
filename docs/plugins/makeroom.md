# MakeRoom Plugin

Automatically kicks the lowest-level non-admin player to make room when an admin joins a full server.

**Plugin name:** `makeroom`
**Requires config:** Yes

## Behavior

- Triggers when a player with level ≥ `min_admin_level` authenticates
- Checks if the server is at `max_players` capacity
- Kicks the connected player with the lowest permission level (never kicks admins)

## Settings

```toml
[[plugins]]
name = "makeroom"
enabled = true

[plugins.settings]
min_admin_level = 20
max_players = 32
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `min_admin_level` | integer | `20` | Minimum level that triggers room-making |
| `max_players` | integer | `32` | Server max player slots |

## Events

`EVT_CLIENT_AUTH`
