# Stats Plugin

Tracks in-memory player kill/death statistics per round with `!stats` and `!topstats` commands.

**Plugin name:** `stats`
**Requires config:** No

## Commands

| Command | Level | Usage | Description |
|---------|-------|-------|-------------|
| `!stats` | 0 | `!stats` | Shows your kills, deaths, K/D ratio, and team kills |
| `!topstats` | 0 | `!topstats` | Shows top 5 players by kills (public message) |

## Behavior

- Tracks kills, deaths, and team kills per player in memory
- Statistics are for the current session only (not persisted)
- `!topstats` sends results as a public message visible to all players

## Settings

None — no configuration required.

## Events

`EVT_CLIENT_KILL`, `EVT_CLIENT_KILL_TEAM`, `EVT_CLIENT_SAY`, `EVT_CLIENT_TEAM_SAY`
