# XLRstats Plugin

Extended Live Rankings and Statistics — persistent ELO-based skill tracking with weapon stats, stored in the database.

**Plugin name:** `xlrstats`
**Requires config:** Yes

## Commands

| Command | Level | Usage | Description |
|---------|-------|-------|-------------|
| `!xlrstats` / `!xlr` | 0 | `!xlrstats` | Shows your XLR stats (kills, deaths, ratio, skill rating) |
| `!xlrtopstats` | 0 | `!xlrtopstats` | Shows top ranked players |

## Behavior

- Tracks kills, deaths, assists, headshots, and team kills per player
- Calculates ELO-style skill ratings that adjust based on opponent skill
- Records weapon-specific statistics
- All data persisted to database (`xlr_playerstats`, `xlr_weaponstats` tables)
- Stats are only displayed after `min_kills` threshold is reached

## Settings

```toml
[[plugins]]
name = "xlrstats"
enabled = true

[plugins.settings]
kill_bonus = 1.2
assist_bonus = 0.5
min_kills = 50
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `kill_bonus` | float | `1.2` | Kill bonus multiplier for skill calculation |
| `assist_bonus` | float | `0.5` | Assist bonus for skill calculation |
| `min_kills` | integer | `50` | Minimum kills before stats are displayed |

## Events

`EVT_CLIENT_KILL`, `EVT_CLIENT_SAY`, `EVT_CLIENT_TEAM_SAY`, `EVT_GAME_ROUND_START`
