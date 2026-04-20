#!/usr/bin/env bash
# ============================================================================
# R3 (Rusty Rules Referee) — Instance Uninstaller
#
# Safely removes a single R3 instance without affecting other installations.
#
# Usage:
#   sudo bash uninstall-r3.sh                 # interactive — lists instances
#   sudo bash uninstall-r3.sh r3              # uninstall the 'r3' service
#   sudo bash uninstall-r3.sh r3-dallas       # uninstall the 'r3-dallas' client
#   sudo bash uninstall-r3.sh --all           # remove ALL r3 instances
#
# What it removes:
#   - systemd service (stop, disable, delete, daemon-reload)
#   - install directory (binary, config, database, logs, certs)
#   - UFW firewall rules opened by the installer
#   - journald logs for the service (optional)
#
# What it does NOT remove:
#   - Other R3 instances
#   - The OS user account (unless you pass --remove-user)
#   - Game server files in ~/urbanterror/ (unless you pass --remove-gameserver)
# ============================================================================
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

log()  { echo -e "${GREEN}✓${NC} $*"; }
warn() { echo -e "${YELLOW}⚠${NC} $*"; }
err()  { echo -e "${RED}✗${NC} $*" >&2; }
info() { echo -e "${CYAN}→${NC} $*"; }

# --------------------------------------------------------------------------
# Require root
# --------------------------------------------------------------------------
if [[ $EUID -ne 0 ]]; then
    err "This script must be run as root (sudo)."
    exit 1
fi

# --------------------------------------------------------------------------
# Parse arguments
# --------------------------------------------------------------------------
SERVICE_NAME=""
REMOVE_ALL=false
REMOVE_USER=false
REMOVE_GAMESERVER=false
SKIP_CONFIRM=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --all)           REMOVE_ALL=true; shift ;;
        --remove-user)   REMOVE_USER=true; shift ;;
        --remove-gameserver) REMOVE_GAMESERVER=true; shift ;;
        -y|--yes)        SKIP_CONFIRM=true; shift ;;
        -h|--help)
            echo "Usage: sudo bash uninstall-r3.sh [SERVICE_NAME] [OPTIONS]"
            echo ""
            echo "Arguments:"
            echo "  SERVICE_NAME          Service name (e.g. 'r3', 'r3-dallas')"
            echo ""
            echo "Options:"
            echo "  --all                 Remove ALL R3 instances"
            echo "  --remove-user         Also remove the OS user (if no other instances remain)"
            echo "  --remove-gameserver   Also remove ~/urbanterror/ game server files"
            echo "  -y, --yes             Skip confirmation prompts"
            echo "  -h, --help            Show this help"
            exit 0
            ;;
        -*)
            err "Unknown option: $1"
            exit 1
            ;;
        *)
            SERVICE_NAME="$1"; shift ;;
    esac
done

# --------------------------------------------------------------------------
# Discover installed R3 instances
# --------------------------------------------------------------------------
discover_instances() {
    local instances=()
    for unit_file in /etc/systemd/system/r3.service /etc/systemd/system/r3-*.service; do
        [[ -f "$unit_file" ]] || continue
        local svc
        svc=$(basename "$unit_file" .service)
        instances+=("$svc")
    done
    echo "${instances[@]}"
}

get_service_info() {
    local svc="$1"
    local unit_file="/etc/systemd/system/${svc}.service"

    if [[ ! -f "$unit_file" ]]; then
        return 1
    fi

    # Extract info from the unit file
    SVC_USER=$(grep -oP '(?<=^User=).*' "$unit_file" 2>/dev/null || echo "")
    SVC_WORKDIR=$(grep -oP '(?<=^WorkingDirectory=).*' "$unit_file" 2>/dev/null || echo "")
    SVC_EXEC=$(grep -oP '(?<=^ExecStart=).*' "$unit_file" 2>/dev/null || echo "")
    SVC_RW_PATHS=$(grep -oP '(?<=^ReadWritePaths=).*' "$unit_file" 2>/dev/null || echo "")

    # Detect mode from ExecStart line
    if echo "$SVC_EXEC" | grep -q -- '--mode master'; then
        SVC_MODE="master"
    elif echo "$SVC_EXEC" | grep -q -- '--mode client'; then
        SVC_MODE="client"
    else
        SVC_MODE="standalone"
    fi

    # Detect ports from config if possible
    SVC_WEB_PORT=""
    SVC_SYNC_PORT=""
    local config_file="${SVC_WORKDIR}/r3.toml"
    if [[ -f "$config_file" ]]; then
        SVC_WEB_PORT=$(grep -oP '(?<=^port\s=\s)\d+' "$config_file" 2>/dev/null | head -1 || echo "")
        if [[ "$SVC_MODE" == "master" ]]; then
            # Master sync port is in [master] section
            SVC_SYNC_PORT=$(awk '/^\[master\]/{found=1} found && /^port\s*=/{print $3; exit}' "$config_file" 2>/dev/null || echo "")
        fi
    fi
}

# --------------------------------------------------------------------------
# Interactive instance selection
# --------------------------------------------------------------------------
INSTANCES=($(discover_instances))

if [[ ${#INSTANCES[@]} -eq 0 ]]; then
    warn "No R3 instances found on this system."
    exit 0
fi

if [[ -z "$SERVICE_NAME" && "$REMOVE_ALL" == false ]]; then
    echo ""
    echo -e "${BOLD}Installed R3 instances:${NC}"
    echo ""
    for i in "${!INSTANCES[@]}"; do
        svc="${INSTANCES[$i]}"
        get_service_info "$svc" || true
        status=$(systemctl is-active "$svc" 2>/dev/null || echo "unknown")
        printf "  ${CYAN}%d)${NC} %-20s  mode=%-12s user=%-12s dir=%s  [%s]\n" \
            $((i+1)) "$svc" "$SVC_MODE" "$SVC_USER" "$SVC_WORKDIR" "$status"
    done
    echo ""
    read -rp "Enter number to uninstall (or 'all'): " choice

    if [[ "$choice" == "all" ]]; then
        REMOVE_ALL=true
    elif [[ "$choice" =~ ^[0-9]+$ ]] && (( choice >= 1 && choice <= ${#INSTANCES[@]} )); then
        SERVICE_NAME="${INSTANCES[$((choice-1))]}"
    else
        err "Invalid selection."
        exit 1
    fi
fi

# --------------------------------------------------------------------------
# Build list of services to remove
# --------------------------------------------------------------------------
TO_REMOVE=()
if [[ "$REMOVE_ALL" == true ]]; then
    TO_REMOVE=("${INSTANCES[@]}")
else
    # Validate the service name
    if [[ ! -f "/etc/systemd/system/${SERVICE_NAME}.service" ]]; then
        err "Service '${SERVICE_NAME}' not found at /etc/systemd/system/${SERVICE_NAME}.service"
        echo ""
        echo "Available instances:"
        for svc in "${INSTANCES[@]}"; do
            echo "  - $svc"
        done
        exit 1
    fi
    TO_REMOVE=("$SERVICE_NAME")
fi

# --------------------------------------------------------------------------
# Confirm
# --------------------------------------------------------------------------
echo ""
echo -e "${BOLD}${RED}The following will be PERMANENTLY removed:${NC}"
echo ""

for svc in "${TO_REMOVE[@]}"; do
    get_service_info "$svc" || true
    echo -e "  ${BOLD}$svc${NC}"
    echo "    Service:   /etc/systemd/system/${svc}.service"
    echo "    Directory: ${SVC_WORKDIR}"
    echo "    User:      ${SVC_USER}"
    echo "    Mode:      ${SVC_MODE}"
    [[ -n "$SVC_WEB_PORT" ]] && echo "    Web port:  ${SVC_WEB_PORT}/tcp (UFW rule)"
    [[ -n "$SVC_SYNC_PORT" ]] && echo "    Sync port: ${SVC_SYNC_PORT}/tcp (UFW rule)"
    echo ""
done

if [[ "$REMOVE_USER" == true ]]; then
    warn "User account removal requested (only if no other R3 instances remain)"
fi
if [[ "$REMOVE_GAMESERVER" == true ]]; then
    warn "Game server directory removal requested"
fi

if [[ "$SKIP_CONFIRM" == false ]]; then
    echo ""
    read -rp "Type 'yes' to confirm uninstall: " confirm
    if [[ "$confirm" != "yes" ]]; then
        echo "Aborted."
        exit 0
    fi
fi

# --------------------------------------------------------------------------
# Uninstall each instance
# --------------------------------------------------------------------------
USERS_CLEANED=()

for svc in "${TO_REMOVE[@]}"; do
    echo ""
    echo -e "${BOLD}━━━ Removing: ${svc} ━━━${NC}"

    get_service_info "$svc" || true
    unit_file="/etc/systemd/system/${svc}.service"

    # 1. Stop and disable systemd service
    info "Stopping service..."
    if systemctl is-active --quiet "$svc" 2>/dev/null; then
        systemctl stop "$svc"
        log "Service stopped"
    else
        log "Service already stopped"
    fi

    info "Disabling service..."
    if systemctl is-enabled --quiet "$svc" 2>/dev/null; then
        systemctl disable "$svc" 2>/dev/null
        log "Service disabled"
    else
        log "Service already disabled"
    fi

    # 2. Remove unit file
    info "Removing unit file..."
    rm -f "$unit_file"
    log "Removed $unit_file"

    # 3. Remove UFW rules (if ufw is active)
    if command -v ufw &>/dev/null && ufw status | grep -q "Status: active"; then
        if [[ -n "$SVC_WEB_PORT" ]]; then
            info "Removing UFW rule for port ${SVC_WEB_PORT}/tcp..."
            ufw delete allow "${SVC_WEB_PORT}/tcp" 2>/dev/null && log "UFW rule removed" || warn "UFW rule not found"
        fi
        if [[ -n "$SVC_SYNC_PORT" ]]; then
            info "Removing UFW rule for port ${SVC_SYNC_PORT}/tcp..."
            ufw delete allow "${SVC_SYNC_PORT}/tcp" 2>/dev/null && log "UFW rule removed" || warn "UFW rule not found"
        fi
    fi

    # 4. Remove install directory
    if [[ -n "$SVC_WORKDIR" && -d "$SVC_WORKDIR" ]]; then
        info "Removing install directory: ${SVC_WORKDIR}"
        rm -rf "$SVC_WORKDIR"
        log "Directory removed"
    else
        warn "Install directory not found: ${SVC_WORKDIR}"
    fi

    # 5. Remove game server files (optional)
    if [[ "$REMOVE_GAMESERVER" == true && -n "$SVC_USER" ]]; then
        local_home=$(eval echo "~${SVC_USER}" 2>/dev/null || echo "")
        if [[ -n "$local_home" && -d "${local_home}/urbanterror" ]]; then
            info "Removing game server: ${local_home}/urbanterror/"
            rm -rf "${local_home}/urbanterror"
            log "Game server files removed"
        fi
    fi

    # 6. Flush journald logs for this unit
    info "Clearing journal logs for ${svc}..."
    journalctl --rotate 2>/dev/null || true
    journalctl --vacuum-time=1s -u "$svc" 2>/dev/null || true
    log "Journal logs cleared"

    # Track user for potential removal
    if [[ -n "$SVC_USER" ]]; then
        USERS_CLEANED+=("$SVC_USER")
    fi

    log "Instance '${svc}' removed"
done

# Reload systemd after all unit files are removed
info "Reloading systemd daemon..."
systemctl daemon-reload
log "Systemd reloaded"

# --------------------------------------------------------------------------
# Optional: remove user account
# --------------------------------------------------------------------------
if [[ "$REMOVE_USER" == true ]]; then
    # Deduplicate users
    declare -A seen_users
    for u in "${USERS_CLEANED[@]}"; do
        seen_users["$u"]=1
    done

    for user in "${!seen_users[@]}"; do
        # Check if any OTHER r3 instances still reference this user
        remaining=false
        for unit_file in /etc/systemd/system/r3.service /etc/systemd/system/r3-*.service; do
            [[ -f "$unit_file" ]] || continue
            if grep -q "^User=${user}$" "$unit_file"; then
                remaining=true
                break
            fi
        done

        if [[ "$remaining" == true ]]; then
            warn "User '${user}' still has other R3 instances — skipping user removal"
        else
            if id "$user" &>/dev/null; then
                info "Removing user '${user}'..."
                userdel "$user" 2>/dev/null && log "User removed" || warn "Could not remove user"
                # Note: not removing home directory — it may contain other data
                warn "Home directory for '${user}' was NOT removed (may contain other data)"
            fi
        fi
    done
fi

# --------------------------------------------------------------------------
# Summary
# --------------------------------------------------------------------------
echo ""
echo -e "${BOLD}${GREEN}━━━ Uninstall Complete ━━━${NC}"
echo ""
for svc in "${TO_REMOVE[@]}"; do
    echo -e "  ${GREEN}✓${NC} ${svc}"
done
echo ""

# Check if any instances remain
remaining_instances=($(discover_instances))
if [[ ${#remaining_instances[@]} -gt 0 ]]; then
    echo -e "  Remaining instances: ${remaining_instances[*]}"
else
    echo -e "  No R3 instances remain on this system."
fi
echo ""
