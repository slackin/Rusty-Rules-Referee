#!/bin/bash
# =============================================================================
# promote.sh — Promote an R3 build between release channels
#
# Release channels (in order of stability):
#   dev -> alpha -> beta -> production
#
# Automated builds publish only to the dev channel (see deploy-remote.sh and
# push-update.sh). Use this script to promote an approved build to the next
# channel. Each promotion is a physical copy of the binary + manifest; each
# channel's latest.json is self-contained with URLs rewritten to point at
# that channel's files.
#
# Usage (run on the update server, or over SSH):
#   ./promote.sh dev-to-alpha
#   ./promote.sh alpha-to-beta
#   ./promote.sh beta-to-production
#   ./promote.sh <direction> --force          # allow re-promoting same build_hash
#
# SSH invocation from a dev machine:
#   ssh root@10.10.0.4 'bash /opt/r3-build/promote.sh dev-to-alpha'
# =============================================================================
set -euo pipefail

# ---- Config ----
PUBLISH_BASE="${PUBLISH_BASE:-/home/bcmx/domains/r3.pugbot.net/public_html/api/updates}"
PUBLIC_BASE_URL="${PUBLIC_BASE_URL:-https://r3.pugbot.net/api/updates}"

# ---- Colors ----
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

info() { echo -e "  ${GREEN}✓${NC} $1"; }
warn() { echo -e "  ${YELLOW}!${NC} $1"; }
err()  { echo -e "  ${RED}✗${NC} $1"; }
die()  { err "$1"; exit 1; }

usage() {
    cat <<EOF
Usage: $0 <direction> [--force]

Directions:
  dev-to-alpha         Promote current dev build to alpha
  alpha-to-beta        Promote current alpha build to beta
  beta-to-production   Promote current beta build to production

Options:
  --force              Promote even if target channel already has the same build_hash

Environment:
  PUBLISH_BASE         Base dir of published channels (default: $PUBLISH_BASE)
  PUBLIC_BASE_URL      Public URL prefix for manifests (default: $PUBLIC_BASE_URL)
EOF
}

# ---- Parse args ----
if [ $# -lt 1 ]; then
    usage
    exit 1
fi

DIRECTION="$1"
shift || true
FORCE=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --force) FORCE=true; shift ;;
        --help|-h) usage; exit 0 ;;
        *) die "Unknown argument: $1" ;;
    esac
done

case "$DIRECTION" in
    dev-to-alpha)       SOURCE="dev";    TARGET="alpha" ;;
    alpha-to-beta)      SOURCE="alpha";  TARGET="beta" ;;
    beta-to-production) SOURCE="beta";   TARGET="production" ;;
    *) die "Invalid direction: $DIRECTION (expected dev-to-alpha | alpha-to-beta | beta-to-production)" ;;
esac

# ---- Validate tools ----
command -v jq >/dev/null 2>&1 || die "jq is required (apt install jq)"
command -v sha256sum >/dev/null 2>&1 || die "sha256sum is required"

SOURCE_DIR="${PUBLISH_BASE}/${SOURCE}"
TARGET_DIR="${PUBLISH_BASE}/${TARGET}"
SOURCE_MANIFEST="${SOURCE_DIR}/latest.json"
TARGET_MANIFEST="${TARGET_DIR}/latest.json"

echo ""
echo -e "${CYAN}${BOLD}=== Promote R3: ${SOURCE} -> ${TARGET} ===${NC}"
echo ""

# ---- Validate source ----
[ -f "$SOURCE_MANIFEST" ] || die "Source manifest not found: $SOURCE_MANIFEST"

SOURCE_JSON=$(cat "$SOURCE_MANIFEST")
echo "$SOURCE_JSON" | jq empty 2>/dev/null || die "Source manifest is not valid JSON: $SOURCE_MANIFEST"

SOURCE_BUILD_HASH=$(echo "$SOURCE_JSON" | jq -r '.build_hash')
SOURCE_VERSION=$(echo "$SOURCE_JSON" | jq -r '.version')
[ -n "$SOURCE_BUILD_HASH" ] && [ "$SOURCE_BUILD_HASH" != "null" ] || die "Source manifest missing build_hash"

info "Source: ${SOURCE} @ ${SOURCE_BUILD_HASH} (version ${SOURCE_VERSION})"

# Verify each platform binary exists in source channel and sha256 matches
PLATFORMS=$(echo "$SOURCE_JSON" | jq -r '.platforms | keys[]')
[ -n "$PLATFORMS" ] || die "Source manifest has no platforms entries"

for PLATFORM in $PLATFORMS; do
    EXPECTED_SHA=$(echo "$SOURCE_JSON" | jq -r --arg p "$PLATFORM" '.platforms[$p].sha256')
    BIN_FILE="${SOURCE_DIR}/binaries/r3-${PLATFORM}"
    [ -f "$BIN_FILE" ] || die "Source binary missing: $BIN_FILE"
    ACTUAL_SHA=$(sha256sum "$BIN_FILE" | awk '{print $1}')
    [ "$ACTUAL_SHA" = "$EXPECTED_SHA" ] || die "SHA-256 mismatch for $BIN_FILE (manifest: $EXPECTED_SHA, actual: $ACTUAL_SHA)"
    info "Verified ${PLATFORM}: SHA-256 OK"
done

# ---- Check target for no-op ----
if [ -f "$TARGET_MANIFEST" ]; then
    TARGET_BUILD_HASH=$(jq -r '.build_hash // ""' "$TARGET_MANIFEST" 2>/dev/null || echo "")
    if [ "$TARGET_BUILD_HASH" = "$SOURCE_BUILD_HASH" ]; then
        if [ "$FORCE" = true ]; then
            warn "Target ${TARGET} already has build ${SOURCE_BUILD_HASH} — continuing (--force)"
        else
            echo ""
            warn "Target ${TARGET} already has build ${SOURCE_BUILD_HASH} — nothing to do"
            warn "Use --force to re-promote anyway"
            exit 0
        fi
    elif [ -n "$TARGET_BUILD_HASH" ]; then
        info "Target currently: ${TARGET} @ ${TARGET_BUILD_HASH} (will be replaced)"
    fi
fi

# ---- Copy artifacts ----
mkdir -p "${TARGET_DIR}/binaries"

for PLATFORM in $PLATFORMS; do
    SRC_BIN="${SOURCE_DIR}/binaries/r3-${PLATFORM}"
    DST_BIN="${TARGET_DIR}/binaries/r3-${PLATFORM}"
    cp "$SRC_BIN" "$DST_BIN"
    chmod 644 "$DST_BIN"
    info "Copied binary: r3-${PLATFORM}"
done

# Rewrite manifest: set channel, rewrite each platform URL to target channel path
NEW_MANIFEST=$(echo "$SOURCE_JSON" | jq \
    --arg channel "$TARGET" \
    --arg base "$PUBLIC_BASE_URL" \
    '
    .channel = $channel
    | .platforms |= with_entries(
        .value.url = ($base + "/" + $channel + "/binaries/r3-" + .key)
      )
    ')

echo "$NEW_MANIFEST" | jq empty 2>/dev/null || die "Rewritten manifest is not valid JSON"
echo "$NEW_MANIFEST" > "$TARGET_MANIFEST"
chmod 644 "$TARGET_MANIFEST"
info "Wrote manifest: ${TARGET_MANIFEST}"

# ---- Verify target ----
for PLATFORM in $PLATFORMS; do
    EXPECTED_SHA=$(echo "$NEW_MANIFEST" | jq -r --arg p "$PLATFORM" '.platforms[$p].sha256')
    BIN_FILE="${TARGET_DIR}/binaries/r3-${PLATFORM}"
    ACTUAL_SHA=$(sha256sum "$BIN_FILE" | awk '{print $1}')
    [ "$ACTUAL_SHA" = "$EXPECTED_SHA" ] || die "Post-copy SHA-256 mismatch for $BIN_FILE"
done
info "Target binaries verified"

# ---- Summary ----
echo ""
echo -e "${GREEN}${BOLD}=== Promotion Successful ===${NC}"
echo ""
echo -e "  ${BOLD}From:${NC}     ${SOURCE}"
echo -e "  ${BOLD}To:${NC}       ${TARGET}"
echo -e "  ${BOLD}Version:${NC}  ${SOURCE_VERSION}"
echo -e "  ${BOLD}Build:${NC}    ${SOURCE_BUILD_HASH}"
echo -e "  ${BOLD}Manifest:${NC} ${PUBLIC_BASE_URL}/${TARGET}/latest.json"
echo ""
echo -e "  Bots on the ${BOLD}${TARGET}${NC} channel with ${BOLD}[update] enabled = true${NC} will pick this up automatically."
echo ""
