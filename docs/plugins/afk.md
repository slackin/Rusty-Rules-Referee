# AFK Plugin

Detects and handles AFK (away from keyboard) players by monitoring activity.

**Plugin name:** `afk`
**Requires config:** Yes

## Behavior

- Tracks last activity time per player (chat, kills, damage)
- Periodically checks for inactive players
- Only enforces when `min_players` are connected
- Can move to spectator first or kick directly

## Settings

```toml
[[plugins]]
name = "afk"
enabled = true

[plugins.settings]
afk_threshold_secs = 300
min_players = 4
check_interval_secs = 60
move_to_spec = true
afk_message = "^7AFK: You have been inactive too long"
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `afk_threshold_secs` | integer | `300` | Seconds of inactivity before considered AFK |
| `min_players` | integer | `4` | Minimum player count before AFK enforcement activates |
| `check_interval_secs` | integer | `60` | How often to check for AFK players (seconds) |
| `move_to_spec` | bool | `true` | Move to spectator first (false = kick directly) |
| `afk_message` | string | See above | Message shown to AFK players |

## Events

`EVT_CLIENT_SAY`, `EVT_CLIENT_TEAM_SAY`, `EVT_CLIENT_KILL`, `EVT_CLIENT_DAMAGE`, `EVT_CLIENT_DISCONNECT`, `EVT_GAME_ROUND_START`
