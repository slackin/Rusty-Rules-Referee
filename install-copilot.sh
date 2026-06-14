#!/bin/bash
# =============================================================================
# install-copilot.sh — Provision the GitHub Copilot CLI on the build server.
#
# Run this ON the build server (10.10.0.4 / r3.pugbot.net) once, so the AI
# bug-fix runner (src/aibug) can drive Claude models via your Copilot
# subscription. After this, set `[aibug] enabled = true` in the master's
# referee config.
#
# Usage:
#   ssh root@10.10.0.4 'bash -s' < install-copilot.sh
#   # then authenticate interactively:
#   gh auth login        # or: copilot   (follow the device-code prompt)
# =============================================================================
set -euo pipefail

GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; CYAN='\033[0;36m'; NC='\033[0m'
ok()   { echo -e "  ${GREEN}✓${NC} $1"; }
warn() { echo -e "  ${YELLOW}!${NC} $1"; }
step() { echo -e "\n${CYAN}=== $1 ===${NC}"; }

# ---- Node.js (Copilot CLI is distributed via npm) ----
step "Checking Node.js"
if ! command -v node >/dev/null 2>&1; then
    warn "Node.js not found — installing Node 20.x"
    curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
    apt-get install -y nodejs
fi
ok "node $(node --version), npm $(npm --version)"

# ---- GitHub Copilot CLI ----
step "Installing GitHub Copilot CLI"
if command -v copilot >/dev/null 2>&1; then
    ok "copilot already installed: $(copilot --version 2>/dev/null | head -1 || echo present)"
else
    npm install -g @github/copilot
    ok "copilot installed: $(copilot --version 2>/dev/null | head -1 || echo present)"
fi

# ---- Dedicated work dir for AI jobs ----
step "Preparing AI work directory"
WORK_DIR="${R3_AI_WORK_DIR:-/opt/r3-ai}"
mkdir -p "$WORK_DIR"
ok "Work dir ready: $WORK_DIR"

# ---- Auth reminder ----
step "Authentication"
echo ""
echo -e "  ${YELLOW}ACTION REQUIRED:${NC} authenticate the Copilot CLI with your account."
echo "    Run interactively on this server:"
echo ""
echo "      copilot         # then follow the device-code login, or"
echo "      gh auth login   # if using the gh-backed flow"
echo ""
echo "  Verify model access:"
echo "      copilot --list-models"
echo ""
echo -e "  Then enable the runner in the master config:"
echo "      [aibug]"
echo "      enabled = true"
echo ""
ok "Copilot CLI provisioning complete"
