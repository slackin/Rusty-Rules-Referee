# Spree Plugin

Announces killing sprees at configurable milestones with bigtext messages.

**Plugin name:** `spree`
**Requires config:** Yes

## Behavior

- Tracks consecutive kills per player within a round
- Announces spree milestones (5, 10, 15, 20 kills) with bigtext
- Announces when a spree is ended (if the player had ≥ `min_spree` kills)
- Resets on round start and disconnect

## Settings

```toml
[[plugins]]
name = "spree"
enabled = true

[plugins.settings]
min_spree = 5

[plugins.settings.spree_messages]
5 = "{name} is on a KILLING SPREE!"
10 = "{name} is UNSTOPPABLE!"
15 = "{name} is GODLIKE!"
20 = "{name} is LEGENDARY!"
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `min_spree` | integer | `5` | Minimum kills for spree-ended announcement |
| `spree_messages` | table | 4 defaults | Kill count → announcement (use `{name}` for player name) |

## Events

`EVT_CLIENT_KILL`, `EVT_GAME_ROUND_START`, `EVT_CLIENT_DISCONNECT`
