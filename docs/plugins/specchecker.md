# SpecChecker Plugin

Kicks idle spectators when the server is busy to free up player slots.

**Plugin name:** `specchecker`
**Requires:** iourt43 parser

## Behavior

- Tracks how long each player has been in spectator mode
- Warns spectators at intervals before kicking
- Only enforces when `min_players` are connected
- Players at or above `immune_level` are exempt

## Settings

```toml
[[plugins]]
name = "specchecker"
enabled = true

[plugins.settings]
max_spec_time = 300
min_players = 8
warn_interval = 60
immune_level = 20
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `max_spec_time` | integer | `300` | Max spectate time in seconds (5 min) |
| `min_players` | integer | `8` | Minimum connected players before enforcement |
| `warn_interval` | integer | `60` | Warning interval in seconds |
| `immune_level` | integer | `20` | Level at which players are immune (Mod+) |

## Events

`EVT_CLIENT_TEAM_CHANGE`, `EVT_CLIENT_TEAM_CHANGE2`, `EVT_CLIENT_JOIN`, `EVT_CLIENT_AUTH`, `EVT_CLIENT_DISCONNECT`, `EVT_GAME_ROUND_START`, `EVT_GAME_ROUND_END`
