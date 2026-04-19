# VPN Check Plugin

Blocks players connecting from known VPN and proxy IP ranges.

**Plugin name:** `vpncheck`
**Requires config:** Yes

## Behavior

- Checks connecting player IPs against a list of blocked ranges
- Kicks players whose IP falls within a blocked range
- IP ranges are specified as start-end pairs in dotted-quad format

## Settings

```toml
[[plugins]]
name = "vpncheck"
enabled = true

[plugins.settings]
kick_reason = "VPN/Proxy connections are not allowed on this server."
blocked_ranges = [
  "10.0.0.0-10.255.255.255",
  "172.16.0.0-172.31.255.255"
]
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `kick_reason` | string | See above | Message shown when kicking VPN users |
| `blocked_ranges` | array | `[]` | IP ranges in `"start-end"` dotted-quad format |

## Events

`EVT_CLIENT_AUTH`
