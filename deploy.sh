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
echo "=== Done! R3 is running ==="
