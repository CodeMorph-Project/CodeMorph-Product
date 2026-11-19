#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
BIN_DIR="${CODEMORPH_INSTALL_DIR:-$HOME/.local/bin}"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
if [[ "$TARGET_DIR" != /* ]]; then
  TARGET_DIR="$ROOT/$TARGET_DIR"
fi
BINARY="$TARGET_DIR/release/codemorph"
if [[ ! -x "$BINARY" ]]; then
  BINARY="$TARGET_DIR/debug/codemorph"
fi
if [[ ! -x "$BINARY" ]]; then
  echo "Build the project first: cargo build --locked --release --workspace" >&2
  exit 1
fi

RUNTIME="$BIN_DIR/../share/codemorph/python/codemorph_ml"
mkdir -p "$BIN_DIR" "$RUNTIME"
install -m 644 "$ROOT"/python/codemorph_ml/*.py "$RUNTIME/"
install -m 755 "$BINARY" "$BIN_DIR/codemorph"
"$BIN_DIR/codemorph" check
echo "Installed $BIN_DIR/codemorph. Add $BIN_DIR to PATH to run codemorph."
