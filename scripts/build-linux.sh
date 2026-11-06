#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
target="${1:?usage: build-linux.sh TARGET}"
case "$target" in
  x86_64-unknown-linux-musl|aarch64-unknown-linux-musl) ;;
  *) echo "Unsupported Linux release target: $target" >&2; exit 1 ;;
esac
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target}"
if [ "$target" = aarch64-unknown-linux-musl ]; then
  if [[ " ${CFLAGS_aarch64_unknown_linux_musl:-} " != *" -mno-outline-atomics "* ]]; then
    export CFLAGS_aarch64_unknown_linux_musl="${CFLAGS_aarch64_unknown_linux_musl:-} -mno-outline-atomics"
  fi
fi
rustup target add "$target"
cargo build --release --locked --target "$target"
python3 scripts/package.py --binary "$CARGO_TARGET_DIR/$target/release/nelko" --target "$target"
