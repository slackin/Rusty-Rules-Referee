# CustomCommands Plugin

Lets you define custom text commands that respond with configured messages.

**Plugin name:** `customcommands`
**Requires config:** Yes

## Commands

Any command name defined in the `commands` table becomes available. Default examples:

| Command | Response |
|---------|----------|
| `!rules` | Server rules message |
| `!discord` | Discord invite link |

All custom commands are available to everyone (Guest level).

## Settings

```toml
[[plugins]]
name = "customcommands"
enabled = true

[plugins.settings.commands]
rules = "^7Server Rules: No cheating, no racism, play fair!"
discord = "^7Join our Discord: ^3discord.gg/example"
website = "^7Visit us at ^3example.com"
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `commands` | table | `{rules, discord}` | Map of command name → response text |

Add as many commands as you like. The key becomes the command name (prefixed with `!`).

## Events

`EVT_CLIENT_SAY`
