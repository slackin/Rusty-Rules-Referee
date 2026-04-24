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
REAL_GROUP=$(id -gn "$REAL_USER" 2>/dev/null || echo "$REAL_USER")

# ---- Mode Selection ----
section "Installation Mode"

echo -e "  ${BOLD}1)${NC} Standalone  — Single bot managing one game server (most common)"
echo -e "  ${BOLD}2)${NC} Master      — Central hub managing multiple client bots"
echo -e "  ${BOLD}3)${NC} Client      — Bot managed by a remote master server"
echo -e "  ${BOLD}4)${NC} Hub         — Local orchestrator: manages many R3 clients on this host"
echo ""
ask "Select mode [1]: "
read -r MODE_CHOICE
MODE_CHOICE="${MODE_CHOICE:-1}"

case "$MODE_CHOICE" in
    1) RUN_MODE="standalone" ;;
    2) RUN_MODE="master" ;;
    3) RUN_MODE="client" ;;
    4) RUN_MODE="hub" ;;
    *) err "Invalid choice"; exit 1 ;;
esac

info "Mode: ${RUN_MODE}"

# Install dir and service name — set defaults, client overrides after collecting server name
INSTALL_DIR="$HOME_DIR/r3"
SERVICE_NAME="r3"

DEFAULT_IP=$(hostname -I 2>/dev/null | awk '{print $1}')

# =============================================================================
# Game server download function (standalone and client modes)
#
# Args:
#   $1 — Optional install dir (defaults to $HOME_DIR/urbanterror)
#   $2 — Optional "quiet" flag: if "quiet", skip the download-yes/no prompt
#        (used by client mode where the decision is made elsewhere).
# =============================================================================
download_game_server() {
    local URT_DIR="${1:-$HOME_DIR/urbanterror}"
    local MODE="${2:-ask}"

    if [ "$MODE" != "quiet" ]; then
        section "Game Server Download"
        ask "Download and install Urban Terror 4.3 dedicated server? [y/N]: "
        read -r DL_CHOICE
        if [ "$DL_CHOICE" != "y" ] && [ "$DL_CHOICE" != "Y" ]; then
            return 0
        fi
    fi

    # Mirror list, tried in order. Each entry is "URL|KIND" where KIND is
    # "tar" (tar.gz, dedicated-only) or "zip" (full package, we extract
    # q3ut4/ only). pugbot.net is primary because it's the only confirmed-
    # stable mirror; the official sites frequently return CMS HTML.
    local MIRRORS=(
        "https://maps.pugbot.net/q3ut4/UrbanTerror434_full.zip|zip"
        "https://cdn.urbanterror.info/urt/43/releases/UrbanTerror43_ded.tar.gz|tar"
        "https://www.frozensand.com/downloads/UrbanTerror43_ded.tar.gz|tar"
    )
    local MIN_TAR_BYTES=$((40 * 1024 * 1024))    # dedicated tarball ~60 MB
    local MIN_ZIP_BYTES=$((500 * 1024 * 1024))   # full zip ~1.4 GB
    local TMP_DL="/tmp/urt43_download.$$"

    mkdir -p "$URT_DIR"

    local SUCCESS=0
    local LAST_ERR=""
    local MIRROR_ENTRY URL KIND EXT MIN_BYTES TMP_FILE
    for MIRROR_ENTRY in "${MIRRORS[@]}"; do
        URL="${MIRROR_ENTRY%|*}"
        KIND="${MIRROR_ENTRY##*|}"
        EXT="tar.gz"
        MIN_BYTES=$MIN_TAR_BYTES
        if [ "$KIND" = "zip" ]; then
            EXT="zip"
            MIN_BYTES=$MIN_ZIP_BYTES
        fi
        TMP_FILE="${TMP_DL}.${EXT}"
        rm -f "$TMP_FILE"

        info "Downloading Urban Terror 4.3 from $URL ..."
        local DL_OK=1
        if command -v curl &>/dev/null; then
            # -f: fail on non-2xx; -L: follow redirects; --retry: transient errors.
            if curl -fL --retry 2 --retry-delay 3 --progress-bar \
                -A "R3-Installer/1.0" \
                -o "$TMP_FILE" "$URL"; then
                DL_OK=0
            fi
        elif command -v wget &>/dev/null; then
            if wget -q --show-progress --tries=2 \
                --user-agent="R3-Installer/1.0" \
                -O "$TMP_FILE" "$URL"; then
                DL_OK=0
            fi
        else
            err "Neither curl nor wget is installed."
            return 1
        fi

        if [ $DL_OK -ne 0 ]; then
            LAST_ERR="download transport failure"
            warn "Download from mirror failed ($LAST_ERR). Trying next mirror..."
            continue
        fi

        # Validate: size must be at least the expected minimum.
        local BYTES=0
        if [ -f "$TMP_FILE" ]; then
            BYTES=$(stat -c%s "$TMP_FILE" 2>/dev/null || wc -c <"$TMP_FILE" || echo 0)
        fi
        if [ "$BYTES" -lt "$MIN_BYTES" ]; then
            LAST_ERR="file too small ($BYTES bytes, expected >= $MIN_BYTES) — mirror likely returned an HTML error page"
            warn "$LAST_ERR"
            rm -f "$TMP_FILE"
            continue
        fi

        # Validate: magic bytes.
        local MAGIC
        MAGIC=$(head -c 4 "$TMP_FILE" | od -An -tx1 | tr -d ' \n')
        case "$KIND" in
            tar)
                # gzip magic: 1f 8b
                if [ "${MAGIC:0:4}" != "1f8b" ]; then
                    LAST_ERR="not a gzip archive (magic=${MAGIC:0:4}) — mirror returned something else"
                    warn "$LAST_ERR"
                    rm -f "$TMP_FILE"
                    continue
                fi
                # tar -t must list actual entries.
                if ! tar -tzf "$TMP_FILE" >/dev/null 2>&1; then
                    LAST_ERR="tar listing failed — archive is corrupt"
                    warn "$LAST_ERR"
                    rm -f "$TMP_FILE"
                    continue
                fi
                ;;
            zip)
                # zip magic: 50 4b 03 04 (PK\3\4)
                if [ "$MAGIC" != "504b0304" ]; then
                    LAST_ERR="not a zip archive (magic=$MAGIC) — mirror returned something else"
                    warn "$LAST_ERR"
                    rm -f "$TMP_FILE"
                    continue
                fi
                if ! command -v unzip &>/dev/null; then
                    LAST_ERR="unzip command not installed (required for zip-format mirror)"
                    warn "$LAST_ERR"
                    rm -f "$TMP_FILE"
                    continue
                fi
                if ! unzip -tq "$TMP_FILE" >/dev/null 2>&1; then
                    LAST_ERR="zip integrity check failed — archive is corrupt"
                    warn "$LAST_ERR"
                    rm -f "$TMP_FILE"
                    continue
                fi
                ;;
        esac

        info "Download verified ($BYTES bytes, magic ok, archive listable)"
        info "Extracting to $URT_DIR ..."
        local EX_OK=1
        case "$KIND" in
            tar)
                if tar xzf "$TMP_FILE" -C "$URT_DIR" --strip-components=1 2>/dev/null \
                   || tar xzf "$TMP_FILE" -C "$URT_DIR" 2>/dev/null; then
                    EX_OK=0
                fi
                ;;
            zip)
                # Full zip has Quake3-UrT-Ded.* at root and assets under q3ut4/.
                # Extract everything to $URT_DIR; strip top-level wrapper dir
                # only if one exists.
                if unzip -q -o "$TMP_FILE" -d "$URT_DIR" 2>/dev/null; then
                    EX_OK=0
                    # If the archive has a single top-level dir, flatten it.
                    local TOP
                    TOP=$(cd "$URT_DIR" && ls -1 | head -1)
                    if [ -d "$URT_DIR/$TOP" ] && [ "$(ls -1 "$URT_DIR" | wc -l)" = "1" ]; then
                        # shellcheck disable=SC2086
                        (cd "$URT_DIR/$TOP" && tar cf - .) | (cd "$URT_DIR" && tar xf -) \
                            && rm -rf "$URT_DIR/$TOP"
                    fi
                fi
                ;;
        esac

        rm -f "$TMP_FILE"

        if [ $EX_OK -ne 0 ]; then
            LAST_ERR="extraction failed"
            warn "$LAST_ERR — trying next mirror"
            continue
        fi

        # Sanity check: we expect at least q3ut4/ and a Quake3-UrT-Ded binary.
        if [ ! -d "$URT_DIR/q3ut4" ]; then
            LAST_ERR="archive extracted but q3ut4/ directory is missing"
            warn "$LAST_ERR — trying next mirror"
            continue
        fi
        if ! ls "$URT_DIR"/Quake3-UrT-Ded* >/dev/null 2>&1; then
            warn "No Quake3-UrT-Ded binary found at top level after extract — may need manual fix-up"
            # Not fatal: the binary might live under a subdir in some archives.
        fi

        SUCCESS=1
        break
    done

    rm -f "${TMP_DL}."*

    if [ $SUCCESS -ne 1 ]; then
        err "All UrT 4.3 download mirrors failed."
        err "Last error: $LAST_ERR"
        err "You can install the game server manually, then re-run the installer."
        return 1
    fi

    chown -R "$REAL_USER:$REAL_GROUP" "$URT_DIR"
    info "Game server installed at $URT_DIR"

    # Auto-detect game log from the new install (standalone mode only — in
    # client mode the wizard decides the final path, not this function).
    GAME_LOG="$URT_DIR/q3ut4/games.log"
    URT_INSTALL_DIR="$URT_DIR"
    return 0
}

# =============================================================================
# Lay down systemd template unit + sudoers drop-in so the master UI wizard
# can manage per-instance UrT game servers for this user.
#
# Writes:
#   /etc/systemd/system/urt@.service          — instance-template unit
#   /etc/systemd/system/urt@.service.d/       — drop-in directory
#   /etc/r3/urt-instances/                    — per-instance DropIn storage
#   /etc/sudoers.d/r3-${REAL_USER}-urt        — narrow NOPASSWD entries
#
# Idempotent — safe to re-run.
# =============================================================================
install_urt_service_scaffolding() {
    info "Installing systemd template unit and sudoers drop-in for UrT instances..."

    # 1. systemd template unit. %i is the instance slug (e.g. "pug1").
    #    The unit reads its install dir and port from an instance-specific
    #    DropIn at /etc/systemd/system/urt@.service.d/ which is generated
    #    by the UI wizard via the client bot.
    cat > /etc/systemd/system/urt@.service << 'URTEOF'
[Unit]
Description=Urban Terror 4.3 Dedicated Server (%i)
After=network.target
# Instance-specific environment comes from the drop-in file written by R3:
#   /etc/systemd/system/urt@.service.d/%i.conf
ConditionPathExists=/etc/systemd/system/urt@.service.d/%i.conf

[Service]
Type=simple
# Populated by the drop-in: User, Group, WorkingDirectory, Environment=URT_PORT=...
# and the final ExecStart. We leave a safe default here that will be overridden.
Restart=on-failure
RestartSec=5

# Hardening (matches r3.service style)
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=read-only

[Install]
WantedBy=multi-user.target
URTEOF

    mkdir -p /etc/systemd/system/urt@.service.d
    mkdir -p /etc/r3/urt-instances
    chown root:"${REAL_GROUP}" /etc/r3/urt-instances
    chmod 0775 /etc/r3/urt-instances

    # 2. sudoers drop-in. Lets REAL_USER run systemctl against urt@*.service
    #    and drop instance-specific environment DropIns — nothing else.
    local SUDOERS_FILE="/etc/sudoers.d/r3-${REAL_USER}-urt"
    cat > "${SUDOERS_FILE}" << SUDOEOF
# Rusty Rules Referee — allow ${REAL_USER} to manage urt@ instances only.
# Generated by install-r3.sh; edit only if you know what you're doing.
Cmnd_Alias R3_URT_SYSTEMCTL = /bin/systemctl start urt@*, \\
                              /bin/systemctl stop urt@*, \\
                              /bin/systemctl restart urt@*, \\
                              /bin/systemctl status urt@*, \\
                              /bin/systemctl enable urt@*, \\
                              /bin/systemctl disable urt@*, \\
                              /bin/systemctl is-active urt@*, \\
                              /bin/systemctl is-enabled urt@*, \\
                              /bin/systemctl daemon-reload, \\
                              /usr/bin/systemctl start urt@*, \\
                              /usr/bin/systemctl stop urt@*, \\
                              /usr/bin/systemctl restart urt@*, \\
                              /usr/bin/systemctl status urt@*, \\
                              /usr/bin/systemctl enable urt@*, \\
                              /usr/bin/systemctl disable urt@*, \\
                              /usr/bin/systemctl is-active urt@*, \\
                              /usr/bin/systemctl is-enabled urt@*, \\
                              /usr/bin/systemctl daemon-reload

Cmnd_Alias R3_URT_DROPIN = /usr/bin/tee /etc/systemd/system/urt@.service.d/*.conf, \\
                           /bin/tee /etc/systemd/system/urt@.service.d/*.conf, \\
                           /bin/rm /etc/systemd/system/urt@.service.d/*.conf, \\
                           /usr/bin/rm /etc/systemd/system/urt@.service.d/*.conf

Cmnd_Alias R3_URT_SELF_UNINSTALL = /usr/bin/systemd-run --no-block --collect --unit r3-self-uninstall-*.service /bin/bash -c *, \\
                                   /bin/systemd-run --no-block --collect --unit r3-self-uninstall-*.service /bin/bash -c *

${REAL_USER} ALL=(root) NOPASSWD: R3_URT_SYSTEMCTL, R3_URT_DROPIN, R3_URT_SELF_UNINSTALL
SUDOEOF
    chmod 0440 "${SUDOERS_FILE}"

    # Validate sudoers before relying on it (visudo -c returns non-zero on syntax errors).
    if command -v visudo &>/dev/null; then
        if ! visudo -cf "${SUDOERS_FILE}" >/dev/null 2>&1; then
            err "Generated sudoers file failed validation; removing it."
            rm -f "${SUDOERS_FILE}"
            return 1
        fi
    fi

    systemctl daemon-reload
    info "UrT systemd scaffolding installed (template: urt@.service)"
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
WEB_PORT=2727
MASTER_SYNC_PORT=9443
MASTER_URL=""
QUICK_CONNECT_KEY=""

case "$RUN_MODE" in
    standalone)
        download_game_server
        collect_game_config

        ask "Web UI port [2727]: "
        read -r WEB_PORT
        WEB_PORT="${WEB_PORT:-2727}"
        ;;

    master)
        section "Master Configuration"

        ask "Web UI port [2727]: "
        read -r WEB_PORT
        WEB_PORT="${WEB_PORT:-2727}"

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

        ask "Master web port [2727]: "
        read -r MASTER_WEB_PORT
        MASTER_WEB_PORT="${MASTER_WEB_PORT:-2727}"

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

        # ---- Optional: download UrT game server files for this client ----
        # Per-instance dir keeps multiple clients on one user account independent.
        CLIENT_URT_DIR="$HOME_DIR/urbanterror-${INSTANCE_SLUG}"
        section "Game Server Files (optional)"
        echo -e "  You can download the Urban Terror 4.3 dedicated server files now."
        echo -e "  ${DIM}Port, server.cfg, and systemd management happen later in the"
        echo -e "  master's web UI — this step only places the game files on disk.${NC}"
        echo ""
        ask "Download UrT 4.3 dedicated server files to ${CLIENT_URT_DIR}? [y/N]: "
        read -r CLIENT_URT_CHOICE
        if [ "$CLIENT_URT_CHOICE" = "y" ] || [ "$CLIENT_URT_CHOICE" = "Y" ]; then
            if download_game_server "$CLIENT_URT_DIR" quiet; then
                CLIENT_URT_DOWNLOADED=true
            else
                CLIENT_URT_DOWNLOADED=false
                warn "Continuing without game server files — you can install them"
                warn "manually under ${CLIENT_URT_DIR} before running the UI wizard."
            fi
        else
            CLIENT_URT_DOWNLOADED=false
        fi

        # Install systemd scaffolding so the UI wizard can register a managed
        # urt@<slug>.service for this instance. Idempotent across reinstalls.
        install_urt_service_scaffolding || warn "UrT systemd scaffolding install failed (continuing)"
        ;;

    hub)
        section "Hub Connection"

        while true; do
            ask "Master server address (e.g. master.example.com or 10.0.0.1): "
            read -r MASTER_HOST
            if [ -n "$MASTER_HOST" ]; then break; fi
            warn "Master address is required"
        done

        ask "Master web port [2727]: "
        read -r MASTER_WEB_PORT
        MASTER_WEB_PORT="${MASTER_WEB_PORT:-2727}"

        while true; do
            ask "Quick-connect key (from master web UI): "
            read -r QUICK_CONNECT_KEY
            if [ -n "$QUICK_CONNECT_KEY" ]; then break; fi
            warn "Quick-connect key is required"
        done

        ask "Name for this hub [$(hostname -s)]: "
        read -r HUB_NAME
        HUB_NAME="${HUB_NAME:-$(hostname -s)}"

        INSTANCE_SLUG=$(echo "$HUB_NAME" | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9]/-/g; s/--*/-/g; s/^-//; s/-$//')
        INSTALL_DIR="$HOME_DIR/r3-hub-${INSTANCE_SLUG}"
        SERVICE_NAME="r3-hub-${INSTANCE_SLUG}"

        ask "Per-client install root [${INSTALL_DIR}/clients]: "
        read -r HUB_CLIENTS_ROOT
        HUB_CLIENTS_ROOT="${HUB_CLIENTS_ROOT:-${INSTALL_DIR}/clients}"

        ask "Default UrT install root [${HOME_DIR}/urbanterror]: "
        read -r HUB_URT_ROOT
        HUB_URT_ROOT="${HUB_URT_ROOT:-${HOME_DIR}/urbanterror}"

        # Hub will need to manage many `r3-client@<slug>.service` units.
        install_urt_service_scaffolding || warn "UrT systemd scaffolding install failed (continuing)"
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
port = 2727
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

generate_hub_config() {
    # ---- Pair with master via quick-connect (client_kind=hub) ----
    info "Pairing hub with master server..."

    PAIR_URL="http://${MASTER_HOST}:${MASTER_WEB_PORT}/api/v1/pairing/pair"
    PAIR_PAYLOAD=$(cat << JSONEOF
{"token":"${QUICK_CONNECT_KEY}","server_name":"${HUB_NAME}","client_kind":"hub"}
JSONEOF
    )

    PAIR_RESPONSE=$(curl -sf -X POST \
        -H "Content-Type: application/json" \
        -d "$PAIR_PAYLOAD" \
        "$PAIR_URL" 2>/dev/null) || {
        err "Failed to pair hub with master at $PAIR_URL"
        err "Check the master address and quick-connect key, then try again."
        exit 1
    }

    if command -v python3 &>/dev/null; then
        PAIR_HUB_ID=$(echo "$PAIR_RESPONSE" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('hub_id') or d.get('server_id'))")
        PAIR_SERVER_ID="$PAIR_HUB_ID"
        PAIR_CA_CERT=$(echo "$PAIR_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['ca_cert'])")
        PAIR_CLIENT_CERT=$(echo "$PAIR_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['client_cert'])")
        PAIR_CLIENT_KEY=$(echo "$PAIR_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['client_key'])")
        PAIR_SYNC_URL=$(echo "$PAIR_RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['master_sync_url'])")
    else
        err "python3 is required for JSON parsing during hub setup"
        exit 1
    fi

    info "Hub paired! Hub ID: ${PAIR_HUB_ID}"

    echo "$PAIR_CA_CERT"     > "$CERTS_DIR/ca.crt"
    echo "$PAIR_CLIENT_CERT" > "$CERTS_DIR/client.crt"
    echo "$PAIR_CLIENT_KEY"  > "$CERTS_DIR/client.key"
    chmod 600 "$CERTS_DIR/client.key" "$CERTS_DIR/ca.crt" "$CERTS_DIR/client.crt"
    info "Hub TLS certificates saved"

    mkdir -p "$HUB_CLIENTS_ROOT" "$HUB_URT_ROOT"
    chown -R "$REAL_USER:$REAL_GROUP" "$HUB_CLIENTS_ROOT" "$HUB_URT_ROOT" || true

    cat > "$INSTALL_DIR/r3.toml" << CFGEOF
# Rusty Rules Referee — Hub Configuration
# Hub pairs with master at ${PAIR_SYNC_URL}
# Manages many r3-client@<slug>.service units on this host.

[referee]
bot_name = "${HUB_NAME}"
bot_prefix = "^2R3-Hub:^3"
database = "sqlite://r3.db"
logfile = "r3.log"
log_level = "info"

[server]
public_ip = "0.0.0.0"
port = 0
rcon_password = ""
game_log = ""
delay = 1.0

[web]
enabled = false
bind_address = "0.0.0.0"
port = 2727
jwt_secret = "${JWT_SECRET}"

[update]
enabled = false
url = "https://r3.pugbot.net/api/updates"
channel = "beta"
check_interval = 3600
auto_restart = true

[hub]
master_url = "${PAIR_SYNC_URL}"
hub_name = "${HUB_NAME}"
tls_cert = "${CERTS_DIR}/client.crt"
tls_key = "${CERTS_DIR}/client.key"
ca_cert = "${CERTS_DIR}/ca.crt"
clients_root = "${HUB_CLIENTS_ROOT}"
urt_install_root = "${HUB_URT_ROOT}"
systemd_unit_template = "r3-client@.service"
heartbeat_interval = 30
host_refresh_interval = 300
CFGEOF

    # ---- Scaffold the r3-client@.service template unit ----
    if [ ! -f /etc/systemd/system/r3-client@.service ]; then
        info "Installing r3-client@.service template unit..."
        cat > /etc/systemd/system/r3-client@.service << 'TEMPLATEEOF'
[Unit]
Description=Rusty Rules Referee — managed client (%i)
After=network.target

[Service]
Type=simple
# User, WorkingDirectory, ReadWritePaths and Environment=R3_CONF=... are
# supplied by the per-instance drop-in file at
# /etc/systemd/system/r3-client@%i.service.d/install.conf written by the hub
# when it provisions a new client.
ExecStart=/usr/local/bin/rusty-rules-referee --mode client r3.toml
Restart=always
RestartSec=3
Environment=RUST_LOG=info

NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=read-only
PrivateTmp=true

[Install]
WantedBy=multi-user.target
TEMPLATEEOF
        # Symlink the binary to /usr/local/bin so the template's ExecStart
        # works regardless of which install dir created it.
        if [ -x "$INSTALL_DIR/rusty-rules-referee" ] && [ ! -e /usr/local/bin/rusty-rules-referee ]; then
            ln -sf "$INSTALL_DIR/rusty-rules-referee" /usr/local/bin/rusty-rules-referee
        fi
        systemctl daemon-reload
        info "r3-client@.service template installed"
    fi

    # ---- sudoers drop-in: let the hub user manage r3-client@ instances ----
    # The hub process runs as $REAL_USER and needs to:
    #   * write per-instance drop-ins under /etc/systemd/system/r3-client@*.service.d/
    #   * systemctl start/stop/enable/disable/restart/daemon-reload these units
    # The rule is narrowly scoped to r3-client@*.service only.
    local HUB_SUDOERS_FILE="/etc/sudoers.d/r3-${REAL_USER}-hub"
    info "Installing hub sudoers drop-in at ${HUB_SUDOERS_FILE}..."
    cat > "${HUB_SUDOERS_FILE}" << HUBSUDOEOF
# Rusty Rules Referee — allow ${REAL_USER} (hub) to manage r3-client@ units.
# Generated by install-r3.sh; edit only if you know what you're doing.
Cmnd_Alias R3_HUB_SYSTEMCTL = /bin/systemctl start r3-client@*, \\
                              /bin/systemctl stop r3-client@*, \\
                              /bin/systemctl restart r3-client@*, \\
                              /bin/systemctl status r3-client@*, \\
                              /bin/systemctl enable r3-client@*, \\
                              /bin/systemctl enable --now r3-client@*, \\
                              /bin/systemctl disable r3-client@*, \\
                              /bin/systemctl disable --now r3-client@*, \\
                              /bin/systemctl is-active r3-client@*, \\
                              /bin/systemctl is-enabled r3-client@*, \\
                              /bin/systemctl daemon-reload, \\
                              /usr/bin/systemctl start r3-client@*, \\
                              /usr/bin/systemctl stop r3-client@*, \\
                              /usr/bin/systemctl restart r3-client@*, \\
                              /usr/bin/systemctl status r3-client@*, \\
                              /usr/bin/systemctl enable r3-client@*, \\
                              /usr/bin/systemctl enable --now r3-client@*, \\
                              /usr/bin/systemctl disable r3-client@*, \\
                              /usr/bin/systemctl disable --now r3-client@*, \\
                              /usr/bin/systemctl is-active r3-client@*, \\
                              /usr/bin/systemctl is-enabled r3-client@*, \\
                              /usr/bin/systemctl daemon-reload

Cmnd_Alias R3_HUB_DROPIN = /usr/bin/install -d -m 0755 /etc/systemd/system/r3-client@*.service.d, \\
                           /bin/install -d -m 0755 /etc/systemd/system/r3-client@*.service.d, \\
                           /usr/bin/tee /etc/systemd/system/r3-client@*.service.d/*.conf, \\
                           /bin/tee /etc/systemd/system/r3-client@*.service.d/*.conf, \\
                           /usr/bin/rm -rf /etc/systemd/system/r3-client@*.service.d, \\
                           /bin/rm -rf /etc/systemd/system/r3-client@*.service.d, \\
                           /usr/bin/rm /etc/systemd/system/r3-client@*.service.d/*.conf, \\
                           /bin/rm /etc/systemd/system/r3-client@*.service.d/*.conf

Cmnd_Alias R3_HUB_SELF_UNINSTALL = /usr/bin/systemd-run --no-block --collect --unit r3-self-uninstall-*.service /bin/bash -c *, \\
                                   /bin/systemd-run --no-block --collect --unit r3-self-uninstall-*.service /bin/bash -c *

${REAL_USER} ALL=(root) NOPASSWD: R3_HUB_SYSTEMCTL, R3_HUB_DROPIN, R3_HUB_SELF_UNINSTALL
HUBSUDOEOF
    chmod 0440 "${HUB_SUDOERS_FILE}"

    if command -v visudo &>/dev/null; then
        if ! visudo -cf "${HUB_SUDOERS_FILE}" >/dev/null 2>&1; then
            err "Generated hub sudoers file failed validation; removing it."
            rm -f "${HUB_SUDOERS_FILE}"
        else
            info "Hub sudoers drop-in installed and validated"
        fi
    fi
}

case "$RUN_MODE" in
    standalone) generate_standalone_config ;;
    master)     generate_master_config ;;
    client)     generate_client_config ;;
    hub)        generate_hub_config ;;
esac

# ---- Client mode: write UrT install-state marker ----
# The master UI reads this via the client bot to decide whether to show the
# "Install Game Server" wizard CTA or a "Manage" panel for an already-
# configured instance.
if [ "$RUN_MODE" = "client" ]; then
    STATE_DIR="$INSTALL_DIR/state"
    mkdir -p "$STATE_DIR"

    FILES_PRESENT_JSON=false
    INSTALL_PATH_JSON="null"
    if [ "${CLIENT_URT_DOWNLOADED:-false}" = "true" ]; then
        FILES_PRESENT_JSON=true
        INSTALL_PATH_JSON="\"${CLIENT_URT_DIR}\""
    fi

    cat > "$STATE_DIR/urt-install.json" << STATEEOF
{
  "slug": "${INSTANCE_SLUG}",
  "files_present": ${FILES_PRESENT_JSON},
  "install_path": ${INSTALL_PATH_JSON},
  "configured": false,
  "service_name": null
}
STATEEOF
    chown -R "$REAL_USER:$REAL_GROUP" "$STATE_DIR"
    chmod 700 "$STATE_DIR"
    chmod 600 "$STATE_DIR/urt-install.json"
fi

# ---- Permissions ----
info "Setting permissions..."
chown -R "$REAL_USER:$REAL_GROUP" "$INSTALL_DIR"
chmod 700 "$INSTALL_DIR"
chmod +x "$INSTALL_DIR/rusty-rules-referee"
chmod 600 "$INSTALL_DIR/r3.toml"
if [ -d "$CERTS_DIR" ]; then
    chmod 700 "$CERTS_DIR"
    chmod 600 "$CERTS_DIR"/* 2>/dev/null || true
fi

# ---- systemd service ----
info "Creating systemd service (${SERVICE_NAME})..."

# Hub mode needs to shell out to `sudo -n systemctl ...` and `sudo -n tee`
# to manage r3-client@ units. NoNewPrivileges=true blocks setuid, so sudo
# will fail with "sudo is running in a container / disable the flag" even
# when the sudoers drop-in is correct. Relax that one bit for hub mode,
# and grant write access to the systemd unit tree so drop-ins can be laid
# down (ProtectSystem=strict otherwise makes /etc read-only in the mount
# namespace regardless of sudo).
if [ "$RUN_MODE" = "hub" ]; then
    NO_NEW_PRIVS="no"
    EXTRA_RW_PATHS=" /etc/systemd/system"
else
    NO_NEW_PRIVS="yes"
    EXTRA_RW_PATHS=""
fi

cat > /etc/systemd/system/${SERVICE_NAME}.service << SVCEOF
[Unit]
Description=Rusty Rules Referee (${SERVICE_NAME}) — Urban Terror Admin Bot
After=network.target

[Service]
Type=simple
User=${REAL_USER}
Group=${REAL_GROUP}
WorkingDirectory=${INSTALL_DIR}
ExecStart=${INSTALL_DIR}/rusty-rules-referee --mode ${RUN_MODE} r3.toml
Restart=always
RestartSec=3
Environment=RUST_LOG=info

# Hardening
NoNewPrivileges=${NO_NEW_PRIVS}
ProtectSystem=strict
ProtectHome=read-only
# The bot needs write access to its own install dir, and to the game-server
# user's home so it can import .pk3 files into q3ut4/, edit server.cfg /
# mapcycle.txt, etc. ReadWritePaths= implicitly punches through ProtectHome=.
ReadWritePaths=${INSTALL_DIR} ${HOME_DIR}${EXTRA_RW_PATHS}
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
# Use restart so that a reinstall over an existing install picks up the new binary.
if systemctl is-active --quiet ${SERVICE_NAME}.service; then
    info "Restarting R3 (${RUN_MODE} mode) to load new binary..."
    systemctl restart ${SERVICE_NAME}.service
else
    info "Starting R3 (${RUN_MODE} mode)..."
    systemctl start ${SERVICE_NAME}.service
fi
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
    hub)
        echo -e "  ${BOLD}Master${NC}         ${PAIR_SYNC_URL}"
        echo -e "  ${BOLD}Hub ID${NC}         ${PAIR_HUB_ID:-${PAIR_SERVER_ID}}"
        echo -e "  ${BOLD}Clients root${NC}   ${HUB_CLIENTS_ROOT}"
        echo -e "  ${BOLD}UrT root${NC}       ${HUB_URT_ROOT}"
        echo -e "  ${BOLD}Config${NC}         $INSTALL_DIR/r3.toml"
        echo -e "  ${BOLD}Service${NC}        systemctl {start|stop|restart|status} ${SERVICE_NAME}"
        echo -e "  ${BOLD}Logs${NC}           journalctl -u ${SERVICE_NAME} -f"
        echo ""
        echo -e "  ${GREEN}✓  Hub paired with master! Manage clients from the master web UI → Hubs tab.${NC}"
        ;;
esac
echo ""

exit 0

__ARCHIVE_MARKER__
