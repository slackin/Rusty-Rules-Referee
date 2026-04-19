# CountryFilter Plugin

Filters players by country using GeoIP lookup. Supports allowlist and blocklist modes.

**Plugin name:** `countryfilter`
**Requires config:** Yes

## Behavior

- Looks up connecting player's country by IP address
- In `blocklist` mode: kicks players from listed countries
- In `allowlist` mode: kicks players NOT from listed countries

## Settings

```toml
[[plugins]]
name = "countryfilter"
enabled = true

[plugins.settings]
mode = "blocklist"
countries = ["XX", "YY"]
kick_message = "Your country is not allowed on this server."
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `mode` | string | `"blocklist"` | `"allowlist"` or `"blocklist"` |
| `countries` | array | `[]` | ISO 3166-1 alpha-2 country codes |
| `kick_message` | string | See above | Message shown when kicking |

## Events

`EVT_CLIENT_CONNECT`
