# SpawnKill Plugin

Detects and punishes spawn killing with a configurable grace period after spawning.

**Plugin name:** `spawnkill`
**Requires config:** Yes
**Requires:** iourt43 parser

## Behavior

- Tracks when each player spawns
- If a player is killed within the grace period, it counts as a spawn kill
- After reaching the threshold, takes the configured action (warn, kick, or tempban)
- Resets counters on round start

## Settings

```toml
[[plugins]]
name = "spawnkill"
enabled = true

[plugins.settings]
grace_period_secs = 3
max_spawnkills = 3
action = "warn"
tempban_duration = 5
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `grace_period_secs` | integer | `3` | Seconds after spawn that count as spawn kill window |
| `max_spawnkills` | integer | `3` | Spawn kills before taking action |
| `action` | string | `"warn"` | Action to take: `"warn"`, `"kick"`, or `"tempban"` |
| `tempban_duration` | integer | `5` | Tempban duration in minutes (when action is `"tempban"`) |

## Events

`EVT_CLIENT_SPAWN`, `EVT_CLIENT_KILL`, `EVT_GAME_ROUND_START`, `EVT_CLIENT_DISCONNECT`
