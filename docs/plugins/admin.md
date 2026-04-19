# Admin Plugin

The core administration plugin providing 50+ commands for player and server management.

**Plugin name:** `admin`
**Requires config:** Yes
**Required by:** censor, spamcontrol, tk, login, follow, makeroom, customcommands

## Commands

Commands can be prefixed with `!` (private response), `@` (public response), or `&` (bigtext).

### Guest (Level 0)

| Command | Usage | Description |
|---------|-------|-------------|
| `!help` | `!help` | Lists commands available at your level |
| `!leveltest` | `!leveltest [player]` | Shows your or a player's group and level |
| `!time` | `!time` | Shows current server time (UTC) |
| `!register` | `!register` / `!regme` | Self-register as User (level 1) |
| `!r3` | `!r3` | Shows R3 version information |

### User (Level 1)

| Command | Usage | Description |
|---------|-------|-------------|
| `!regulars` | `!regulars` | Lists online Regular+ players |
| `!rules` | `!rules [player]` | Shows server rules (optionally sends to a target) |

### Moderator (Level 20)

| Command | Usage | Description |
|---------|-------|-------------|
| `!status` | `!status` | Shows connected players with slot, name, ID, and level |
| `!list` | `!list` | Compact connected player list |
| `!lookup` | `!lookup <name>` | Searches database for players by name |
| `!find` | `!find <name>` | Finds connected players by partial name |
| `!admins` | `!admins` | Lists online admins |
| `!warn` | `!warn <player> [reason/keyword]` | Warns a player; auto-kicks at max warnings |
| `!kick` | `!kick <player> [reason]` | Kicks a player from the server |
| `!spank` | `!spank <player> [reason]` | Kicks with a public humiliation message |
| `!seen` | `!seen <name>` | Shows when a player was last online |
| `!aliases` | `!aliases <name or @id>` | Shows a player's known name history |
| `!poke` | `!poke <player>` | Sends an attention message to a player |
| `!notice` | `!notice <player> <text>` | Adds a note to a player's record |
| `!clear` | `!clear <player>` | Clears all warnings and notices |
| `!warns` | `!warns` | Lists available warn keywords |
| `!warntest` | `!warntest <keyword>` | Tests a warn keyword without applying |
| `!warnremove` | `!warnremove <player>` | Removes last warning from a player |
| `!warninfo` | `!warninfo <player>` | Shows active warning count |

### Admin (Level 40)

| Command | Usage | Description |
|---------|-------|-------------|
| `!tempban` | `!tempban <player> [duration] [reason]` | Temporarily bans (duration: Nm/Nh/Nd/Nw, default 2h) |
| `!lastbans` | `!lastbans` | Shows 5 most recent bans |
| `!baninfo` | `!baninfo <player>` | Shows ban count for a player |
| `!spam` | `!spam <keyword> [player]` | Sends a predefined spam message |
| `!spams` | `!spams` | Lists available spam keywords |
| `!clientinfo` | `!clientinfo <name or @id>` | Detailed client info (IP, GUID, level) |

### Senior Admin (Level 60)

| Command | Usage | Description |
|---------|-------|-------------|
| `!ban` | `!ban <player> [reason]` | Permanently bans a player |
| `!permban` | `!permban <player> [reason]` | Permanent ban with explicit message |
| `!unban` | `!unban <name or @id>` | Unbans a player |
| `!say` | `!say <message>` | Public server message |
| `!scream` | `!scream <message>` | Bigtext message to entire server |
| `!longlist` | `!longlist` | Detailed player list with IPs |
| `!warnclear` | `!warnclear <player>` | Clears all warnings |
| `!kickall` | `!kickall <pattern> [reason]` | Kicks all players matching name pattern |
| `!banall` | `!banall <pattern> [reason]` | Bans all players matching name pattern |
| `!spankall` | `!spankall <pattern> [reason]` | Spanks all matching players |
| `!mask` | `!mask [player] [level]` | Masks a player's level (appear as lower) |
| `!unmask` | `!unmask [player]` | Removes level mask |
| `!makereg` | `!makereg <player>` | Promotes player to Regular group |
| `!unreg` | `!unreg <player>` | Demotes player to User |

### Super Admin (Level 80)

| Command | Usage | Description |
|---------|-------|-------------|
| `!putgroup` | `!putgroup <player> <group>` | Sets player's group |
| `!ungroup` | `!ungroup <player>` | Removes player from all groups |
| `!map` | `!map <mapname>` | Changes to a specific map |
| `!maps` | `!maps` | Lists available maps |
| `!nextmap` | `!nextmap` | Shows next map in rotation |
| `!maprotate` | `!maprotate` | Cycles to next map |
| `!die` | `!die` | Shuts down R3 |
| `!restart` | `!restart` | Restarts R3 |
| `!reconfig` | `!reconfig` | Reloads configuration |
| `!pause` | `!pause` | Pauses log parsing |
| `!rebuild` | `!rebuild` | Re-syncs client list from server |
| `!runas` | `!runas <player> <command>` | Runs a command as another player |
| `!iamgod` | `!iamgod` | Promotes self to Super Admin (only when none exist) |

## Settings

```toml
[[plugins]]
name = "admin"
enabled = true

[plugins.settings]
warn_reason = "Server Rule Violation"
max_warnings = 3

[plugins.settings.warn_reasons]
spam = { duration = "1h", reason = "Stop spamming" }
lang = { duration = "2h", reason = "Watch your language" }
rage = { duration = "30m", reason = "No rage quitting" }
tk = { duration = "1h", reason = "Stop team killing" }
camp = { duration = "30m", reason = "No camping" }
afk = { duration = "30m", reason = "AFK - not playing" }

[plugins.settings.spam_messages]
rules = "^7Server rules: No cheating, no racism, no excessive TK. Type ^3!rules^7 for full list."
website = "^7Visit our website at ^3example.com"
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `warn_reason` | string | `"Server Rule Violation"` | Default warn reason when no keyword given |
| `max_warnings` | integer | `3` | Warnings before automatic kick |
| `warn_reasons` | table | 6 defaults | Keyword → `{duration, reason}` for `!warn <keyword>` |
| `spam_messages` | table | 2 defaults | Keyword → message for `!spam <keyword>` |
| `rules` | array | 5 defaults | Server rules displayed by `!rules` |

## Events

`EVT_CLIENT_SAY`, `EVT_CLIENT_TEAM_SAY`, `EVT_CLIENT_PRIVATE_SAY`, `EVT_CLIENT_AUTH`
