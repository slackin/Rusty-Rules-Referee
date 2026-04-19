# CensorUrt Plugin

Urban Terror specific name and clan tag censoring with pre-built patterns for common offensive terms. Strips Quake 3 color codes before checking.

**Plugin name:** `censorurt`
**Requires config:** Yes

## Behavior

- Checks player names on authentication and name change
- Strips color codes (`^0`–`^9`) before matching
- Kicks players with banned names

## Settings

```toml
[[plugins]]
name = "censorurt"
enabled = true

[plugins.settings]
banned_names = [
  "(?i)n[i1]gg",
  "(?i)f[a@]gg",
  "(?i)nazi",
  "(?i)hitler",
  "(?i)\\bkk+k\\b"
]
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `banned_names` | array | 5 default patterns | Regex patterns for banned names (slurs, hate symbols) |

## Events

`EVT_CLIENT_AUTH`, `EVT_CLIENT_NAME_CHANGE`
