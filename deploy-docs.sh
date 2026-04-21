#!/usr/bin/env bash
set -euo pipefail

# Deploy R3 docs to r3.pugbot.net
# Requires: sshpass, npm
# Usage: DEPLOY_PASS="yourpass" ./deploy-docs.sh

REMOTE_USER="root"
REMOTE_HOST="10.10.0.4"
REMOTE_DIR="/home/bcmx/domains/r3.pugbot.net/public_html"

if [ -z "${DEPLOY_PASS:-}" ]; then
  echo "Error: DEPLOY_PASS environment variable is not set."
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DOCS_DIR="$SCRIPT_DIR/docs"
DIST_DIR="$DOCS_DIR/.vitepress/dist"

echo "==> Installing dependencies..."
cd "$DOCS_DIR"
npm install

echo "==> Building docs..."
npm run docs:build

if [ ! -d "$DIST_DIR" ]; then
  echo "Error: Build output not found at $DIST_DIR"
  exit 1
fi

echo "==> Deploying to $REMOTE_HOST..."
# Preserve /media/ (demo videos, posters, etc.) so docs redeploys don't wipe
# assets published via `video/npm run publish`.
sshpass -p "$DEPLOY_PASS" rsync -avz --delete \
  --exclude='media/' --exclude='media' \
  -e "ssh -o StrictHostKeyChecking=no" \
  "$DIST_DIR/" "$REMOTE_USER@$REMOTE_HOST:$REMOTE_DIR/"

echo "==> Setting ownership..."
sshpass -p "$DEPLOY_PASS" ssh -o StrictHostKeyChecking=no \
  "$REMOTE_USER@$REMOTE_HOST" \
  "chown -R bcmx:bcmx $REMOTE_DIR"

echo "==> Done! Site deployed to r3.pugbot.net"
