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
RUN_USER="${R3_RUN_USER:-root}"   # the user the R3 master runs as (root or r3)
mkdir -p "$WORK_DIR"
chown "$RUN_USER":"$RUN_USER" "$WORK_DIR" 2>/dev/null || true
ok "Work dir ready: $WORK_DIR (owner: $RUN_USER)"

# ---- Auth ----
# The Copilot CLI authenticates as the user that runs it. The R3 master spawns
# the agent in-process, so it must be authenticated for that user ($RUN_USER).
# Two supported methods:
step "Authentication"
cat <<EOF

  The agent runs as the master's service user: ${RUN_USER}.
  Authenticate Copilot for THAT user, using ONE of:

  A) Headless token (recommended for automation):
     Create a GitHub fine-grained PAT with the "Copilot Requests" permission
     (and "Contents: read & write" if the same token will push fixes), then
     expose it to the master service as an environment variable:
       COPILOT_GITHUB_TOKEN=<token>     (or GH_TOKEN / GITHUB_TOKEN)
     e.g. add to the systemd unit:  systemctl edit r3.service
       [Service]
       Environment=COPILOT_GITHUB_TOKEN=<token>

  B) Interactive device login (stores creds in ~${RUN_USER}/.copilot):
       copilot login        # run as ${RUN_USER}

  Verify it works (as the run user):
       copilot -p "say ok" --allow-all-tools

  NOTE: the Copilot CLI has NO model-list command. Models are chosen with
  --model <id>; the selectable set is the curated [aibug] fallback_models in
  the master config. Discover available ids interactively via the /model
  command inside \`copilot\`.

  Then enable the runner in the master config and restart:
       [aibug]
       enabled = true
       sudo systemctl restart r3.service
EOF
ok "Copilot CLI provisioning complete"
