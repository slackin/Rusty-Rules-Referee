# Follow Plugin

Allows admins to follow a player and receive private notifications about their activity (kills, chat, team changes, disconnects).

**Plugin name:** `follow`
**Requires:** admin plugin

## Commands

| Command | Level | Usage | Description |
|---------|-------|-------|-------------|
| `!follow` | 0 | `!follow <player>` | Start following a player |
| `!unfollow` | 0 | `!unfollow <player>` | Stop following a player |

## Behavior

- When following a player, you receive private messages when they:
  - Kill or get killed
  - Send chat messages
  - Change teams
  - Disconnect
- Multiple admins can follow the same player
- Following state is cleared on disconnect

## Settings

None — no configuration required.

## Events

`EVT_CLIENT_SAY`, `EVT_CLIENT_KILL`, `EVT_CLIENT_TEAM_CHANGE`, `EVT_CLIENT_DISCONNECT`
