#!/bin/bash
# =============================================================================
# deploy.sh — Deploy R3 from your dev machine (bash / Git Bash / WSL)
#
# This script:
#   1. Checks for uncommitted changes and auto-commits
#   2. Pushes to GitHub
#   3. SSHs to the build server (r3.pugbot.net) and runs deploy-remote.sh
#   4. The build server pulls, builds, and publishes the update
#   5. Game servers auto-update from the published manifest
#
# Usage:
#   ./deploy.sh                     # auto-commit with default message
#   ./deploy.sh "fix censor bug"    # auto-commit with custom message
#   ./deploy.sh --no-commit         # skip commit (must be pushed already)
#
# Prerequisites:
#   - SSH key auth to root@10.10.0.4 (r3.pugbot.net)
#   - Build server set up via setup-build-server.sh
# =============================================================================
set -euo pipefail

# ---- Config ----
BUILD_SERVER="root@10.10.0.4"
BUILD_DIR="/opt/r3-build"
REQUIRED_BRANCH="main"

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
fail()  { echo -e "  ${RED}✗${NC} $1"; }

die() {
    fail "$1"
    exit 1
}

# ---- Parse args ----
COMMIT_MSG=""
NO_COMMIT=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --no-commit) NO_COMMIT=true; shift ;;
        --help|-h)
            echo 'Usage: ./deploy.sh ["commit message"] [--no-commit]'
            echo '  "message"     Custom commit message (default: "deploy: vX.Y.Z")'
            echo '  --no-commit   Skip auto-commit, just push and build'
            exit 0
            ;;
        *) COMMIT_MSG="$1"; shift ;;
    esac
done

echo -e "${CYAN}${BOLD}"
echo "  ╔══════════════════════════════════════╗"
echo "  ║       R3 Deploy Pipeline             ║"
echo "  ╚══════════════════════════════════════╝"
echo -e "${NC}"

# ---- Pre-flight: Verify git repo ----
if [ ! -d ".git" ]; then
    die "Not in a git repository. Run from the R3 project root."
fi

# ---- Pre-flight: Check branch ----
step "Pre-flight checks"
BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$BRANCH" != "$REQUIRED_BRANCH" ]; then
    warn "On branch '$BRANCH' (expected '$REQUIRED_BRANCH')"
    read -p "  Deploy from '$BRANCH' anyway? [y/N] " -r REPLY < /dev/tty
    if [[ ! "$REPLY" =~ ^[Yy]$ ]]; then
        die "Aborted. Switch to $REQUIRED_BRANCH first."
    fi
fi
ok "Branch: $BRANCH"

# ---- Pre-flight: Check SSH connectivity ----
if ! ssh -o ConnectTimeout=5 -o BatchMode=yes "$BUILD_SERVER" "echo ok" &>/dev/null; then
    die "Cannot SSH to $BUILD_SERVER — set up SSH key auth first (see setup-build-server.sh)"
fi
ok "SSH to $BUILD_SERVER: connected"

# ---- Step: Auto-commit ----
step "Git commit & push"

if [ "$NO_COMMIT" = false ]; then
    # Check for changes (staged + unstaged + untracked)
    if [ -n "$(git status --porcelain)" ]; then
        # Get version from Cargo.toml
        VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')

        if [ -z "$COMMIT_MSG" ]; then
            COMMIT_MSG="deploy: v${VERSION}"
        else
            COMMIT_MSG="deploy: v${VERSION} — ${COMMIT_MSG}"
        fi

        git add -A
        git commit -m "$COMMIT_MSG"
        ok "Committed: $COMMIT_MSG"
    else
        ok "Working tree clean, nothing to commit"
    fi
else
    ok "Skipping commit (--no-commit)"
fi

# ---- Step: Push to GitHub ----
git push origin "$BRANCH" || die "git push failed — resolve conflicts first"
COMMIT=$(git rev-parse --short=8 HEAD)
ok "Pushed $COMMIT to origin/$BRANCH"

# ---- Step: Build & publish on update server ----
step "Building on r3.pugbot.net"
echo ""

# Source cargo env on the remote in case it's not in the default login shell PATH
ssh -o StrictHostKeyChecking=accept-new -o ServerAliveInterval=30 -o ServerAliveCountMax=60 \
    "$BUILD_SERVER" "bash ${BUILD_DIR}/deploy-remote.sh" \
    || die "Remote build failed — SSH to $BUILD_SERVER and check ${BUILD_DIR}/deploy-remote.sh"

# ---- Done ----
echo ""
echo -e "${GREEN}${BOLD}  Deploy complete!${NC} Game servers will auto-update."
echo ""
