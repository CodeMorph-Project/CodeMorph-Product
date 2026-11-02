#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
BIN_DIR="${CODEMORPH_INSTALL_DIR:-$HOME/.local/bin}"
PYTHON_DIR="${CODEMORPH_PYTHON:-/opt/homebrew/bin/python3.12}"

echo "CodeMorph bootstrap"
mkdir -p "$BIN_DIR"

if [ -x "$HOME/.rustup/toolchains/nightly-aarch64-apple-darwin/bin/cargo" ]; then
  export PATH="$HOME/.rustup/toolchains/nightly-aarch64-apple-darwin/bin:$PATH"
fi
if [ -x "$HOME/.cargo/bin/cargo" ]; then
  export PATH="$HOME/.cargo/bin:$PATH"
fi

if command -v cargo >/dev/null 2>&1 && command -v rustc >/dev/null 2>&1; then
  echo "Cargo available; installing with cargo install --path crates/codemorph-cli"
  cargo install --path "$ROOT/crates/codemorph-cli" --locked
else
  if [ -x "$ROOT/target/release/codemorph" ]; then
    install -m 755 "$ROOT/target/release/codemorph" "$BIN_DIR/codemorph"
  elif [ -x "$ROOT/target/debug/codemorph" ]; then
    install -m 755 "$ROOT/target/debug/codemorph" "$BIN_DIR/codemorph"
  else
    echo "No binary found. Build first: cd \"$ROOT\" && cargo build --workspace"
    exit 1
  fi
fi

PROFILE="${CODEMORPH_SHELL_PROFILE:-$HOME/.zshrc}"
if ! grep -q "# codemorph shell integration" "$PROFILE" 2>/dev/null; then
  {
    echo ""
    echo "# codemorph shell integration"
    echo "export PATH=\"$BIN_DIR:\\$PATH\""
    echo "export CODEMORPH_PYTHON=\"$PYTHON_DIR\""
  } >> "$PROFILE"
fi

export PATH="$BIN_DIR:$HOME/.cargo/bin:$PATH"
export CODEMORPH_PYTHON="$PYTHON_DIR"

if ! command -v codemorph >/dev/null 2>&1; then
  echo "codemorph binary is not on PATH; check ~/.cargo/bin"
  exit 1
fi

echo "Verifying:"
codemorph check
codemorph models validate || true

echo ""
echo "Done. Open a new terminal or run: source $PROFILE"
echo "Usage: codemorph migrate ./cpp-project --output ./rust-project --full --cases 5"
