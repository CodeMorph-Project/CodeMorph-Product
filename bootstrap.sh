#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"
cargo build --locked --release --workspace
exec "$ROOT/install.sh"
