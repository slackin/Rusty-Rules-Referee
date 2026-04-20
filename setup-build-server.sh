#!/bin/bash
# =============================================================================
# setup-build-server.sh — One-time setup for r3.pugbot.net as the build server
#
# Run this ON the update server (10.10.0.4) as root.
# It installs the Rust toolchain, Node.js, clones the repo, and does a first build.
#
# Usage:
#   ssh root@10.10.0.4 'bash -s' < setup-build-server.sh
#   # Or copy to server and run:
#   scp setup-build-server.sh root@10.10.0.4:/tmp/ && ssh root@10.10.0.4 bash /tmp/setup-build-server.sh
#
# After running this, you need to manually:
#   1. Add the generated GitHub deploy key to your repo (printed at the end)
#   2. Run: ssh root@10.10.0.4 'cd /opt/r3-build && git pull'  (to verify GitHub access)
# =============================================================================
set -euo pipefail

# ---- Config ----
BUILD_DIR="/opt/r3-build"
GITHUB_REPO="git@github.com:slackin/Rusty-Rules-Referee.git"
GITHUB_DEPLOY_KEY="$HOME/.ssh/github_deploy"

# ---- Colors ----
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

step()  { echo -e "\n${CYAN}${BOLD}=== $1 ===${NC}"; }
ok()    { echo -e "  ${GREEN}✓${NC} $1"; }
warn()  { echo -e "  ${YELLOW}!${NC} $1"; }
fail()  { echo -e "  ${RED}✗${NC} $1"; exit 1; }

echo -e "${CYAN}${BOLD}"
echo "  ╔══════════════════════════════════════╗"
echo "  ║    R3 Build Server Setup             ║"
echo "  ║    r3.pugbot.net                     ║"
echo "  ╚══════════════════════════════════════╝"
echo -e "${NC}"

# ---- Check we're root ----
if [ "$(id -u)" -ne 0 ]; then
    fail "Must run as root"
fi

# ---- Step 1: System packages ----
step "Installing system packages"
apt-get update -qq
apt-get install -y build-essential pkg-config libssl-dev git curl 2>&1 | tail -3
ok "System packages installed"

# ---- Step 2: Node.js (if not installed) ----
step "Installing Node.js"
if command -v node &>/dev/null; then
    NODE_VER=$(node --version)
    ok "Node.js already installed: $NODE_VER"
else
    curl -fsSL https://deb.nodesource.com/setup_20.x | bash - 2>&1 | tail -3
    apt-get install -y nodejs 2>&1 | tail -3
    ok "Node.js installed: $(node --version)"
fi
ok "npm version: $(npm --version)"

# ---- Step 3: Rust (if not installed) ----
step "Installing Rust"
if command -v rustc &>/dev/null; then
    RUST_VER=$(rustc --version)
    ok "Rust already installed: $RUST_VER"
else
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y 2>&1 | tail -5
    # Source cargo env for this session
    source "$HOME/.cargo/env"
    ok "Rust installed: $(rustc --version)"
fi

# Make sure cargo is in PATH for this session
if ! command -v cargo &>/dev/null; then
    source "$HOME/.cargo/env" 2>/dev/null || true
    export PATH="$HOME/.cargo/bin:$PATH"
fi
ok "cargo version: $(cargo --version)"

# ---- Step 4: GitHub deploy key ----
step "Setting up GitHub SSH deploy key"
if [ -f "$GITHUB_DEPLOY_KEY" ]; then
    ok "Deploy key already exists: $GITHUB_DEPLOY_KEY"
else
    ssh-keygen -t ed25519 -f "$GITHUB_DEPLOY_KEY" -N "" -C "r3-build-server@r3.pugbot.net"
    ok "Deploy key generated: $GITHUB_DEPLOY_KEY"
fi

# Configure SSH to use the deploy key for github.com
SSH_CONFIG="$HOME/.ssh/config"
if ! grep -q "github.com" "$SSH_CONFIG" 2>/dev/null; then
    mkdir -p "$HOME/.ssh"
    cat >> "$SSH_CONFIG" <<EOF

Host github.com
    HostName github.com
    User git
    IdentityFile $GITHUB_DEPLOY_KEY
    StrictHostKeyChecking accept-new
EOF
    chmod 600 "$SSH_CONFIG"
    ok "SSH config updated for github.com"
else
    ok "SSH config already has github.com entry"
fi

# ---- Step 5: Clone repo ----
step "Cloning repository"
if [ -d "$BUILD_DIR/.git" ]; then
    ok "Repository already cloned at $BUILD_DIR"
    cd "$BUILD_DIR"
    git fetch origin 2>&1 || warn "git fetch failed — add the deploy key to GitHub first"
else
    echo ""
    echo -e "  ${YELLOW}${BOLD}ACTION REQUIRED:${NC}"
    echo ""
    echo "  Add this deploy key to your GitHub repo before continuing:"
    echo "  Repo: https://github.com/slackin/Rusty-Rules-Referee/settings/keys"
    echo ""
    echo "  ${BOLD}Public key:${NC}"
    echo ""
    cat "${GITHUB_DEPLOY_KEY}.pub"
    echo ""
    echo -e "  ${YELLOW}Steps:${NC}"
    echo "    1. Go to https://github.com/slackin/Rusty-Rules-Referee/settings/keys"
    echo "    2. Click 'Add deploy key'"
    echo "    3. Title: 'r3-build-server'"
    echo "    4. Paste the public key above"
    echo "    5. Check 'Allow write access'"
    echo "    6. Click 'Add key'"
    echo ""
    read -p "  Press Enter after adding the deploy key to GitHub... " < /dev/tty

    git clone "$GITHUB_REPO" "$BUILD_DIR" || fail "git clone failed — verify the deploy key was added correctly"
    ok "Repository cloned to $BUILD_DIR"
    cd "$BUILD_DIR"
fi

# Set git identity for the build server
git config user.name "R3 Deploy"
git config user.email "deploy@r3.pugbot.net"
ok "Git identity configured"

# ---- Step 6: First build ----
step "Running first build (this may take several minutes)"

cd "$BUILD_DIR"

echo "  Installing npm dependencies..."
cd ui
npm ci --loglevel=error 2>&1 || npm install --loglevel=error 2>&1
ok "npm dependencies installed"

echo "  Building UI..."
npm run build 2>&1 | tail -5
ok "UI built"

cd "$BUILD_DIR"

echo "  Building Rust binary (this is the slow part on first run)..."
cargo build --release 2>&1 | tail -10
ok "Binary built"

BINARY="target/release/rusty-rules-referee"
if [ -f "$BINARY" ]; then
    BUILD_HASH=$("./$BINARY" --build-hash 2>/dev/null | tail -1) || BUILD_HASH="unknown"
    ok "Binary works! Build hash: $BUILD_HASH"
else
    fail "Binary not found after build"
fi

# ---- Step 7: Ensure publish directory exists ----
step "Setting up publish directory"
PUBLISH_BASE="/home/bcmx/domains/r3.pugbot.net/public_html/api/updates"
mkdir -p "${PUBLISH_BASE}/binaries"
ok "Publish directory ready: $PUBLISH_BASE"

# ---- Step 8: Make deploy-remote.sh executable ----
step "Finalizing"
if [ -f "$BUILD_DIR/deploy-remote.sh" ]; then
    chmod +x "$BUILD_DIR/deploy-remote.sh"
    ok "deploy-remote.sh is executable"
fi

# ---- Done ----
echo ""
echo -e "${GREEN}${BOLD}"
echo "  ╔══════════════════════════════════════╗"
echo "  ║    Setup Complete!                   ║"
echo "  ╚══════════════════════════════════════╝"
echo -e "${NC}"
echo ""
echo "  Build server is ready. You can now deploy from your dev machine:"
echo ""
echo "    ${BOLD}Bash:${NC}        ./deploy.sh"
echo "    ${BOLD}PowerShell:${NC}  ./deploy.ps1"
echo ""
echo "  Or run a build directly on this server:"
echo ""
echo "    ${BOLD}$BUILD_DIR/deploy-remote.sh${NC}"
echo ""

# ---- Reminder: SSH key hardening ----
echo -e "  ${YELLOW}${BOLD}Security reminder:${NC}"
echo "    - Set up SSH key auth from your dev machine to this server"
echo "    - Then disable password auth: edit /etc/ssh/sshd_config"
echo "      PasswordAuthentication no"
echo "    - Restart sshd: systemctl restart sshd"
echo "    - Rotate the root password"
echo ""
