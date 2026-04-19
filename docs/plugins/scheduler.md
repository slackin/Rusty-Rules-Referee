# Scheduler Plugin

Runs configured actions (RCON commands or say messages) triggered by game events.

**Plugin name:** `scheduler`
**Requires config:** Yes

## Behavior

- Executes tasks when specific game events occur
- Each task maps an event trigger to an action
- Supports `"say"` (public message) and `"rcon"` (RCON command) action types
- Subscribes dynamically to whatever events are configured

## Settings

```toml
[[plugins]]
name = "scheduler"
enabled = true

[[plugins.settings.tasks]]
event_trigger = "EVT_GAME_ROUND_START"
action_type = "say"
action_value = "^7Round started! Good luck!"

[[plugins.settings.tasks]]
event_trigger = "EVT_GAME_MAP_CHANGE"
action_type = "rcon"
action_value = "set g_friendlyfire 1"
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `tasks` | array of tables | `[]` | Each task has `event_trigger`, `action_type`, and `action_value` |

### Task Fields

| Field | Type | Description |
|-------|------|-------------|
| `event_trigger` | string | Event name (e.g., `"EVT_GAME_ROUND_START"`, `"EVT_GAME_MAP_CHANGE"`) |
| `action_type` | string | `"say"` for public message, `"rcon"` for RCON command |
| `action_value` | string | The message or command to execute |

## Events

Dynamic — subscribes to whatever events are configured in tasks.
