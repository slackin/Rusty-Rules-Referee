# NickReg Plugin

Nickname registration and protection. Tracks registered nicknames tied to client database IDs and kicks impostors using someone else's registered name.

**Plugin name:** `nickreg`
**Requires config:** Yes

## Behavior

- When a player authenticates, checks if their name is registered to another client
- If the name belongs to someone else, warns or kicks the impostor
- Names are linked to players via their database ID/GUID

## Settings

```toml
[[plugins]]
name = "nickreg"
enabled = true

[plugins.settings]
warn_before_kick = true
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `warn_before_kick` | bool | `true` | Warn impostors before kicking (false = kick immediately) |

## Events

`EVT_CLIENT_AUTH`
