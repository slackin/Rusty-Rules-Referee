# PingWatch Plugin

Monitors player pings and kicks players with consistently high ping.

**Plugin name:** `pingwatch`

## Behavior

- Periodically polls player pings
- Warns players when their ping exceeds `max_ping`
- Resets warnings when ping drops below `warn_threshold`
- Kicks after `max_warnings` consecutive high-ping checks

## Settings

```toml
[[plugins]]
name = "pingwatch"
enabled = true

[plugins.settings]
max_ping = 250
warn_threshold = 200
max_warnings = 3
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `max_ping` | integer | `250` | Maximum allowed ping (ms) |
| `warn_threshold` | integer | `200` | Ping below which warnings reset |
| `max_warnings` | integer | `3` | Warnings before kick |

## Events

Timer-based — does not subscribe to specific events.
