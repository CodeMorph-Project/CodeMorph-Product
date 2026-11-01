#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
BIN_DIR="${CODEMORPH_INSTALL_DIR:-$HOME/.local/bin}"
mkdir -p "$BIN_DIR"

echo "Installing CodeMorph into: $BIN_DIR"
install -m 755 "$ROOT/target/release/codemorph" "$BIN_DIR/codemorph" 2>/dev/null || \
install -m 755 "$ROOT/target/debug/codemorph" "$BIN_DIR/codemorph" 2>/dev/null || \
{ echo "Build the project first: cargo build --workspace"; exit 1; }

echo "Checking toolchain..."
"$BIN_DIR/codemorph" check

if [ -d "$ROOT/models" ]; then
  echo "Model registry found in: $ROOT/models"
  "$BIN_DIR/codemorph" models validate || true
fi

echo "Done. Ensure $BIN_DIR is in PATH."
