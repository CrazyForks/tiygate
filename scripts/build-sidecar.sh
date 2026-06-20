#!/bin/bash
# Build the tiygate-server binary as a Tauri sidecar and place it in
# src-tauri/binaries/ with the platform-specific target-triple suffix
# that Tauri's externalBin mechanism expects.
#
# Usage:
#   ./scripts/build-sidecar.sh            # release build for host target
#   ./scripts/build-sidecar.sh --debug    # debug build
set -euo pipefail

cd "$(dirname "$0")/.."

# Features for the sidecar binary. Exclude redis-quota (not needed for
# local client) and bedrock (heavy AWS SDK). Exclude dotenv — the Tauri
# host injects all env vars explicitly, and dotenvy would search the
# CWD for a .env file, which can trigger macOS TCC prompts when the
# workspace lives under ~/Documents.
SIDECAR_FEATURES="webui,admin,cache,providers,control-plane,tracing"

# Determine the build profile.
PROFILE="release"
CARGO_FLAGS="--release"
if [[ "${1:-}" == "--debug" ]]; then
    PROFILE="debug"
    CARGO_FLAGS=""
fi

# Get the host target triple.
TARGET=$(rustc -vV | grep '^host:' | sed 's/^host: //')
if [[ -z "$TARGET" ]]; then
    echo "ERROR: could not determine host target triple"
    exit 1
fi

echo ">> Building tiygate-server sidecar (target: $TARGET, profile: $PROFILE)"

# Build the webui first so rust-embed can embed dist/.
# Set TAURI_ENV=1 so vite uses base="/" (Tauri webview root) instead
# of "/admin/ui/" (browser path).
echo ">> Building webui…"
cd webui && TAURI_ENV=1 npm run build && cd ..

# Build the server binary with the sidecar feature set.
echo ">> Building tiygate-server with features: $SIDECAR_FEATURES"
cargo build -p tiygate-server --features "$SIDECAR_FEATURES" $CARGO_FLAGS

# Determine the binary name with the target-triple suffix.
# On Windows the binary has a .exe extension.
BIN_NAME="tiygate"
if [[ "$TARGET" == *"windows"* ]]; then
    BIN_NAME="tiygate.exe"
fi

# Locate the built binary.
if [[ "$PROFILE" == "release" ]]; then
    SRC_BIN="target/release/$BIN_NAME"
else
    SRC_BIN="target/debug/$BIN_NAME"
fi

if [[ ! -f "$SRC_BIN" ]]; then
    echo "ERROR: built binary not found at $SRC_BIN"
    exit 1
fi

# Copy to src-tauri/binaries/ with the target-triple suffix.
DEST_DIR="src-tauri/binaries"
mkdir -p "$DEST_DIR"
DEST_BIN="$DEST_DIR/tiygate-$TARGET"
if [[ "$TARGET" == *"windows"* ]]; then
    DEST_BIN="${DEST_BIN}.exe"
fi

echo ">> Copying $SRC_BIN → $DEST_BIN"
cp "$SRC_BIN" "$DEST_BIN"
chmod +x "$DEST_BIN" 2>/dev/null || true

echo ">> Sidecar binary ready: $DEST_BIN"
