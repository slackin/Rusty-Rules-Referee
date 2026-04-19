# NameChecker Plugin

Monitors and enforces player name policies including duplicate detection, forbidden name patterns, and name change rate limiting.

**Plugin name:** `namechecker`
**Requires:** iourt43 parser

## Behavior

- Checks names on auth and name change against forbidden patterns
- Detects duplicate names among connected players
- Rate-limits name changes per time window
- Kicks players violating name policies

## Settings

```toml
[[plugins]]
name = "namechecker"
enabled = true

[plugins.settings]
max_name_changes = 5
name_change_window = 300
check_duplicates = true
forbidden_patterns = ["^\\s*$", "^player$", "^unnamed\\s*player$", "^newplayer$"]
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `max_name_changes` | integer | `5` | Max name changes per time window |
| `name_change_window` | integer | `300` | Time window in seconds (5 min) |
| `check_duplicates` | bool | `true` | Check for duplicate names |
| `forbidden_patterns` | array | 4 defaults | Regex patterns for forbidden names |

## Events

`EVT_CLIENT_AUTH`, `EVT_CLIENT_NAME_CHANGE`, `EVT_CLIENT_DISCONNECT`
