# API Reference

The R3 web dashboard exposes a REST API at `/api/v1/`. All endpoints require JWT authentication unless noted otherwise.

## Authentication

### Login
```
POST /api/v1/auth/login
```
**Body:** `{ "username": "admin", "password": "changeme" }`
**Response:** `{ "token": "eyJ..." }`

Use the returned token in subsequent requests:
```
Authorization: Bearer eyJ...
```

### Current User
```
GET /api/v1/auth/me
```
Returns the authenticated user's info.

## Server

### Server Status
```
GET /api/v1/server/status
```
Returns current game state: map, game type, player count, round times, etc.

### Send RCON Command
```
POST /api/v1/server/rcon
```
**Body:** `{ "command": "status" }`
**Requires:** Admin role. Audit-logged.

### Send Public Message
```
POST /api/v1/server/say
```
**Body:** `{ "message": "Hello server!" }`
**Requires:** Admin role.

## Players

### List Connected Players
```
GET /api/v1/players
```
Returns all connected players with group info, scores, and pings.

### Player Detail
```
GET /api/v1/players/:id
```
Returns full player info: database record, live status, stats, penalties, aliases, XLR stats, and gear decoding.

## Penalties

### List Penalties
```
GET /api/v1/penalties?client_id=123&type=Ban&limit=50&offset=0
```
Query penalties by client ID, type, with pagination.

### Disable Penalty
```
POST /api/v1/penalties/:id/disable
```
Disables a ban or tempban. **Requires:** Admin role.

## Statistics

### Leaderboard
```
GET /api/v1/stats/leaderboard?limit=50&offset=0
```
XLR leaderboard with pagination.

### Player Stats
```
GET /api/v1/stats/player/:id
```
Individual player XLR stats and weapon stats.

### Weapon Stats
```
GET /api/v1/stats/weapons
```
Global weapon usage statistics.

### Map Stats
```
GET /api/v1/stats/maps
```
Per-map statistics.

### Dashboard Summary
```
GET /api/v1/stats/summary
```
Aggregate counts for dashboard stat cards.

## Groups

### List Groups
```
GET /api/v1/groups
```
Returns all permission groups with their levels.

## Aliases

### Player Aliases
```
GET /api/v1/aliases?client_id=123
```
Returns name history for a player.

## Chat

### Chat History
```
GET /api/v1/chat?limit=50&offset=0
```
Returns recent chat messages from the database.

## Votes

### Vote History
```
GET /api/v1/votes?limit=50&offset=0
```
Returns recent callvote history.

## Notes

### Get Notes
```
GET /api/v1/notes
```
Returns the authenticated user's personal notes.

### Save Notes
```
PUT /api/v1/notes
```
**Body:** `{ "content": "My notes here..." }`

## Plugins

### List Plugins
```
GET /api/v1/plugins
```
Returns all plugins with their enabled/disabled status.

## Configuration

### Get Config
```
GET /api/v1/config
```
Returns current configuration with secrets redacted. **Requires:** Admin role.

### Update Config
```
PUT /api/v1/config
```
Updates config and writes to disk. **Requires:** Admin role. Audit-logged.

## Admin Users

### List Users
```
GET /api/v1/users
```
**Requires:** Admin role.

### Create User
```
POST /api/v1/users
```
**Body:** `{ "username": "newadmin", "password": "secret", "role": "admin" }`
**Requires:** Admin role.

### Update User
```
PUT /api/v1/users/:id
```
**Body:** `{ "password": "newpass", "role": "viewer" }`
**Requires:** Admin role.

### Delete User
```
DELETE /api/v1/users/:id
```
Cannot delete yourself. **Requires:** Admin role.

### Change Own Password
```
PUT /api/v1/users/me/password
```
**Body:** `{ "current_password": "old", "new_password": "new" }`

## Audit Log

### Get Audit Log
```
GET /api/v1/audit-log?limit=50&offset=0
```
Returns paginated admin action history. **Requires:** Admin role.

## WebSocket

### Connect
```
WS /ws?token=eyJ...
```

Receives real-time JSON events:

```json
{ "type": "player_connect", "data": { "name": "Player1", "cid": "0" } }
{ "type": "kill", "data": { "killer": "Player1", "victim": "Player2", "weapon": "ump45" } }
{ "type": "say", "data": { "name": "Player1", "message": "gg", "channel": "all" } }
{ "type": "callvote", "data": { "name": "Player1", "vote_type": "map", "data": "ut4_abbey" } }
```
