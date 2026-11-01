#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
BIN_DIR="${CODEMORPH_BIN_DIR:-$HOME/.local/bin}"
PYTHON_DIR="${CODEMORPH_PYTHON:-/opt/homebrew/bin/python3.12}"

echo "CodeMorph bootstrap"
mkdir -p "$BIN_DIR"

if command -v cargo >/dev/null 2>&1 && command -v rustc >/dev/null 2>&1; then
  echo "Cargo available; installing with cargo install --path crates/codemorph-cli"
  cargo install --manifest-path "$ROOT/Cargo.toml" --path "$ROOT/crates/codemorph-cli" --locked
else
  echo "Cargo missing; falling back to an existing binary"
  if [ -x "$ROOT/target/release/codemorph" ]; then
    install -m 755 "$ROOT/target/release/codemorph" "$BIN_DIR/codemorph"
  elif [ -x "$ROOT/target/debug/codemorph" ]; then
    install -m 755 "$ROOT/target/debug/codemorph" "$BIN_DIR/codemorph"
  else
    echo "No binary found. Build first: cd \"$ROOT\" && cargo build --workspace"
    exit 1
  fi
fi

# Keep user .env under current source tree, but avoid exporting if profile absent.
if ! echo "$PATH" | tr ':' '\n' | grep -qx "$BIN_DIR"; then
  echo ""
  echo "Add this to your shell profile once:"
  echo "  export PATH=\"$BIN_DIR:\$PATH\""
  echo ""
fi

export PATH="$BIN_DIR:$PATH"
export CODEMORPH_PYTHON="$PYTHON_DIR"

echo "Verifying:"
"$BIN_DIR/codemorph" check
"$BIN_DIR/codemorph" models validate || true

echo ""
echo "Usage:"
echo "  codemorph migrate ./cpp-project --output ./rust-project --full --cases 5"
