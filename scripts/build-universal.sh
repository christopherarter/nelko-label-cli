#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
export MACOSX_DEPLOYMENT_TARGET=12.0
export CARGO_TARGET_DIR=target
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo build --release --locked --target aarch64-apple-darwin
cargo build --release --locked --target x86_64-apple-darwin
mkdir -p dist
lipo -create target/aarch64-apple-darwin/release/open-nelko target/x86_64-apple-darwin/release/open-nelko -output dist/open-nelko
codesign --force --sign - --identifier io.github.christopherarter.open-nelko dist/open-nelko
codesign --verify --strict dist/open-nelko
python3 scripts/package.py --binary dist/open-nelko --target universal-apple-darwin
