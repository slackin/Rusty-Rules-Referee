# Team Kill (TK) Plugin

Monitors team kills and team damage, automatically penalizing excessive team killers.

**Plugin name:** `tk`
**Requires config:** Yes
**Requires:** admin plugin

## Commands

| Command | Alias | Level | Description |
|---------|-------|-------|-------------|
| `!forgive` | `!f` | 0 | Forgive the last person who team killed you |
| `!forgivelist` | `!fl` | 0 | List all unforgiven TKs against you |
| `!forgiveall` | `!fa` | 0 | Forgive all TKs against you |
| `!forgiveinfo` | `!fi` | 0 | Show your unforgiven TK count and kicks remaining |
| `!forgiveprev` | `!fp` | 0 | Show who last team killed you |
| `!forgiveclear` | `!fc` | 20 | Admin: clear TK records for a player |

## Behavior

- Tracks team kills and team damage per player per round
- Resets counters at round start
- Auto-kicks when unforgiven TK threshold is exceeded
- Victims are notified and can `!forgive` to prevent penalties
- Forgiven TKs do not count toward the kick threshold

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

`EVT_CLIENT_KILL_TEAM`, `EVT_CLIENT_DAMAGE_TEAM`, `EVT_CLIENT_SAY`, `EVT_CLIENT_TEAM_SAY`, `EVT_GAME_ROUND_START`
