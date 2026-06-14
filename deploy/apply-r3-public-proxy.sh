#!/bin/bash
# =============================================================================
# apply-r3-public-proxy.sh — (re)apply the public bug-report reverse proxy.
#
# Run on the web server (root@10.10.0.4). The R3 public bug-report page is a
# static file served by the r3.pugbot.net docs site; it POSTs to
# /api/v1/bug-reports, which must be proxied to the R3 master on 127.0.0.1:2727.
#
# This proxy lives in the Apache vhost. Virtualmin's `--add-directive` mangles
# directive values that contain spaces (the ProxyPass target), so we add the
# block directly and idempotently. If Virtualmin ever fully regenerates the
# vhost (e.g. you change SSL/PHP settings in the Virtualmin UI), the proxy may
# be dropped — just re-run this script to restore it.
#
# ONLY the single public endpoint is proxied. The admin dashboard is NOT
# exposed on the domain (it stays on 127.0.0.1:2727, firewalled to the LAN/VPN
# and whitelisted admin IPs).
# =============================================================================
set -uo pipefail
F=/etc/apache2/sites-enabled/r3.pugbot.net.conf
TS=$(date +%Y%m%d-%H%M%S)

if [ ! -f "$F" ]; then echo "vhost not found: $F"; exit 1; fi

if grep -q '/api/v1/bug-reports' "$F"; then
  echo "proxy already present — nothing to do"
  exit 0
fi

cp -a "$F" "/root/r3.pugbot.net.conf.bak-reapply-$TS"
echo "backup -> /root/r3.pugbot.net.conf.bak-reapply-$TS"

python3 - "$F" <<'PY'
import sys, re
p = sys.argv[1]; s = open(p).read()
block = """    # --- R3 public bug-submission endpoint ONLY (not the admin app) ---
    ProxyPass /api/v1/bug-reports http://127.0.0.1:2727/api/v1/bug-reports
    ProxyPassReverse /api/v1/bug-reports http://127.0.0.1:2727/api/v1/bug-reports
    # --- end R3 public endpoint ---
"""
m = re.search(r'(<VirtualHost\s+66\.96\.82\.50:443>.*?)(</VirtualHost>)', s, re.S)
if not m: sys.stderr.write("ERROR: :443 vhost not found\n"); sys.exit(2)
s = s[:m.start()] + m.group(1) + block + m.group(2) + s[m.end():]
open(p,'w').write(s); print("inserted proxy block into :443 vhost")
PY

if apache2ctl configtest 2>&1 | grep -q 'Syntax OK'; then
  systemctl reload apache2
  echo "reloaded. verifying:"
  curl -sS -m 8 -X POST -H 'Content-Type: application/json' -d '{"title":""}' \
    -o /dev/null -w '  POST /api/v1/bug-reports -> HTTP %{http_code} (expect 400)\n' \
    https://r3.pugbot.net/api/v1/bug-reports
else
  echo "configtest FAILED — restoring"
  cp -a "/root/r3.pugbot.net.conf.bak-reapply-$TS" "$F"; systemctl reload apache2 || true
  exit 1
fi
