#!/bin/bash
# =============================================================================
# push-update.sh — Push a new R3 build to the update server (r3.pugbot.net)
#
# This script:
#   1. Takes a compiled binary (or builds one)
#   2. Extracts the build hash from the binary
#   3. Computes SHA-256 of the binary
#   4. Generates latest.json manifest
#   5. Uploads both to r3.pugbot.net via scp
#
# Usage:
#   DEPLOY_PASS="yourpass" ./push-update.sh                    # uses default binary
#   DEPLOY_PASS="yourpass" ./push-update.sh --binary path/to/binary
#   DEPLOY_PASS="yourpass" ./push-update.sh --build            # build first, then push
#
# Requires: sshpass, sha256sum (or shasum on macOS)
# =============================================================================
set -euo pipefail

# ---- Config ----
REMOTE_USER="root"
REMOTE_HOST="10.10.0.4"
REMOTE_BASE="/home/bcmx/domains/r3.pugbot.net/public_html/api/updates"
PLATFORM="linux-x86_64"

# Release channels: production, beta, alpha, dev.
# This script ALWAYS publishes to the dev channel. Use promote.sh on the
# update server to move builds between channels (dev -> alpha -> beta -> production).
CHANNEL="dev"
REMOTE_DIR="${REMOTE_BASE}/${CHANNEL}"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BINARY=""
DO_BUILD=false

# ---- Colors ----
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

info()  { echo -e "  ${GREEN}✓${NC} $1"; }
warn()  { echo -e "  ${YELLOW}!${NC} $1"; }
err()   { echo -e "  ${RED}✗${NC} $1"; }

# ---- Parse args ----
while [[ $# -gt 0 ]]; do
    case "$1" in
        --binary)
            BINARY="$2"
            shift 2
            ;;
        --platform)
            PLATFORM="$2"
            shift 2
            ;;
        --build)
            DO_BUILD=true
            shift
            ;;
        --help|-h)
            echo "Usage: DEPLOY_PASS=\"pass\" $0 [--binary path] [--platform name] [--build]"
            echo ""
            echo "  --binary PATH     Path to compiled binary (default: target/release/rusty-rules-referee)"
            echo "  --platform NAME   Platform identifier (default: linux-x86_64)"
            echo "  --build           Build the binary first (runs build.sh)"
            exit 0
            ;;
        *)
            err "Unknown argument: $1"
            exit 1
            ;;
    esac
done

# ---- Validate ----
if [ -z "${DEPLOY_PASS:-}" ]; then
    err "DEPLOY_PASS environment variable is not set."
    echo "  Usage: DEPLOY_PASS=\"yourpass\" $0"
    exit 1
fi

if ! command -v sshpass &>/dev/null; then
    err "sshpass is required. Install with: apt install sshpass"
    exit 1
fi

# ---- Build if requested ----
if [ "$DO_BUILD" = true ]; then
    echo ""
    echo -e "${CYAN}${BOLD}=== Building R3 ===${NC}"
    "$SCRIPT_DIR/build.sh"
    echo ""
fi

# ---- Resolve binary path ----
if [ -z "$BINARY" ]; then
    BINARY="$SCRIPT_DIR/target/release/rusty-rules-referee"
fi

if [ ! -f "$BINARY" ]; then
    err "Binary not found: $BINARY"
    echo "  Build first with: ./build.sh"
    echo "  Or specify: $0 --binary /path/to/binary"
    exit 1
fi

echo ""
echo -e "${CYAN}${BOLD}=== Push R3 Update ===${NC}"
echo ""

# ---- Extract build hash from binary ----
info "Extracting build hash..."
BUILD_HASH=$("$BINARY" --build-hash 2>/dev/null) || {
    err "Failed to extract build hash from binary. Is it a valid R3 binary?"
    exit 1
}
info "Build hash: ${BOLD}${BUILD_HASH}${NC}"

# Parse version and git commit from build hash (format: version-githash-timestamp)
VERSION=$(echo "$BUILD_HASH" | cut -d'-' -f1)
GIT_COMMIT=$(echo "$BUILD_HASH" | cut -d'-' -f2)
info "Version: ${VERSION}, Git: ${GIT_COMMIT}"

# ---- Compute SHA-256 ----
info "Computing SHA-256..."
if command -v sha256sum &>/dev/null; then
    SHA256=$(sha256sum "$BINARY" | awk '{print $1}')
elif command -v shasum &>/dev/null; then
    SHA256=$(shasum -a 256 "$BINARY" | awk '{print $1}')
else
    err "Neither sha256sum nor shasum found."
    exit 1
fi
info "SHA-256: ${SHA256}"

# ---- Get file size ----
FILE_SIZE=$(stat -c%s "$BINARY" 2>/dev/null || stat -f%z "$BINARY" 2>/dev/null)
info "Size: ${FILE_SIZE} bytes ($(numfmt --to=iec "$FILE_SIZE" 2>/dev/null || echo "${FILE_SIZE}B"))"

# ---- Generate latest.json ----
BINARY_FILENAME="r3-${PLATFORM}"
DOWNLOAD_URL="https://r3.pugbot.net/api/updates/${CHANNEL}/binaries/${BINARY_FILENAME}"
RELEASED_AT=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

MANIFEST=$(cat <<EOF
{
  "channel": "${CHANNEL}",
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
)

info "Generated manifest:"
echo "$MANIFEST" | sed 's/^/    /'
echo ""

# ---- Upload to server ----
info "Creating remote directories (channel: ${CHANNEL})..."
sshpass -p "$DEPLOY_PASS" ssh -o StrictHostKeyChecking=no "${REMOTE_USER}@${REMOTE_HOST}" \
    "mkdir -p ${REMOTE_DIR}/binaries"

info "Uploading binary (${BINARY_FILENAME})..."
sshpass -p "$DEPLOY_PASS" scp -o StrictHostKeyChecking=no \
    "$BINARY" "${REMOTE_USER}@${REMOTE_HOST}:${REMOTE_DIR}/binaries/${BINARY_FILENAME}"

info "Uploading latest.json..."
echo "$MANIFEST" | sshpass -p "$DEPLOY_PASS" ssh -o StrictHostKeyChecking=no \
    "${REMOTE_USER}@${REMOTE_HOST}" "cat > ${REMOTE_DIR}/latest.json"

# ---- Set permissions ----
sshpass -p "$DEPLOY_PASS" ssh -o StrictHostKeyChecking=no "${REMOTE_USER}@${REMOTE_HOST}" \
    "chmod 644 ${REMOTE_DIR}/latest.json ${REMOTE_DIR}/binaries/${BINARY_FILENAME}"

echo ""
info "Update pushed successfully!"
echo ""
echo -e "  ${BOLD}Channel:${NC}  ${CHANNEL}"
echo -e "  ${BOLD}Build:${NC}    ${BUILD_HASH}"
echo -e "  ${BOLD}Manifest:${NC} https://r3.pugbot.net/api/updates/${CHANNEL}/latest.json"
echo -e "  ${BOLD}Binary:${NC}   ${DOWNLOAD_URL}"
echo ""
echo -e "  Bots on the ${BOLD}${CHANNEL}${NC} channel with ${BOLD}[update] enabled = true${NC} will pick this up automatically."
echo -e "  Use ${BOLD}promote.sh${NC} to move this build to alpha/beta/production."
echo ""
