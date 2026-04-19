# FlagAnnounce Plugin

Announces flag events (pickup, drop, capture, return) with bigtext messages for CTF game modes.

**Plugin name:** `flagannounce`
**Requires config:** No

## Behavior

- Monitors all flag-related events
- Announces each event with a bigtext message visible to all players
- Includes player name and team information

## Settings

None — no configuration required.

## Events

`EVT_CLIENT_FLAG_PICKUP`, `EVT_CLIENT_FLAG_DROPPED`, `EVT_CLIENT_FLAG_CAPTURED`, `EVT_CLIENT_FLAG_RETURNED`
