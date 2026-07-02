#!/bin/bash
SD=agent.r3.pugbot.net
CRED='r3admin:imEPpMD4kHaqjxuOsfiT'
echo "=== 1. login THROUGH the proxy (Basic Auth + JSON body) ==="
TOK=$(curl -sS -m 10 -u "$CRED" -X POST -H 'Content-Type: application/json' \
  --data '{"username":"admin","password":"changeme"}' \
  https://$SD/api/v1/auth/login | sed -n 's/.*"token":"\([^"]*\)".*/\1/p')
if [ -n "$TOK" ]; then echo "  login OK, got token (len ${#TOK})"; else echo "  LOGIN FAILED through proxy"; fi
echo
echo "=== 2. authenticated call with Bearer token THROUGH proxy (this is what breaks) ==="
echo "  --- with ONLY Bearer (browser also sends Basic, but curl test isolates) ---"
curl -sS -m 10 -H "Authorization: Bearer $TOK" -o /dev/null \
  -w '  GET /api/v1/auth/me (Bearer only) -> HTTP %{http_code}\n' https://$SD/api/v1/auth/me
echo "  --- with Basic AND Bearer can't both be in one header; browser replaces Basic with Bearer ---"
echo "  => Apache Basic Auth sees a Bearer header and returns 401. THAT is the bounce-back."
