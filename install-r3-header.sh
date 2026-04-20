#!/bin/bash
# =============================================================================
#  Rusty Rules Referee — Installer v2.0.0
#  Urban Terror 4.3 Server Administration Bot
#
#  Supports three modes: Standalone, Master, or Client
#  Usage: sudo bash install-r3.sh
# =============================================================================
set -e

# ---- Colors ----
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
NC='\033[0m'

info()    { echo -e "  ${GREEN}✓${NC} $1"; }
warn()    { echo -e "  ${YELLOW}!${NC} $1"; }
err()     { echo -e "  ${RED}✗${NC} $1"; }
ask()     { echo -en "  ${CYAN}?${NC} $1"; }
section() { echo ""; echo -e "  ${BOLD}── $1 ──${NC}"; echo ""; }

# ---- Banner ----
echo ""
echo -e "${CYAN}${BOLD}"
echo "  ██████╗ ██████╗ "
echo "  ██╔══██╗╚════██╗"
echo "  ██████╔╝ █████╔╝"
echo "  ██╔══██╗ ╚═══██╗"
echo "  ██║  ██║██████╔╝"
echo "  ╚═╝  ╚═╝╚═════╝ "
echo -e "${NC}"
echo -e "  ${BOLD}Rusty Rules Referee${NC} — Urban Terror Server Admin Bot"
echo -e "  ${DIM}Version 2.0.0 — Installer${NC}"
echo ""

# ---- Root check ----
if [ "$(id -u)" -ne 0 ]; then
    err "Please run as root:  sudo bash $0"
    exit 1
fi

# ---- Detect user ----
REAL_USER="${SUDO_USER:-}"
if [ -z "$REAL_USER" ] || [ "$REAL_USER" = "root" ]; then
    ask "Which system user should run the bot? "
    read -r REAL_USER
    if [ -z "$REAL_USER" ]; then
        err "A non-root user is required."
        exit 1
    fi
    if ! id "$REAL_USER" &>/dev/null; then
        info "Creating user '$REAL_USER'..."
        useradd -m -s /bin/bash "$REAL_USER"
    fi
fi

HOME_DIR=$(eval echo "~$REAL_USER")

# ---- Mode Selection ----
section "Installation Mode"

echo -e "  ${BOLD}1)${NC} Standalone  — Single bot managing one game server (most common)"
echo -e "  ${BOLD}2)${NC} Master      — Central hub managing multiple client bots"
echo -e "  ${BOLD}3)${NC} Client      — Bot managed by a remote master server"
echo ""
ask "Select mode [1]: "
read -r MODE_CHOICE
MODE_CHOICE="${MODE_CHOICE:-1}"

case "$MODE_CHOICE" in
    1) RUN_MODE="standalone" ;;
    2) RUN_MODE="master" ;;
    3) RUN_MODE="client" ;;
    *) err "Invalid choice"; exit 1 ;;
esac

info "Mode: ${RUN_MODE}"

# Install dir and service name — set defaults, client overrides after collecting server name
INSTALL_DIR="$HOME_DIR/r3"
SERVICE_NAME="r3"

DEFAULT_IP=$(hostname -I 2>/dev/null | awk '{print $1}')

# =============================================================================
# Game server download function (standalone and client modes)
# =============================================================================
download_game_server() {
    section "Game Server Download"
    ask "Download and install Urban Terror 4.3 dedicated server? [y/N]: "
    read -r DL_CHOICE
    if [ "$DL_CHOICE" != "y" ] && [ "$DL_CHOICE" != "Y" ]; then
        return 0
    fi

    local URT_DIR="$HOME_DIR/urbanterror"
    local URT_URL="https://www.urbanterror.info/downloads/software/urt/43/UrbanTerror43_ded.tar.gz"

    info "Downloading Urban Terror 4.3 dedicated server..."
    mkdir -p "$URT_DIR"

    if command -v wget &>/dev/null; then
        wget -q --show-progress -O "/tmp/urt43_ded.tar.gz" "$URT_URL" || {
            warn "Download failed. You can install the game server manually later."
            return 0
        }
    elif command -v curl &>/dev/null; then
        curl -fL --progress-bar -o "/tmp/urt43_ded.tar.gz" "$URT_URL" || {
            warn "Download failed. You can install the game server manually later."
            return 0
        }
    else
        warn "Neither wget nor curl found. Install the game server manually."
        return 0
    fi

    info "Extracting to $URT_DIR..."
    tar xzf /tmp/urt43_ded.tar.gz -C "$URT_DIR" --strip-components=1 2>/dev/null || \
        tar xzf /tmp/urt43_ded.tar.gz -C "$URT_DIR" 2>/dev/null || {
            warn "Extraction failed. Check the archive manually."
            return 0
        }
    rm -f /tmp/urt43_ded.tar.gz

    chown -R "$REAL_USER:$REAL_USER" "$URT_DIR"
    info "Game server installed at $URT_DIR"

    # Auto-detect game log from the new install
    GAME_LOG="$URT_DIR/q3ut4/games.log"
}

# =============================================================================
# Collect config per mode
# =============================================================================
collect_game_config() {
    # Used by standalone and client modes
    section "Game Server Configuration"

    ask "Game server IP [${DEFAULT_IP}]: "
    read -r SERVER_IP
    SERVER_IP="${SERVER_IP:-$DEFAULT_IP}"

    ask "Game server port [27960]: "
    read -r SERVER_PORT
    SERVER_PORT="${SERVER_PORT:-27960}"

    while true; do
        ask "RCON password (required): "
        read -rs RCON_PASS
        echo ""
        if [ -n "$RCON_PASS" ]; then break; fi
        warn "RCON password cannot be empty"
    done

    # Auto-detect game log
    if [ -z "$GAME_LOG" ]; then
        for candidate in \
            "$HOME_DIR/.q3a/q3ut4/games.log" \
            "/home/$REAL_USER/.q3a/q3ut4/games.log" \
            "$HOME_DIR/urbanterror/q3ut4/games.log" \
            "$HOME_DIR/urbanterror/UrbanTerror43/q3ut4/games.log" \
            "/opt/urbanterror/q3ut4/games.log"; do
            if [ -f "$candidate" ]; then
                GAME_LOG="$candidate"
                break
            fi
        done
    fi

    if [ -n "$GAME_LOG" ]; then
        ask "Game log path [${GAME_LOG}]: "
        read -r LOG_INPUT
        GAME_LOG="${LOG_INPUT:-$GAME_LOG}"
    else
        echo ""
        warn "Could not auto-detect game log. Common locations:"
        echo -e "    ${DIM}~/.q3a/q3ut4/games.log${NC}"
        echo -e "    ${DIM}~/urbanterror/UrbanTerror43/q3ut4/games.log${NC}"
        echo ""
        ask "Game log path: "
        read -r GAME_LOG
        if [ -z "$GAME_LOG" ]; then
            GAME_LOG="$HOME_DIR/.q3a/q3ut4/games.log"
            warn "Defaulting to $GAME_LOG — update in web UI if incorrect"
        fi
    fi
}

# =============================================================================
# Mode-specific setup
# =============================================================================
GAME_LOG=""
SERVER_IP="$DEFAULT_IP"
SERVER_PORT=27960
RCON_PASS=""
WEB_PORT=8080
MASTER_SYNC_PORT=9443
MASTER_URL=""
QUICK_CONNECT_KEY=""

case "$RUN_MODE" in
    standalone)
        download_game_server
        collect_game_config

        ask "Web UI port [8080]: "
        read -r WEB_PORT
        WEB_PORT="${WEB_PORT:-8080}"
        ;;

    master)
        section "Master Configuration"

        ask "Web UI port [8080]: "
        read -r WEB_PORT
        WEB_PORT="${WEB_PORT:-8080}"

        ask "Sync API port [9443]: "
        read -r MASTER_SYNC_PORT
        MASTER_SYNC_PORT="${MASTER_SYNC_PORT:-9443}"

        ask "Master bind IP [${DEFAULT_IP}]: "
        read -r SERVER_IP
        SERVER_IP="${SERVER_IP:-$DEFAULT_IP}"
        ;;

    client)
        section "Master Connection"

        while true; do
            ask "Master server address (e.g. master.example.com or 10.0.0.1): "
            read -r MASTER_HOST
            if [ -n "$MASTER_HOST" ]; then break; fi
            warn "Master address is required"
        done

        ask "Master web port [8080]: "
        read -r MASTER_WEB_PORT
        MASTER_WEB_PORT="${MASTER_WEB_PORT:-8080}"

        while true; do
            ask "Quick-connect key (from master web UI): "
            read -r QUICK_CONNECT_KEY
            if [ -n "$QUICK_CONNECT_KEY" ]; then break; fi
            warn "Quick-connect key is required"
        done

        ask "Name for this server [$(hostname -s)]: "
        read -r CLIENT_SERVER_NAME
        CLIENT_SERVER_NAME="${CLIENT_SERVER_NAME:-$(hostname -s)}"

        # Derive unique install dir and service name from server name
        INSTANCE_SLUG=$(echo "$CLIENT_SERVER_NAME" | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9]/-/g; s/--*/-/g; s/^-//; s/-$//')
        INSTALL_DIR="$HOME_DIR/r3-${INSTANCE_SLUG}"
        SERVICE_NAME="r3-${INSTANCE_SLUG}"
        ;;
esac

echo -e "  Install path: ${BOLD}$INSTALL_DIR${NC}"
echo -e "  Run as user:  ${BOLD}$REAL_USER${NC}"
echo ""

# =============================================================================
# Install
# =============================================================================
section "Installing"

# ---- Extract ----
info "Extracting files to $INSTALL_DIR..."
ARCHIVE_LINE=$(awk '/^__ARCHIVE_MARKER__$/{print NR + 1; exit 0;}' "$0")

# Backup existing config if present
if [ -f "$INSTALL_DIR/r3.toml" ]; then
    BACKUP="$INSTALL_DIR/r3.toml.bak.$(date +%s)"
    cp "$INSTALL_DIR/r3.toml" "$BACKUP"
    warn "Existing config backed up to $BACKUP"
fi

mkdir -p "$INSTALL_DIR"
tail -n +"$ARCHIVE_LINE" "$0" | tar xz --strip-components=1 -C "$INSTALL_DIR"

# ---- Generate config ----
info "Generating configuration..."
JWT_SECRET=$(cat /proc/sys/kernel/random/uuid 2>/dev/null || head -c 32 /dev/urandom | base64 | tr -d '/+=' | head -c 36)

CERTS_DIR="$INSTALL_DIR/certs"
mkdir -p "$CERTS_DIR"

# =============================================================================
# Config generation per mode
# =============================================================================
generate_standalone_config() {
    cat > "$INSTALL_DIR/r3.toml" << CFGEOF
# Rusty Rules Referee — Standalone Configuration
# Finish setup via the web UI at http://${SERVER_IP}:${WEB_PORT}/setup

[referee]
bot_name = "Referee"
bot_prefix = "^2R3:^3"
database = "sqlite://r3.db"
logfile = "r3.log"
log_level = "info"

[server]
public_ip = "${SERVER_IP}"
port = ${SERVER_PORT}
rcon_password = "${RCON_PASS}"
game_log = "${GAME_LOG}"
delay = 0.33

[web]
enabled = true
bind_address = "0.0.0.0"
port = ${WEB_PORT}
jwt_secret = "${JWT_SECRET}"

[update]
enabled = false
url = "https://r3.pugbot.net/api/updates"
channel = "beta"          # production | beta | alpha | dev
check_interval = 3600
auto_restart = true

# ---- Plugins ----

[[plugins]]
name = "admin"
enabled = true

[[plugins]]
name = "poweradminurt"
enabled = true

[[plugins]]
name = "censor"
enabled = true

[[plugins]]
name = "spamcontrol"
enabled = true

[[plugins]]
name = "tk"
enabled = true

[[plugins]]
name = "welcome"
enabled = true

[[plugins]]
name = "chatlogger"
enabled = true

[[plugins]]
name = "stats"
enabled = true

[[plugins]]
name = "pingwatch"
enabled = true
CFGEOF
}

generate_master_config() {
    # Generate CA and server certificates
    info "Generating TLS certificates (CA + server)..."

    # CA cert
    openssl req -x509 -newkey rsa:4096 -nodes \
        -keyout "$CERTS_DIR/ca.key" \
        -out "$CERTS_DIR/ca.crt" \
        -days 3650 \
        -subj "/CN=R3 Master CA" 2>/dev/null

    # Server cert signed by CA
    openssl req -newkey rsa:2048 -nodes \
        -keyout "$CERTS_DIR/server.key" \
        -out "$CERTS_DIR/server.csr" \
        -subj "/CN=R3 Master Server" 2>/dev/null

    cat > "$CERTS_DIR/server_ext.cnf" << EXTEOF
authorityKeyIdentifier=keyid,issuer
basicConstraints=CA:FALSE
keyUsage = digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth
subjectAltName = @alt_names

[alt_names]
DNS.1 = localhost
IP.1 = 127.0.0.1
IP.2 = ${SERVER_IP}
EXTEOF

    openssl x509 -req -in "$CERTS_DIR/server.csr" \
        -CA "$CERTS_DIR/ca.crt" -CAkey "$CERTS_DIR/ca.key" \
        -CAcreateserial \
        -out "$CERTS_DIR/server.crt" \
        -days 1825 \
        -extfile "$CERTS_DIR/server_ext.cnf" 2>/dev/null

    rm -f "$CERTS_DIR/server.csr" "$CERTS_DIR/server_ext.cnf" "$CERTS_DIR/ca.srl"
    info "TLS certificates generated"

    cat > "$INSTALL_DIR/r3.toml" << CFGEOF
# Rusty Rules Referee — Master Configuration
# Finish setup via the web UI at http://${SERVER_IP}:${WEB_PORT}/setup

[referee]
bot_name = "R3 Master"
bot_prefix = "^2R3:^3"
database = "sqlite://r3.db"
logfile = "r3.log"
log_level = "info"

[server]
public_ip = "${SERVER_IP}"
port = 27960
rcon_password = ""
game_log = ""
delay = 0.33

[web]
enabled = true
bind_address = "0.0.0.0"
port = ${WEB_PORT}
jwt_secret = "${JWT_SECRET}"

[update]
enabled = false
url = "https://r3.pugbot.net/api/updates"
channel = "beta"          # production | beta | alpha | dev
check_interval = 3600
auto_restart = true

[master]
bind_address = "0.0.0.0"
port = ${MASTER_SYNC_PORT}
tls_cert = "${CERTS_DIR}/server.crt"
tls_key = "${CERTS_DIR}/server.key"
ca_cert = "${CERTS_DIR}/ca.crt"
ca_key = "${CERTS_DIR}/ca.key"
CFGEOF
}

generate_client_config() {
    # ---- Pair with master via quick-connect ----
    info "Pairing with master server..."

    PAIR_URL="http://${MASTER_HOST}:${MASTER_WEB_PORT}/api/v1/pairing/pair"
    PAIR_PAYLOAD=$(cat << JSONEOF
{"token":"${QUICK_CONNECT_KEY}","server_name":"${CLIENT_SERVER_NAME}"}
JSONEOF
    )

    PAIR_RESPONSE=$(curl -sf -X POST \
        -H "Content-Type: application/json" \
        -d "$PAIR_PAYLOAD" \
        "$PAIR_URL" 2>/dev/null) || {
        err "Failed to pair with master at $PAIR_URL"
        err "Check the master address and quick-connect key, then try again."
        exit 1
    }

    # Extract fields from JSON response
    # Uses python if available, otherwise basic sed parsing
    if command -v python3 &>/dev/null; then
        PAIR_SERVER_ID=$(echo "$PAIR_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['server_id'])")
        PAIR_CA_CERT=$(echo "$PAIR_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['ca_cert'])")
        PAIR_CLIENT_CERT=$(echo "$PAIR_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['client_cert'])")
        PAIR_CLIENT_KEY=$(echo "$PAIR_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['client_key'])")
        PAIR_SYNC_URL=$(echo "$PAIR_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['master_sync_url'])")
    else
        err "python3 is required for JSON parsing during client setup"
        exit 1
    fi

    info "Paired! Server ID: ${PAIR_SERVER_ID}"

    # Write certificates
    echo "$PAIR_CA_CERT" > "$CERTS_DIR/ca.crt"
    echo "$PAIR_CLIENT_CERT" > "$CERTS_DIR/client.crt"
    echo "$PAIR_CLIENT_KEY" > "$CERTS_DIR/client.key"
    chmod 600 "$CERTS_DIR/client.key" "$CERTS_DIR/ca.crt" "$CERTS_DIR/client.crt"
    info "TLS certificates saved"

    cat > "$INSTALL_DIR/r3.toml" << CFGEOF
# Rusty Rules Referee — Client Configuration
# Managed by master at ${PAIR_SYNC_URL}

[referee]
bot_name = "${CLIENT_SERVER_NAME}"
bot_prefix = "^2R3:^3"
database = "sqlite://r3.db"
logfile = "r3.log"
log_level = "info"

[server]
public_ip = "0.0.0.0"
port = 0
rcon_password = ""
game_log = ""
delay = 0.33

[web]
enabled = false
bind_address = "0.0.0.0"
port = 8080
jwt_secret = "${JWT_SECRET}"

[update]
enabled = false
url = "https://r3.pugbot.net/api/updates"
channel = "beta"          # production | beta | alpha | dev
check_interval = 3600
auto_restart = true

[client]
master_url = "${PAIR_SYNC_URL}"
server_name = "${CLIENT_SERVER_NAME}"
tls_cert = "${CERTS_DIR}/client.crt"
tls_key = "${CERTS_DIR}/client.key"
ca_cert = "${CERTS_DIR}/ca.crt"

# ---- Plugins ----

[[plugins]]
name = "admin"
enabled = true

[[plugins]]
name = "poweradminurt"
enabled = true

[[plugins]]
name = "censor"
enabled = true

[[plugins]]
name = "spamcontrol"
enabled = true

[[plugins]]
name = "tk"
enabled = true

[[plugins]]
name = "welcome"
enabled = true

[[plugins]]
name = "chatlogger"
enabled = true

[[plugins]]
name = "stats"
enabled = true

[[plugins]]
name = "pingwatch"
enabled = true
CFGEOF
}

case "$RUN_MODE" in
    standalone) generate_standalone_config ;;
    master)     generate_master_config ;;
    client)     generate_client_config ;;
esac

# ---- Permissions ----
info "Setting permissions..."
chown -R "$REAL_USER:$REAL_USER" "$INSTALL_DIR"
chmod 700 "$INSTALL_DIR"
chmod +x "$INSTALL_DIR/rusty-rules-referee"
chmod 600 "$INSTALL_DIR/r3.toml"
if [ -d "$CERTS_DIR" ]; then
    chmod 700 "$CERTS_DIR"
    chmod 600 "$CERTS_DIR"/* 2>/dev/null || true
fi

# ---- systemd service ----
info "Creating systemd service (${SERVICE_NAME})..."
cat > /etc/systemd/system/${SERVICE_NAME}.service << SVCEOF
[Unit]
Description=Rusty Rules Referee (${SERVICE_NAME}) — Urban Terror Admin Bot
After=network.target

[Service]
Type=simple
User=${REAL_USER}
Group=${REAL_USER}
WorkingDirectory=${INSTALL_DIR}
ExecStart=${INSTALL_DIR}/rusty-rules-referee --mode ${RUN_MODE} r3.toml
Restart=always
RestartSec=3
Environment=RUST_LOG=info

# Hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
ReadWritePaths=${INSTALL_DIR}
PrivateTmp=true

[Install]
WantedBy=multi-user.target
SVCEOF

systemctl daemon-reload
systemctl enable ${SERVICE_NAME}.service >/dev/null 2>&1

# ---- Firewall ----
if command -v ufw &>/dev/null && ufw status 2>/dev/null | grep -q "Status: active"; then
    info "Opening firewall port ${WEB_PORT}/tcp..."
    ufw allow "${WEB_PORT}/tcp" >/dev/null 2>&1 || true
    if [ "$RUN_MODE" = "master" ]; then
        info "Opening firewall port ${MASTER_SYNC_PORT}/tcp (sync API)..."
        ufw allow "${MASTER_SYNC_PORT}/tcp" >/dev/null 2>&1 || true
    fi
fi

# ---- Start ----
info "Starting R3 (${RUN_MODE} mode)..."
systemctl start ${SERVICE_NAME}.service
sleep 2

if systemctl is-active --quiet ${SERVICE_NAME}.service; then
    info "Bot is running!"
else
    warn "Bot may not have started correctly"
    echo -e "    ${DIM}Check logs: journalctl -u ${SERVICE_NAME} -f${NC}"
fi

section "Installation Complete"

echo -e "  ${GREEN}${BOLD}Rusty Rules Referee is installed and running!${NC}"
echo -e "  ${BOLD}Mode${NC}           ${RUN_MODE}"
echo ""

case "$RUN_MODE" in
    standalone)
        echo -e "  ${BOLD}Setup Wizard${NC}   http://${SERVER_IP}:${WEB_PORT}/setup"
        echo -e "  ${BOLD}Config${NC}         $INSTALL_DIR/r3.toml"
        echo -e "  ${BOLD}Service${NC}        systemctl {start|stop|restart|status} ${SERVICE_NAME}"
        echo -e "  ${BOLD}Logs${NC}           journalctl -u ${SERVICE_NAME} -f"
        echo ""
        echo -e "  ${YELLOW}⚠  Open the Setup Wizard URL to create your admin account.${NC}"
        ;;
    master)
        echo -e "  ${BOLD}Setup Wizard${NC}   http://${SERVER_IP}:${WEB_PORT}/setup"
        echo -e "  ${BOLD}Sync API${NC}       https://${SERVER_IP}:${MASTER_SYNC_PORT}"
        echo -e "  ${BOLD}Config${NC}         $INSTALL_DIR/r3.toml"
        echo -e "  ${BOLD}Certificates${NC}   $CERTS_DIR/"
        echo -e "  ${BOLD}Service${NC}        systemctl {start|stop|restart|status} ${SERVICE_NAME}"
        echo -e "  ${BOLD}Logs${NC}           journalctl -u ${SERVICE_NAME} -f"
        echo ""
        echo -e "  ${YELLOW}⚠  Open the Setup Wizard URL to create your admin account.${NC}"
        echo -e "  ${YELLOW}⚠  Then enable Quick-Connect in the web UI to pair client bots.${NC}"
        ;;
    client)
        echo -e "  ${BOLD}Master${NC}         ${PAIR_SYNC_URL}"
        echo -e "  ${BOLD}Server ID${NC}      ${PAIR_SERVER_ID}"
        echo -e "  ${BOLD}Config${NC}         $INSTALL_DIR/r3.toml"
        echo -e "  ${BOLD}Service${NC}        systemctl {start|stop|restart|status} ${SERVICE_NAME}"
        echo -e "  ${BOLD}Logs${NC}           journalctl -u ${SERVICE_NAME} -f"
        echo ""
        echo -e "  ${GREEN}✓  Paired with master! This bot is managed from the master web UI.${NC}"
        echo -e "  ${YELLOW}⚠  Configure game server details (IP, port, RCON) from the master dashboard.${NC}"
        ;;
esac
echo ""

exit 0

__ARCHIVE_MARKER__
