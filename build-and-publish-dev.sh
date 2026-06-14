#!/bin/bash
# =============================================================================
# build-and-publish-dev.sh — Publish a branch build to the `dev` update channel.
#
# Invoked by the AI bug-fix runner (src/aibug) after it has built a fix in an
# isolated git worktree. It (re)builds the release binary if needed and
# publishes the binary + manifest to the dev channel, WITHOUT touching the main
# /opt/r3-build checkout or any other channel. Promotion to alpha/beta/prod
# remains a manual `promote.sh` step.
#
# Usage:
#   ./build-and-publish-dev.sh --dir /opt/r3-ai/job-12 --branch ai/bug-7
#
# This is intentionally a subset of deploy-remote.sh (dev channel, linux only).
# =============================================================================
set -euo pipefail

export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:$HOME/.cargo/bin:$PATH"

# ---- Config ----
PUBLISH_BASE="${PUBLISH_BASE:-/home/bcmx/domains/r3.pugbot.net/public_html/api/updates}"
PLATFORM="linux-x86_64"
BINARY_NAME="rusty-rules-referee"
BINARY_FILENAME="r3-${PLATFORM}"
CHANNEL="dev"

WORK_DIR=""
BRANCH=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dir)    WORK_DIR="$2"; shift 2 ;;
        --branch) BRANCH="$2"; shift 2 ;;
        --help|-h)
            echo "Usage: $0 --dir <worktree> --branch <branch>"
            exit 0
            ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

[ -n "$WORK_DIR" ] || { echo "--dir is required" >&2; exit 1; }
[ -d "$WORK_DIR" ] || { echo "Work dir not found: $WORK_DIR" >&2; exit 1; }

PUBLISH_DIR="${PUBLISH_BASE}/${CHANNEL}"
cd "$WORK_DIR"

# ---- Ensure UI is built (idempotent; runner usually already did this) ----
if [ ! -d "ui/build" ] || [ -z "$(ls -A ui/build 2>/dev/null)" ]; then
    echo "Building UI..."
    ( cd ui && (npm ci --loglevel=error || npm install --loglevel=error) && npm run build )
fi

# ---- Ensure release binary exists ----
BINARY="target/release/${BINARY_NAME}"
if [ ! -f "$BINARY" ]; then
    echo "Building release binary..."
    touch src/web/mod.rs
    cargo build --release
fi
[ -f "$BINARY" ] || { echo "Binary not found after build: $BINARY" >&2; exit 1; }

# ---- Extract build metadata ----
BUILD_HASH=$("./$BINARY" --build-hash 2>/dev/null | tail -1) || { echo "Failed to read build hash" >&2; exit 1; }
SHA256=$(sha256sum "$BINARY" | awk '{print $1}')
FILE_SIZE=$(stat -c%s "$BINARY")
VERSION=$(echo "$BUILD_HASH" | cut -d'-' -f1)
GIT_COMMIT=$(echo "$BUILD_HASH" | cut -d'-' -f2)
RELEASED_AT=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

echo "Build hash: ${BUILD_HASH} (branch ${BRANCH:-unknown})"

# ---- Publish binary + manifest to dev channel ----
mkdir -p "${PUBLISH_DIR}/binaries"
cp "$BINARY" "${PUBLISH_DIR}/binaries/${BINARY_FILENAME}"
chmod 644 "${PUBLISH_DIR}/binaries/${BINARY_FILENAME}"

DOWNLOAD_URL="https://r3.pugbot.net/api/updates/${CHANNEL}/binaries/${BINARY_FILENAME}"
cat > "${PUBLISH_DIR}/latest.json" <<EOF
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
chmod 644 "${PUBLISH_DIR}/latest.json"

echo "Published ${BUILD_HASH} to dev channel: ${DOWNLOAD_URL}"
echo "Promote with: ./promote.sh dev-to-alpha"
