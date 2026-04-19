# ChatLogger Plugin

Logs all chat messages to daily rotating log files and persists them to the database.

**Plugin name:** `chatlogger`

## Behavior

- Captures all chat messages (public, team, and private)
- Writes to daily log files with timestamps and player info
- Also stores messages in the `chat_messages` database table
- Auto-creates the log directory if it doesn't exist

## Settings

```toml
[[plugins]]
name = "chatlogger"
enabled = true

[plugins.settings]
log_dir = "chat_logs"
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `log_dir` | string | `"chat_logs"` | Directory for daily chat log files |

## Events

`EVT_CLIENT_SAY`, `EVT_CLIENT_TEAM_SAY`, `EVT_CLIENT_PRIVATE_SAY`
