#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
# Prefer ARM Rustup over broken /usr/local i386 tools (if any).
if [ -x "$HOME/.rustup/toolchains/nightly-aarch64-apple-darwin/bin/cargo" ]; then
  export PATH="$HOME/.rustup/toolchains/nightly-aarch64-apple-darwin/bin:$PATH"
fi
if [ -x "$HOME/.cargo/bin/cargo" ]; then
  export PATH="$HOME/.cargo/bin:$PATH"
fi
BIN_DIR="${CODEMORPH_INSTALL_DIR:-$HOME/.local/bin}"
PYTHON_DIR="${CODEMORPH_PYTHON:-/opt/homebrew/bin/python3.12}"

echo "CodeMorph bootstrap"
mkdir -p "$BIN_DIR"

if command -v cargo >/dev/null 2>&1 && command -v rustc >/dev/null 2>&1; then
  :
else
  if [ -x "$HOME/.rustup/toolchains/nightly-aarch64-apple-darwin/bin/cargo" ]; then
    export PATH="$HOME/.rustup/toolchains/nightly-aarch64-apple-darwin/bin:$PATH"
  fi
  if [ -x "$HOME/.cargo/bin/cargo" ]; then
    export PATH="$HOME/.cargo/bin:$PATH"
  fi
fi

if command -v cargo >/dev/null 2>&1 && command -v rustc >/dev/null 2>&1; then
  echo "Cargo available; installing with cargo install --path crates/codemorph-cli"
  cargo install --manifest-path "$ROOT/Cargo.toml" --path "$ROOT/crates/codemorph-cli" --locked || cargo install --path "$ROOT/crates/codemorph-cli" --locked || {
    echo "Cargo install failed"
    exit 1
  }
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

PROFILE="${CODEMORPH_SHELL_PROFILE:-$HOME/.zshrc}"
if command -v codemorph >/dev/null 2>&1; then
  CODEMORPH_BIN="$(command -v codemorph)"
else
  CODEMORPH_BIN="$BIN_DIR/codemorph"
fi

if ! grep -q "# codemorph shell integration" "$PROFILE" 2>/dev/null; then
  {
    echo ""
    echo "# codemorph shell integration"
    echo "export PATH=\"$BIN_DIR:\$PATH\""
    echo "export CODEMORPH_PYTHON=\"$PYTHON_DIR\""
  } >> "$PROFILE"
fi

export PATH="$BIN_DIR:$HOME/.cargo/bin:$PATH"
export CODEMORPH_PYTHON="$PYTHON_DIR"

if [ -n "${CODEMORPH_MODELS_ROOT:-}" ]; then
  echo "Models root set from env: $CODEMORPH_MODELS_ROOT"
else
  echo "Auto resolver will look under ~/.codemorph/models, repo, cwd"
fi

echo "Verifying:"
if ! command -v codemorph >/dev/null 2>&1; then
  export PATH="$HOME/.cargo/bin:$PATH"
fi
if command -v codemorph >/dev/null 2>&1; then
  bin="$(command -v codemorph)"
  install -m 755 "$bin" "$BIN_DIR/codemorph"
  codemorph check
  codemorph models validate || true
else
  echo "Cargo install did not expose codemorph binary"
  exit 1
fi

echo ""
echo "Done. Open a new terminal or run: source $PROFILE"
echo "Usage: codemorph migrate ./cpp-project --output ./rust-project --full --cases 5"
