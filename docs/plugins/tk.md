# Team Kill (TK) Plugin

Monitors team kills and team damage, automatically penalizing excessive team killers.

**Plugin name:** `tk`
**Requires config:** Yes
**Requires:** admin plugin

## Behavior

- Tracks team kills and team damage per player per round
- Resets counters at round start
- Auto-kicks when thresholds are exceeded

## Settings

```toml
[[plugins]]
name = "tk"
enabled = true

[plugins.settings]
max_team_kills = 5
max_team_damage = 300.0
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `max_team_kills` | integer | `5` | Team kills before auto-kick |
| `max_team_damage` | float | `300.0` | Cumulative team damage threshold before auto-kick |

## Events

`EVT_CLIENT_KILL_TEAM`, `EVT_CLIENT_DAMAGE_TEAM`, `EVT_GAME_ROUND_START`
