# Login Plugin

Requires high-level admins to authenticate with a password before their admin commands become active.

**Plugin name:** `login`
**Requires config:** Yes
**Requires:** admin plugin

## Commands

| Command | Level | Usage | Description |
|---------|-------|-------|-------------|
| `!login` | 0 | `!login <password>` | Authenticates with your admin password |
| `!setpassword` | 0 | `!setpassword <password>` | Sets or changes your admin password (min 4 characters) |

## Behavior

- Players at or above `min_level` must `!login` before any admin commands work
- Password is set per-player using `!setpassword`
- Login status is cleared on disconnect

## Settings

```toml
[[plugins]]
name = "login"
enabled = true

[plugins.settings]
min_level = 20
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `min_level` | integer | `20` | Minimum level that must login before using admin commands |

## Events

`EVT_CLIENT_SAY`, `EVT_CLIENT_TEAM_SAY`, `EVT_CLIENT_DISCONNECT`
