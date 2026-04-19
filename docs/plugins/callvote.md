# Callvote Plugin

Controls and filters in-game callvotes. Blocks specific vote types and enforces minimum permission levels.

**Plugin name:** `callvote`
**Requires config:** Yes
**Requires:** iourt43 parser

## Behavior

- Intercepts callvotes before they're processed
- Blocks votes from players below `min_level`
- Blocks specific vote types listed in `blocked_votes`
- Limits votes per player per round
- Vetoes blocked votes automatically

## Settings

```toml
[[plugins]]
name = "callvote"
enabled = true

[plugins.settings]
min_level = 0
max_votes_per_round = 3
blocked_votes = ["kick", "map"]
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `min_level` | integer | `0` | Minimum level required to call votes |
| `max_votes_per_round` | integer | `3` | Max votes per player per round |
| `blocked_votes` | array | `[]` | Vote types to block (e.g., `"kick"`, `"map"`, `"cyclemap"`) |

## Events

`EVT_CLIENT_CALLVOTE`
