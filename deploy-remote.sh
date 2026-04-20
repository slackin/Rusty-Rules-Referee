#!/bin/bash
# =============================================================================
# deploy-remote.sh — Build & publish R3 on the update server (r3.pugbot.net)
#
# This script runs ON the update server (10.10.0.4). It:
#   1. Pulls the latest code from GitHub
#   2. Builds the SvelteKit UI (clean)
#   3. Builds the Rust binary (with embedded UI)
#   4. Publishes the binary + manifest for auto-update
#
# Usage (on the server directly):
#   /opt/r3-build/deploy-remote.sh
#   /opt/r3-build/deploy-remote.sh --skip-pull   # skip git pull (already up to date)
#
# Called automatically by deploy.sh / deploy.ps1 from the dev machine.
# =============================================================================
set -euo pipefail

# ---- Ensure full PATH (some SSH sessions have minimal PATH) ----
export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:$HOME/.cargo/bin:$PATH"

# ---- Config ----
BUILD_DIR="/opt/r3-build"
PUBLISH_BASE="/home/bcmx/domains/r3.pugbot.net/public_html/api/updates"
PLATFORM="linux-x86_64"
BINARY_NAME="rusty-rules-referee"
BINARY_FILENAME="r3-${PLATFORM}"

SKIP_PULL=false

# ---- Colors ----
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

step()  { echo -e "\n${CYAN}${BOLD}[$1/$TOTAL_STEPS] $2${NC}"; }
ok()    { echo -e "  ${GREEN}✓${NC} $1"; }
warn()  { echo -e "  ${YELLOW}!${NC} $1"; }
fail()  { echo -e "  ${RED}✗ $1${NC}"; }

die() {
    fail "$1"
    echo -e "\n${RED}${BOLD}Deploy failed at: $1${NC}\n"
    exit 1
}

# ---- Parse args ----
while [[ $# -gt 0 ]]; do
    case "$1" in
        --skip-pull) SKIP_PULL=true; shift ;;
        --help|-h)
            echo "Usage: $0 [--skip-pull]"
            echo "  --skip-pull   Skip git pull (code already up to date)"
            exit 0
            ;;
        *) die "Unknown argument: $1" ;;
    esac
done

# ---- Determine step count ----
if [ "$SKIP_PULL" = true ]; then
    TOTAL_STEPS=6
else
    TOTAL_STEPS=7
fi

CURRENT_STEP=0

echo -e "${CYAN}${BOLD}"
echo "  ╔══════════════════════════════════════╗"
echo "  ║       R3 Build & Publish             ║"
echo "  ║       r3.pugbot.net                  ║"
echo "  ╚══════════════════════════════════════╝"
echo -e "${NC}"

# ---- Validate environment ----
if [ ! -d "$BUILD_DIR" ]; then
    die "Build directory not found: $BUILD_DIR — run setup-build-server.sh first"
fi

if [ ! -d "$BUILD_DIR/.git" ]; then
    die "Not a git repository: $BUILD_DIR"
fi

cd "$BUILD_DIR"

# ---- Step: Git Pull ----
if [ "$SKIP_PULL" = false ]; then
    CURRENT_STEP=$((CURRENT_STEP + 1))
    step $CURRENT_STEP "Pulling latest from GitHub"

    # Fail if there are local modifications (there shouldn't be — server is build-only)
    if [ -n "$(git status --porcelain)" ]; then
        warn "Working tree is dirty — resetting to match remote"
        git reset --hard HEAD
        git clean -fd
    fi

    git fetch origin || die "git fetch failed — check SSH key / network"
    BRANCH=$(git rev-parse --abbrev-ref HEAD)
    git merge --ff-only "origin/$BRANCH" || die "git merge failed — branch has diverged, manual intervention needed"
    ok "Updated to $(git rev-parse --short=8 HEAD) on branch $BRANCH"
fi

# ---- Step: Build UI ----
CURRENT_STEP=$((CURRENT_STEP + 1))
step $CURRENT_STEP "Building SvelteKit UI"

# Clean stale chunks to prevent rust-embed from serving old files
rm -rf ui/build
ok "Cleaned old ui/build"

cd ui

# Use npm ci for deterministic installs; fall back to npm install if no lockfile
if [ -f "package-lock.json" ]; then
    npm ci --loglevel=error 2>&1 || die "npm ci failed"
else
    npm install --loglevel=error 2>&1 || die "npm install failed"
fi
ok "Dependencies installed"

npm run build 2>&1 || die "npm run build failed"
ok "UI built successfully"

cd "$BUILD_DIR"

# Verify UI output exists
if [ ! -d "ui/build" ] || [ -z "$(ls -A ui/build 2>/dev/null)" ]; then
    die "ui/build is empty after npm run build — check SvelteKit config"
fi
ok "ui/build contains $(find ui/build -type f | wc -l) files"

# ---- Step: Build Rust Binary ----
CURRENT_STEP=$((CURRENT_STEP + 1))
step $CURRENT_STEP "Building Rust binary"

cargo build --release 2>&1 | tail -20 || die "cargo build --release failed"

BINARY="target/release/${BINARY_NAME}"
if [ ! -f "$BINARY" ]; then
    die "Binary not found after build: $BINARY"
fi
ok "Binary built: $(du -h "$BINARY" | awk '{print $1}')"

# ---- Step: Extract Build Info ----
CURRENT_STEP=$((CURRENT_STEP + 1))
step $CURRENT_STEP "Extracting build metadata"

BUILD_HASH=$("./$BINARY" --build-hash 2>/dev/null | tail -1) || die "Failed to extract build hash from binary"
SHA256=$(sha256sum "$BINARY" | awk '{print $1}')
FILE_SIZE=$(stat -c%s "$BINARY")
VERSION=$(echo "$BUILD_HASH" | cut -d'-' -f1)
GIT_COMMIT=$(echo "$BUILD_HASH" | cut -d'-' -f2)
RELEASED_AT=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

ok "Build hash: ${BUILD_HASH}"
ok "SHA-256:    ${SHA256}"
ok "Size:       ${FILE_SIZE} bytes"

# ---- Step: Publish Binary ----
CURRENT_STEP=$((CURRENT_STEP + 1))
step $CURRENT_STEP "Publishing binary & manifest"

mkdir -p "${PUBLISH_BASE}/binaries"

cp "$BINARY" "${PUBLISH_BASE}/binaries/${BINARY_FILENAME}" || die "Failed to copy binary to publish path"
ok "Binary copied to ${PUBLISH_BASE}/binaries/${BINARY_FILENAME}"

# Generate latest.json
DOWNLOAD_URL="https://r3.pugbot.net/api/updates/binaries/${BINARY_FILENAME}"
cat > "${PUBLISH_BASE}/latest.json" <<EOF
{
  "version": "${VERSION}",
  "build_hash": "${BUILD_HASH}",
  "git_commit": "${GIT_COMMIT}",
  "released_at": "${RELEASED_AT}",
  "platforms": {
    "${PLATFORM}": {
      "url": "${DOWNLOAD_URL}",
      "sha256": "${SHA256}",
      "size": ${FILE_SIZE}
    }
  }
}
EOF
ok "Manifest written to ${PUBLISH_BASE}/latest.json"

# Set permissions (readable by nginx)
chmod 644 "${PUBLISH_BASE}/latest.json" "${PUBLISH_BASE}/binaries/${BINARY_FILENAME}"
ok "Permissions set (644)"

# ---- Step: Verify ----
CURRENT_STEP=$((CURRENT_STEP + 1))
step $CURRENT_STEP "Verifying deployment"

# Verify the published binary matches what we built
PUB_SHA256=$(sha256sum "${PUBLISH_BASE}/binaries/${BINARY_FILENAME}" | awk '{print $1}')
if [ "$PUB_SHA256" != "$SHA256" ]; then
    die "SHA-256 mismatch! Published binary doesn't match build"
fi
ok "Published binary SHA-256 verified"

# Verify manifest is valid JSON
if ! python3 -c "import json; json.load(open('${PUBLISH_BASE}/latest.json'))" 2>/dev/null; then
    die "latest.json is not valid JSON"
fi
ok "Manifest JSON validated"

# ---- Step: Build & Publish Installer ----
CURRENT_STEP=$((CURRENT_STEP + 1))
step $CURRENT_STEP "Building & publishing installer"

INSTALLER_FILENAME="install-r3.sh"
bash "${BUILD_DIR}/build-installer.sh" "$BINARY" || die "build-installer.sh failed"

if [ ! -f "${BUILD_DIR}/install-r3.sh" ]; then
    die "Installer not found after build"
fi

cp "${BUILD_DIR}/install-r3.sh" "${PUBLISH_BASE}/${INSTALLER_FILENAME}" || die "Failed to copy installer to publish path"
chmod 644 "${PUBLISH_BASE}/${INSTALLER_FILENAME}"
INSTALLER_SIZE=$(du -h "${PUBLISH_BASE}/${INSTALLER_FILENAME}" | awk '{print $1}')
ok "Installer published: ${PUBLISH_BASE}/${INSTALLER_FILENAME} (${INSTALLER_SIZE})"

# Publish uninstaller separately too
if [ -f "${BUILD_DIR}/uninstall-r3.sh" ]; then
    cp "${BUILD_DIR}/uninstall-r3.sh" "${PUBLISH_BASE}/uninstall-r3.sh"
    chmod 644 "${PUBLISH_BASE}/uninstall-r3.sh"
    ok "Uninstaller published: ${PUBLISH_BASE}/uninstall-r3.sh"
fi

# ---- Summary ----
echo ""
echo -e "${GREEN}${BOLD}"
echo "  ╔══════════════════════════════════════╗"
echo "  ║       Deploy Successful!             ║"
echo "  ╚══════════════════════════════════════╝"
echo -e "${NC}"
echo -e "  ${BOLD}Version:${NC}  ${VERSION}"
echo -e "  ${BOLD}Build:${NC}    ${BUILD_HASH}"
echo -e "  ${BOLD}SHA-256:${NC}  ${SHA256}"
echo -e "  ${BOLD}Manifest:${NC} https://r3.pugbot.net/api/updates/latest.json"
echo -e "  ${BOLD}Binary:${NC}   ${DOWNLOAD_URL}"
echo -e "  ${BOLD}Installer:${NC}https://r3.pugbot.net/api/updates/install-r3.sh"
echo ""
echo -e "  Game servers with ${BOLD}[update] enabled = true${NC} will auto-update."
echo ""
