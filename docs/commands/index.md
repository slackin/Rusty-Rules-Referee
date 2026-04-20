# Command Reference

Complete list of all R3 commands organized by permission level. Commands use `!` for private response, `@` for public response, or `&` for bigtext.

## Guest (Level 0)

Available to all players.

| Command | Plugin | Description |
|---------|--------|-------------|
| `!help` | admin | Lists commands available at your level |
| `!leveltest [player]` | admin | Shows your or a player's group and level |
| `!time` | admin | Shows current server time (UTC) |
| `!register` / `!regme` | admin | Self-register as User (level 1) |
| `!r3` | admin | Shows R3 version information |
| `!stats` | stats | Shows your kills, deaths, K/D ratio |
| `!topstats` | stats | Shows top 5 players by kills |
| `!xlrstats` / `!xlr` | xlrstats | Shows your XLR stats and skill rating |
| `!xlrtopstats` / `!topstats` | xlrstats | Shows top 10 ranked players by skill |
| `!hs` / `!headshots [player]` | headshotcounter | Shows headshot stats |
| `!login <password>` | login | Authenticates with admin password |
| `!setpassword <password>` | login | Sets/changes admin password |
| `!follow <player>` | follow | Start following a player |
| `!unfollow <player>` | follow | Stop following a player |
| `!greeting` | welcome | Show your current custom greeting |
| `!setgreeting <message>` | welcome | Set a custom greeting (use $name for your name, 'none' to clear) |
| `!forgive` / `!f` | tk | Forgive the last person who team killed you |
| `!forgivelist` / `!fl` | tk | List unforgiven TKs against you |
| `!forgiveall` / `!fa` | tk | Forgive all TKs against you |
| `!forgiveinfo` / `!fi` | tk | Show your unforgiven TK count and kicks remaining |
| `!forgiveprev` / `!fp` | tk | Show who last team killed you |
| `!<custom>` | customcommands | Any user-defined command |

## User (Level 1)

Registered players.

| Command | Plugin | Description |
|---------|--------|-------------|
| `!regulars` | admin | Lists online Regular+ players |
| `!rules [player]` | admin | Shows server rules |

## Moderator (Level 20)

Can manage players and issue warnings.

| Command | Plugin | Description |
|---------|--------|-------------|
| `!status` | admin | Connected players with slot, name, ID, level |
| `!list` | admin | Compact player list |
| `!lookup <name>` | admin | Search database for players |
| `!find <name>` | admin | Find connected players by name |
| `!admins` | admin | List online admins |
| `!warn <player> [reason/keyword]` | admin | Warn a player |
| `!kick <player> [reason]` | admin | Kick a player |
| `!spank <player> [reason]` | admin | Kick with public humiliation |
| `!seen <name>` | admin | When was a player last online |
| `!aliases <name or @id>` | admin | Player's name history |
| `!poke <player>` | admin | Send attention message |
| `!notice <player> <text>` | admin | Add note to player's record |
| `!clear <player>` | admin | Clear all warnings and notices |
| `!warns` | admin | List warn keywords |
| `!warntest <keyword>` | admin | Test a warn keyword |
| `!warnremove <player>` | admin | Remove last warning |
| `!warninfo <player>` | admin | Show warning count |
| `!ident [player]` / `!id` | poweradminurt | Show player @id |
| `!forgiveclear <player>` / `!fc` | tk | Admin: clear TK records for a player |

## Admin (Level 40)

Can tempban and use advanced server commands.

| Command | Plugin | Description |
|---------|--------|-------------|
| `!tempban <player> [duration] [reason]` | admin | Temporary ban (Nm/Nh/Nd/Nw) |
| `!lastbans` | admin | Show 5 most recent bans |
| `!baninfo <player>` | admin | Show ban count |
| `!spam <keyword> [player]` | admin | Send predefined message |
| `!spams` | admin | List spam keywords |
| `!clientinfo <name or @id>` | admin | Detailed client info |
| `!slap <player> [count]` | poweradminurt | Slap a player (max 25) |
| `!nuke <player>` | poweradminurt | Nuke a player |
| `!mute <player> [seconds]` | poweradminurt | Mute a player |
| `!kill <player>` | poweradminurt | Smite a player |
| `!force <player> <team>` | poweradminurt | Force to red/blue/spec/free |
| `!swap <p1> [p2]` | poweradminurt | Swap two players' teams |
| `!swap2` / `!swap3` | poweradminurt | Auto-balance swap |
| `!balance` | poweradminurt | Auto-balance teams |
| `!veto` | poweradminurt | Veto current vote |
| `!swapteams` | poweradminurt | Swap entire teams |
| `!shuffleteams` | poweradminurt | Shuffle teams randomly |
| `!muteall <on/off>` | poweradminurt | Mute/unmute all |
| `!captain [player]` | poweradminurt | Set captain (match mode) |
| `!sub [player]` | poweradminurt | Set substitute (match mode) |
| `!lock` / `!unlock` | poweradminurt | Lock/unlock team assignments |
| `!teams` | poweradminurt | Force team balance by player count |
| `!skuffle` | poweradminurt | Skill-based team shuffle |
| `!advise` | poweradminurt | Report team balance status |
| `!autoskuffle [mode]` | poweradminurt | Set skill balance mode (0-3) |

## Senior Admin (Level 60)

Can permanently ban and configure server settings.

| Command | Plugin | Description |
|---------|--------|-------------|
| `!ban <player> [reason]` | admin | Permanent ban |
| `!permban <player> [reason]` | admin | Permanent ban (explicit) |
| `!unban <name or @id>` | admin | Unban a player |
| `!say <message>` | admin | Public server message |
| `!scream <message>` | admin | Bigtext to server |
| `!longlist` | admin | Detailed player list with IPs |
| `!warnclear <player>` | admin | Clear all warnings |
| `!kickall <pattern> [reason]` | admin | Kick all matching players |
| `!banall <pattern> [reason]` | admin | Ban all matching players |
| `!spankall <pattern> [reason]` | admin | Spank all matching players |
| `!mask [player] [level]` | admin | Mask player's level |
| `!unmask [player]` | admin | Remove level mask |
| `!makereg <player>` | admin | Promote to Regular |
| `!unreg <player>` | admin | Demote to User |
| `!gear [+/-weapon]` | poweradminurt | Manage allowed gear |
| `!skins <on/off>` | poweradminurt | Toggle client skins |
| `!funstuff <on/off>` | poweradminurt | Toggle funstuff |
| `!goto <on/off>` | poweradminurt | Toggle goto |
| `!instagib <on/off>` | poweradminurt | Toggle instagib |
| `!hardcore <on/off>` | poweradminurt | Toggle hardcore |
| `!randomorder <on/off>` | poweradminurt | Toggle random order |
| `!stamina <mode>` | poweradminurt | Set stamina mode |
| `!moon <on/off>` | poweradminurt | Toggle low gravity |
| `!public [password]` | poweradminurt | Set server public/password |
| `!waverespawns <on/off>` | poweradminurt | Toggle wave respawns |
| `!respawngod <on/off>` | poweradminurt | Toggle respawn protection |
| `!respawndelay [secs]` | poweradminurt | Get/set respawn delay |
| `!caplimit [num]` | poweradminurt | Get/set capture limit |
| `!fraglimit [num]` | poweradminurt | Get/set frag limit |
| `!timelimit [mins]` | poweradminurt | Get/set time limit |
| `!hotpotato [val]` | poweradminurt | Get/set hot potato |
| `!setnextmap <map>` | poweradminurt | Set next map |
| `!maplist` | poweradminurt | List available maps |
| `!cyclemap` | poweradminurt | Cycle to next map |
| `!mapreload` | poweradminurt | Reload current map |
| `!maprestart` | poweradminurt | Restart current map |
| `!matchon` / `!matchoff` | poweradminurt | Enable/disable match mode |
| `!lms` / `!jump` / `!freeze` / `!gungame` | poweradminurt | Set gametype |
| `!ffa` / `!tdm` / `!ts` / `!ftl` | poweradminurt | Set gametype (FFA, TDM, Team Survivor, Follow The Leader) |
| `!cah` / `!ctf` / `!bomb` | poweradminurt | Set gametype (Capture And Hold, CTF, Bomb) |
| `!bluewave` / `!redwave [secs]` | poweradminurt | Get/set team wave respawn time |
| `!setwave <secs>` | poweradminurt | Set wave respawn for both teams |
| `!setgravity [val]` | poweradminurt | Get/set gravity (use 'default' to reset) |
| `!vote <on/off/reset>` | poweradminurt | Enable/disable/reset voting |
| `!bigtext <message>` | poweradminurt | Display big text on all screens |
| `!version` | poweradminurt | Show PowerAdminUrt version info |
| `!pause` | poweradminurt | Pause/unpause game |

## Super Admin (Level 80)

Full access to all commands.

| Command | Plugin | Description |
|---------|--------|-------------|
| `!putgroup <player> <group>` | admin | Set player's group |
| `!ungroup <player>` | admin | Remove from all groups |
| `!map <mapname>` | admin | Change map |
| `!maps` | admin | List available maps |
| `!nextmap` | admin | Show next map |
| `!maprotate` | admin | Cycle to next map |
| `!die` | admin | Shut down R3 |
| `!restart` | admin | Restart R3 |
| `!reconfig` | admin | Reload configuration |
| `!pause` | admin | Pause log parsing |
| `!rebuild` | admin | Re-sync client list |
| `!runas <player> <command>` | admin | Run command as another player |
| `!iamgod` | admin | Promote self to Super Admin (first time only) |
| `!set <cvar> <value>` | poweradminurt | Set any server cvar |
| `!get <cvar>` | poweradminurt | Get any server cvar |
| `!exec <configfile>` | poweradminurt | Execute config file |

## Group Names

For `!putgroup`, use these group keywords:

| Keyword | Level |
|---------|-------|
| `guest` | 0 |
| `user` | 1 |
| `regular` / `reg` | 2 |
| `mod` / `moderator` | 20 |
| `admin` | 40 |
| `senioradmin` / `senior` | 60 |
| `superadmin` / `super` | 80 |

## Duration Format

For `!tempban`, durations can be specified as:

| Format | Example | Duration |
|--------|---------|----------|
| `Nm` | `30m` | 30 minutes |
| `Nh` | `2h` | 2 hours |
| `Nd` | `7d` | 7 days |
| `Nw` | `2w` | 2 weeks |

Default duration (when omitted): **2 hours**.
