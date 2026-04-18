#!/bin/bash
# Build script for R3 Admin — builds the SvelteKit frontend then the Rust binary
set -e

echo "=== Building R3 Admin UI ==="
cd "$(dirname "$0")/ui"

if [ ! -d "node_modules" ]; then
    echo "Installing npm dependencies..."
    npm install
fi

echo "Building SvelteKit app..."
npm run build

cd ..

echo ""
echo "=== Building Rust binary ==="
cargo build --release

echo ""
echo "Build complete!"
echo "Binary: target/release/rusty-rules-referee"
