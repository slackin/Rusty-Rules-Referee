#!/bin/bash
# =============================================================================
# build-installer.sh — Build a self-extracting installer for Rusty Rules Referee
#
# Run this on the Linux build server after compiling the release binary.
# It packages the binary, embedded UI, migrations, and config template into
# a single self-extracting install-r3.sh script.
#
# Usage: ./build-installer.sh [binary_path]
#   binary_path defaults to ./target/release/rusty-rules-referee
# =============================================================================
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BINARY="${1:-$SCRIPT_DIR/target/release/rusty-rules-referee}"
OUTPUT="$SCRIPT_DIR/install-r3.sh"

# Verify binary exists
if [ ! -f "$BINARY" ]; then
    echo "ERROR: Binary not found at $BINARY"
    echo "Build first with: cargo build --release"
    exit 1
fi

echo "=== Building R3 Installer ==="
echo "Binary: $BINARY ($(du -h "$BINARY" | cut -f1))"

# Create temp staging directory
STAGE=$(mktemp -d)
trap "rm -rf $STAGE" EXIT

# Stage the binary
mkdir -p "$STAGE/r3"
cp "$BINARY" "$STAGE/r3/rusty-rules-referee"
chmod +x "$STAGE/r3/rusty-rules-referee"

# Stage the example config as reference
if [ -f "$SCRIPT_DIR/referee.example.toml" ]; then
    cp "$SCRIPT_DIR/referee.example.toml" "$STAGE/r3/referee.example.toml"
fi

# Create the tarball
echo "Creating archive..."
(cd "$STAGE" && tar czf "$STAGE/payload.tar.gz" r3/)

echo "Payload size: $(du -h "$STAGE/payload.tar.gz" | cut -f1)"

# Write the self-extracting installer header
# Copy everything from install-r3.sh up to (and including) __ARCHIVE_MARKER__
HEADER_FILE="$SCRIPT_DIR/install-r3.sh"
if [ -f "$HEADER_FILE" ]; then
    # Extract the header portion (everything up to and including __ARCHIVE_MARKER__)
    MARKER_LINE=$(grep -n '^__ARCHIVE_MARKER__$' "$HEADER_FILE" | head -1 | cut -d: -f1)
    if [ -z "$MARKER_LINE" ]; then
        echo "ERROR: No __ARCHIVE_MARKER__ found in install-r3.sh"
        exit 1
    fi
    head -n "$MARKER_LINE" "$HEADER_FILE" > "$OUTPUT"
else
    echo "ERROR: install-r3.sh not found at $HEADER_FILE"
    exit 1
fi

# Append the binary payload to the installer
cat "$STAGE/payload.tar.gz" >> "$OUTPUT"
chmod +x "$OUTPUT"

FINAL_SIZE=$(du -h "$OUTPUT" | cut -f1)
echo ""
echo "=== Installer built ==="
echo "  Output: $OUTPUT ($FINAL_SIZE)"
echo ""
echo "  Distribute this single file to server admins."
echo "  Install with:  sudo bash install-r3.sh"
echo ""
