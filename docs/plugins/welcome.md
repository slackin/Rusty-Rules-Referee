# Welcome Plugin

Greets new and returning players with customizable messages.

**Plugin name:** `welcome`
**Requires config:** No (optional settings)

## Behavior

- Detects first-time players via database lookup
- Sends different messages to new vs. returning players
- Supports template variables in messages

## Template Variables

| Variable | Description |
|----------|-------------|
| `$name` | Player's current name |
| `$last_visit` | When the player was last seen |

## Settings

```toml
[[plugins]]
name = "welcome"
enabled = true

[plugins.settings]
new_player_message = "^7Welcome to the server, ^2$name^7! Type ^3!help^7 for commands."
returning_player_message = "^7Welcome back, ^2$name^7! You were last seen ^3$last_visit^7."
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `new_player_message` | string | See above | Message for first-time players |
| `returning_player_message` | string | See above | Message for returning players |

## Events

`EVT_CLIENT_AUTH`
