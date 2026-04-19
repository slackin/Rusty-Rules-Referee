# MapConfig Plugin

Executes map-specific RCON command sequences on map change.

**Plugin name:** `mapconfig`
**Requires config:** Yes

## Behavior

- Watches for map change events
- Looks up the new map name in the configuration
- Executes the list of RCON commands associated with that map
- Useful for map-specific settings (gravity, gear restrictions, time limits, etc.)

## Settings

```toml
[[plugins]]
name = "mapconfig"
enabled = true

[plugins.settings.map_configs]
ut4_turnpike = ["set g_gear 0", "set timelimit 20"]
ut4_abbey = ["set g_gear 63", "set timelimit 15"]
ut4_riyadh = ["set g_gravity 600"]
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `map_configs` | table | `{}` | Map name → array of RCON commands to execute |

## Events

`EVT_GAME_MAP_CHANGE`
