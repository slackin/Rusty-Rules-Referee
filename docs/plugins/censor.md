# Censor Plugin

Filters offensive language from chat messages and player names using configurable regex patterns.

**Plugin name:** `censor`
**Requires config:** Yes
**Requires:** admin plugin

## Behavior

- Scans all chat messages against `bad_words` patterns
- Scans player names against `bad_names` patterns on name change
- Warns offending players; auto-kicks after `max_warnings`

## Settings

```toml
[[plugins]]
name = "censor"
enabled = true

[plugins.settings]
warn_message = "Watch your language!"
max_warnings = 3
bad_words = ["\\b(badword1|badword2)\\b"]
bad_names = ["offensive.*pattern"]
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `warn_message` | string | `"Watch your language!"` | Warning message shown to offenders |
| `max_warnings` | integer | `3` | Warnings before auto-kick |
| `bad_words` | array | `[]` | Regex patterns for banned chat words |
| `bad_names` | array | `[]` | Regex patterns for banned player names |

## Events

`EVT_CLIENT_SAY`, `EVT_CLIENT_TEAM_SAY`, `EVT_CLIENT_NAME_CHANGE`
