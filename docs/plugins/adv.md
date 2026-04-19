# Adv Plugin

Displays rotating server advertisement messages on round start.

**Plugin name:** `adv`
**Requires config:** Yes

## Behavior

- Cycles through a list of messages
- Sends one message at each round start
- Advances to the next message each round

## Settings

```toml
[[plugins]]
name = "adv"
enabled = true

[plugins.settings]
interval_secs = 120
messages = [
  "^7Welcome to our server! Type ^3!help^7 for commands.",
  "^7Visit our website at ^3example.com",
  "^7Join our Discord: ^3discord.gg/example",
  "^7Report cheaters with ^3!report"
]
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `interval_secs` | integer | `120` | Interval between messages (for timer mode) |
| `messages` | array | 4 defaults | List of rotating advertisement messages |

## Events

`EVT_GAME_ROUND_START`
