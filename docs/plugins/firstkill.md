# FirstKill Plugin

Announces the first kill of each round with a bigtext message.

**Plugin name:** `firstkill`
**Requires config:** No

## Behavior

- Watches for the first kill event after each round starts
- Announces the killer and victim via bigtext
- Uses atomic flag to prevent duplicate announcements

## Settings

None — no configuration required.

## Events

`EVT_CLIENT_KILL`, `EVT_GAME_ROUND_START`
