# SpamControl Plugin

Prevents chat flooding and message repetition by tracking message frequency per player.

**Plugin name:** `spamcontrol`
**Requires config:** Yes
**Requires:** admin plugin

## Behavior

- Tracks messages per player within a time window
- Detects repeated identical messages
- Warns then kicks chronic spammers

## Settings

```toml
[[plugins]]
name = "spamcontrol"
enabled = true

[plugins.settings]
max_messages = 5
time_window_secs = 10
max_repeats = 3
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `max_messages` | integer | `5` | Max messages in time window before warning |
| `time_window_secs` | integer | `10` | Time window for flood detection (seconds) |
| `max_repeats` | integer | `3` | Max identical consecutive messages allowed |

## Events

`EVT_CLIENT_SAY`, `EVT_CLIENT_TEAM_SAY`
