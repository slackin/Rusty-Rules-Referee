#!/bin/bash
set -e

SERVER="root@10.10.0.2"
APP_USER="rusty"
APP_DIR="/home/rusty/big-brother-bot"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "=== Building UI ==="
cd "$SCRIPT_DIR/ui"
npm run build
cd "$SCRIPT_DIR"

echo ""
echo "=== Uploading to server ==="
scp -o StrictHostKeyChecking=no -r src "$SERVER:$APP_DIR/src"
scp -o StrictHostKeyChecking=no Cargo.toml "$SERVER:$APP_DIR/Cargo.toml"
ssh -o StrictHostKeyChecking=no $SERVER "rm -rf $APP_DIR/ui/build"
scp -o StrictHostKeyChecking=no -r ui/build "$SERVER:$APP_DIR/ui/build"
ssh -o StrictHostKeyChecking=no $SERVER "chown -R $APP_USER:$APP_USER $APP_DIR/src $APP_DIR/Cargo.toml $APP_DIR/ui"

echo ""
echo "=== Building on server ==="
ssh -o StrictHostKeyChecking=no $SERVER "su - $APP_USER -c 'cd $APP_DIR && cargo build --release 2>&1 | tail -5'"

echo ""
echo "=== Restarting R3 ==="
ssh -o StrictHostKeyChecking=no $SERVER "su - $APP_USER -c 'screen -S b3 -X quit 2>/dev/null; sleep 1; cd $APP_DIR && screen -dmS b3 ./target/release/rusty-rules-referee b3.toml' && sleep 2 && su - $APP_USER -c 'screen -ls'"

echo ""
echo "=== Pushing update to r3.pugbot.net ==="
UPDATE_SERVER="root@10.10.0.4"
UPDATE_BASE="/home/bcmx/domains/r3.pugbot.net/public_html/api/updates"
BINARY_PATH="$APP_DIR/target/release/rusty-rules-referee"
PLATFORM="linux-x86_64"

# Extract build info from the newly built binary on the game server
BUILD_HASH=$(ssh -o StrictHostKeyChecking=no $SERVER "su - $APP_USER -c '$BINARY_PATH --build-hash'" 2>/dev/null | tail -1)
SHA256=$(ssh -o StrictHostKeyChecking=no $SERVER "sha256sum $BINARY_PATH" | awk '{print $1}')
FILE_SIZE=$(ssh -o StrictHostKeyChecking=no $SERVER "stat -c%s $BINARY_PATH")
VERSION=$(echo "$BUILD_HASH" | cut -d'-' -f1)
GIT_COMMIT=$(echo "$BUILD_HASH" | cut -d'-' -f2)
RELEASED_AT=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

echo "  Build: $BUILD_HASH"
echo "  SHA256: $SHA256"
echo "  Size: $FILE_SIZE"

# Create directories on update server
ssh -o StrictHostKeyChecking=no $UPDATE_SERVER "mkdir -p ${UPDATE_BASE}/binaries"

# Push binary from game server to update server
ssh -o StrictHostKeyChecking=no $SERVER "scp -o StrictHostKeyChecking=no $BINARY_PATH ${UPDATE_SERVER}:${UPDATE_BASE}/binaries/r3-${PLATFORM}"

# Generate and upload latest.json
MANIFEST="{
  \"version\": \"${VERSION}\",
  \"build_hash\": \"${BUILD_HASH}\",
  \"git_commit\": \"${GIT_COMMIT}\",
  \"released_at\": \"${RELEASED_AT}\",
  \"platforms\": {
    \"${PLATFORM}\": {
      \"url\": \"https://r3.pugbot.net/api/updates/binaries/r3-${PLATFORM}\",
      \"sha256\": \"${SHA256}\",
      \"size\": ${FILE_SIZE}
    }
  }
}"
echo "$MANIFEST" | ssh -o StrictHostKeyChecking=no $UPDATE_SERVER "cat > ${UPDATE_BASE}/latest.json"

# Set permissions
ssh -o StrictHostKeyChecking=no $UPDATE_SERVER "chmod 644 ${UPDATE_BASE}/latest.json ${UPDATE_BASE}/binaries/r3-${PLATFORM}"

echo "  Manifest: https://r3.pugbot.net/api/updates/latest.json"
echo "  Binary:   https://r3.pugbot.net/api/updates/binaries/r3-${PLATFORM}"

echo ""
echo "=== Done! R3 is running and update published ==="
