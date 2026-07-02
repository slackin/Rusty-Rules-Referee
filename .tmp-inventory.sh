#!/bin/bash
echo "===== OS / RESOURCES ====="
. /etc/os-release; echo "OS: $PRETTY_NAME"
echo "CPU: $(nproc) cores"; free -m | awk 'NR==1||NR==2'
echo "--- disk ---"; df -h / /home 2>/dev/null | grep -v Filesystem
echo
echo "===== VIRTUALMIN ====="
if command -v virtualmin >/dev/null 2>&1; then
  echo "virtualmin: present ($(virtualmin --version 2>/dev/null | head -1))"
else
  echo "virtualmin: NOT FOUND in PATH"
fi
[ -d /etc/webmin/virtual-server ] && echo "webmin virtual-server module dir: present" || echo "webmin virtual-server module dir: absent"
echo "--- domains ---"
virtualmin list-domains --name-only 2>/dev/null || echo "(could not list domains via virtualmin)"
echo
echo "===== WEB SERVER ====="
if command -v apache2 >/dev/null 2>&1; then echo "apache2: $(apache2 -v 2>/dev/null | head -1)"; else echo "apache2: absent"; fi
if command -v nginx >/dev/null 2>&1; then echo "nginx: $(nginx -v 2>&1)"; else echo "nginx: absent"; fi
echo "apache2 active: $(systemctl is-active apache2 2>/dev/null)"
echo "nginx active: $(systemctl is-active nginx 2>/dev/null)"
echo "--- apache proxy/ssl modules ---"
if command -v apache2ctl >/dev/null 2>&1; then
  apache2ctl -M 2>/dev/null | grep -E 'proxy|ssl|wstunnel|rewrite' || echo "(no proxy/ssl/rewrite modules loaded)"
fi
echo
echo "===== DNS ====="
if command -v named >/dev/null 2>&1; then echo "bind/named: present"; else echo "bind/named: absent"; fi
echo "named active: $(systemctl is-active named bind9 2>/dev/null | tr '\n' ' ')"
echo
echo "===== NETWORK / PUBLIC IP ====="
ip -4 addr show scope global | awk '/inet /{print "iface_ip: "$2}'
echo "default route via: $(ip route | awk '/default/{print $3}')"
echo "detected public IP: $(curl -s --max-time 6 https://api.ipify.org 2>/dev/null || echo '(no outbound/curl)')"
echo
echo "===== USERS / DOMAINS ON DISK ====="
ls -1 /home 2>/dev/null | sed 's/^/home_user: /'
for d in /home/*/domains; do [ -d "$d" ] && ls -1 "$d" 2>/dev/null | sed "s|^|domain_dir: |"; done
echo
echo "===== EXISTING R3 / PORTS ====="
systemctl list-units --type=service --all 2>/dev/null | grep -iE 'r3|referee|urt' || echo "(no r3/referee/urt services)"
ss -tlnp 2>/dev/null | grep -E ':80 |:443 |:2727 ' || echo "(nothing on 80/443/2727)"
echo "===== END ====="
