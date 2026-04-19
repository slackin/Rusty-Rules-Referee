# Plugins Overview

R3 ships with **30 plugins** covering server administration, moderation, statistics, anti-abuse, and more. All plugins are optional and individually configurable.

## Enabling Plugins

Add a `[[plugins]]` entry to your `referee.toml` for each plugin you want:

```toml
[[plugins]]
name = "admin"
enabled = true

[plugins.settings]
max_warnings = 3
```

Set `enabled = false` to disable a plugin without removing its configuration.

## Plugin Categories

### Core Administration
| Plugin | Description |
|--------|-------------|
| [admin](/plugins/admin) | Core command handler — !kick, !ban, !warn, !tempban, and 50+ commands |
| [poweradminurt](/plugins/poweradminurt) | Urban Terror specific commands — !slap, !nuke, !gear, team management |

### Moderation
| Plugin | Description |
|--------|-------------|
| [censor](/plugins/censor) | Filters offensive language from chat with regex patterns |
| [censorurt](/plugins/censorurt) | Urban Terror specific name/clan tag censoring |
| [spamcontrol](/plugins/spamcontrol) | Prevents chat flooding and message repetition |
| [tk](/plugins/tk) | Team kill monitoring and automatic penalties |
| [spawnkill](/plugins/spawnkill) | Detects and punishes spawn killing |

### Player Management
| Plugin | Description |
|--------|-------------|
| [welcome](/plugins/welcome) | Greets new and returning players |
| [makeroom](/plugins/makeroom) | Kicks lowest-level player to make room for admins |
| [nickreg](/plugins/nickreg) | Nickname registration and impostor protection |
| [namechecker](/plugins/namechecker) | Name policy enforcement (duplicates, forbidden names, change limits) |
| [specchecker](/plugins/specchecker) | Kicks idle spectators when the server is busy |
| [login](/plugins/login) | Requires admins to authenticate before using commands |
| [follow](/plugins/follow) | Follow a player and get notifications about their activity |

### Anti-Abuse
| Plugin | Description |
|--------|-------------|
| [afk](/plugins/afk) | Detects and handles AFK players |
| [pingwatch](/plugins/pingwatch) | Monitors and kicks high-ping players |
| [vpncheck](/plugins/vpncheck) | Blocks VPN/proxy connections |
| [countryfilter](/plugins/countryfilter) | GeoIP-based country filtering |
| [callvote](/plugins/callvote) | Controls and filters in-game callvotes |

### Statistics
| Plugin | Description |
|--------|-------------|
| [stats](/plugins/stats) | In-memory K/D tracking with !stats and !topstats |
| [xlrstats](/plugins/xlrstats) | Extended live rankings with ELO skill tracking |
| [headshotcounter](/plugins/headshotcounter) | Headshot streak tracking and aimbot detection |
| [spree](/plugins/spree) | Killing spree announcements |
| [firstkill](/plugins/firstkill) | First kill of each round announcements |
| [flagannounce](/plugins/flagannounce) | Flag event announcements (CTF) |

### Chat & Logging
| Plugin | Description |
|--------|-------------|
| [chatlogger](/plugins/chatlogger) | Logs all chat to daily rotating files and database |
| [customcommands](/plugins/customcommands) | User-defined text commands (!rules, !discord, etc.) |

### Server Management
| Plugin | Description |
|--------|-------------|
| [adv](/plugins/adv) | Rotating server advertisement messages |
| [scheduler](/plugins/scheduler) | Runs commands on game events (round start, map change, etc.) |
| [mapconfig](/plugins/mapconfig) | Executes map-specific RCON commands on map change |

## Plugin Dependencies

Some plugins depend on others:

- **censor**, **spamcontrol**, **tk**, **login**, **follow**, **makeroom**, **customcommands** require the **admin** plugin
- **callvote**, **spawnkill**, **headshotcounter**, **namechecker**, **specchecker** require the **iourt43** parser

The admin plugin should always be loaded first.

## Permission Levels

Many plugins reference permission levels. R3 uses this hierarchy:

| Level | Value | Description |
|-------|-------|-------------|
| Guest | 0 | Default for new players |
| User | 1 | Registered players |
| Regular | 2 | Trusted regulars |
| Moderator | 20 | Can warn, kick, manage players |
| Admin | 40 | Can tempban, use advanced commands |
| Senior Admin | 60 | Can permanently ban, configure server |
| Super Admin | 80 | Full access to all commands |
