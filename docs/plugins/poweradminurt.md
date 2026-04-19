# PowerAdminUrt Plugin

Urban Terror 4.3 specific administration commands for server management, team control, and game settings.

**Plugin name:** `poweradminurt`
**Requires config:** Yes
**Requires:** admin plugin, iourt43 parser

## Commands

All commands also work with a `pa` prefix (e.g., `!paslap`, `!panuke`).

### Moderator (Level 20)

| Command | Usage | Description |
|---------|-------|-------------|
| `!ident` | `!ident [player]` / `!id [player]` | Shows player @id (full info at Senior+ level) |

### Admin (Level 40)

| Command | Usage | Description |
|---------|-------|-------------|
| `!slap` | `!slap <player> [count]` | Slaps a player (max 25 times) |
| `!nuke` | `!nuke <player>` | Nukes a player |
| `!mute` | `!mute <player> [seconds]` | Mutes a player (default 60s) |
| `!kill` | `!kill <player>` | Smites/kills a player |
| `!force` | `!force <player> <team>` | Forces player to red/blue/spec/free |
| `!poke` | `!poke <player> [message]` | Triple-message poke |
| `!swap` | `!swap <p1> [p2]` | Swaps two players' teams |
| `!swap2` | `!swap2` | Auto-balance swap (2nd player) |
| `!swap3` | `!swap3` | Auto-balance swap (3rd player) |
| `!balance` | `!balance` | Auto-balance teams |
| `!veto` | `!veto` | Vetoes current vote |
| `!swapteams` | `!swapteams` | Swaps entire teams |
| `!shuffleteams` | `!shuffleteams` | Shuffles teams randomly |
| `!muteall` | `!muteall <on/off>` | Mutes/unmutes all players |
| `!captain` | `!captain [player]` | Sets captain (match mode only) |
| `!sub` | `!sub [player]` | Sets substitute (match mode only) |
| `!lock` | `!lock` | Locks all players to current teams |
| `!unlock` | `!unlock` | Unlocks team assignments |

### Senior Admin (Level 60)

| Command | Usage | Description |
|---------|-------|-------------|
| `!gear` | `!gear [+/-weapon]` | Manages allowed weapons/gear |
| `!skins` | `!skins <on/off>` | Toggles client skins |
| `!funstuff` | `!funstuff <on/off>` | Toggles funstuff |
| `!goto` | `!goto <on/off>` | Toggles goto |
| `!instagib` | `!instagib <on/off>` | Toggles instagib mode |
| `!hardcore` | `!hardcore <on/off>` | Toggles hardcore mode |
| `!randomorder` | `!randomorder <on/off>` | Toggles random order |
| `!stamina` | `!stamina <default/regain/infinite>` | Sets stamina mode |
| `!moon` | `!moon <on/off>` | Toggles low gravity |
| `!public` | `!public [password]` | Makes server public or sets password |
| `!waverespawns` | `!waverespawns <on/off>` | Toggles wave respawns |
| `!respawngod` | `!respawngod <on/off>` | Toggles respawn protection |
| `!respawndelay` | `!respawndelay [seconds]` | Gets/sets respawn delay |
| `!caplimit` | `!caplimit [number]` | Gets/sets capture limit |
| `!fraglimit` | `!fraglimit [number]` | Gets/sets frag limit |
| `!timelimit` | `!timelimit [minutes]` | Gets/sets time limit |
| `!hotpotato` | `!hotpotato [value]` | Gets/sets hot potato setting |
| `!setnextmap` | `!setnextmap <map>` | Sets next map |
| `!maplist` | `!maplist` | Lists available maps |
| `!cyclemap` | `!cyclemap` | Cycles to next map |
| `!mapreload` | `!mapreload` | Reloads current map |
| `!maprestart` | `!maprestart` | Restarts current map |
| `!matchon` | `!matchon` | Enables match mode |
| `!matchoff` | `!matchoff` | Disables match mode |
| `!lms` | `!lms` | Sets gametype to Last Man Standing |
| `!jump` | `!jump` | Sets gametype to Jump |
| `!freeze` | `!freeze` | Sets gametype to Freeze Tag |
| `!gungame` | `!gungame` | Sets gametype to GunGame |

### Super Admin (Level 80)

| Command | Usage | Description |
|---------|-------|-------------|
| `!set` | `!set <cvar> <value>` | Sets any server cvar |
| `!get` | `!get <cvar>` | Gets any server cvar |
| `!exec` | `!exec <configfile>` | Executes a config file |

## Settings

```toml
[[plugins]]
name = "poweradminurt"
enabled = true

[plugins.settings]
team_balance_enabled = true
team_diff = 1
rsp_enable = false
rsp_mute_duration = 2
rsp_max_spamins = 10
rsp_falloff_rate = 2
full_ident_level = 60
```

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `team_balance_enabled` | bool | `true` | Enable automatic team balance checks |
| `team_diff` | integer | `1` | Maximum allowed team size difference |
| `rsp_enable` | bool | `false` | Enable radio spam protection |
| `rsp_mute_duration` | integer | `2` | Mute duration for radio spam (seconds) |
| `rsp_max_spamins` | integer | `10` | Radio spam point threshold |
| `rsp_falloff_rate` | integer | `2` | Radio spam decay rate |
| `full_ident_level` | integer | `60` | Level required to see full !ident info (IP/GUID) |

## Events

`EVT_CLIENT_SAY`, `EVT_CLIENT_TEAM_SAY`, `EVT_CLIENT_RADIO`, `EVT_CLIENT_TEAM_CHANGE`, `EVT_CLIENT_TEAM_CHANGE2`
