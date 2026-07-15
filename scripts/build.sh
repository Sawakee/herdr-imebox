#!/bin/sh
# herdr runs this at plugin install/update time (cwd = plugin root).
# Builds the imebox binary from source; requires a Rust toolchain.
set -eu

if command -v rustup >/dev/null 2>&1 && CARGO=$(rustup which cargo 2>/dev/null); then
    : # prefer the real toolchain binary (robust against broken cargo shims)
elif command -v cargo >/dev/null 2>&1; then
    CARGO=cargo
elif [ -x "$HOME/.cargo/bin/cargo" ]; then
    CARGO="$HOME/.cargo/bin/cargo"
else
    echo "herdr-imebox: no Rust toolchain found; install one from https://rustup.rs" >&2
    exit 1
fi

exec "$CARGO" build --release
