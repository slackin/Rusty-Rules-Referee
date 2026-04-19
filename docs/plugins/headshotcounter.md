# HeadshotCounter Plugin

Tracks headshots per player, announces headshot streaks, and can auto-ban suspected aimbotters based on headshot ratio.

**Plugin name:** `headshotcounter`
**Requires:** iourt43 parser

## Commands

| Command | Level | Usage | Description |
|---------|-------|-------|-------------|
| `!hs` / `!headshots` | 0 | `!hs [player]` | Shows headshot stats (kills, HS count, ratio, best streak) |

## Behavior

- Tracks headshots per player per map
- Announces headshot streaks every `announce_interval` headshots
- Warns players exceeding `warn_ratio` headshot percentage
- Auto-tempbans players exceeding `ban_ratio` (after `min_kills`)
- Resets on map change

## Settings

```toml
[[plugins]]
name = "headshotcounter"
enabled = true

[plugins.settings]
warn_ratio = 0.70
ban_ratio = 0.85
min_kills = 15
ban_duration = 60
announce_interval = 10
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `warn_ratio` | float | `0.70` | HS ratio threshold for warning (70%) |
| `ban_ratio` | float | `0.85` | HS ratio threshold for auto-tempban (85%) |
| `min_kills` | integer | `15` | Minimum kills before ratio checks apply |
| `ban_duration` | integer | `60` | Tempban duration in minutes |
| `announce_interval` | integer | `10` | Announce streak every N headshots |

## Events

`EVT_CLIENT_KILL`, `EVT_CLIENT_SAY`, `EVT_CLIENT_TEAM_SAY`, `EVT_GAME_MAP_CHANGE`, `EVT_GAME_EXIT`, `EVT_CLIENT_DISCONNECT`
