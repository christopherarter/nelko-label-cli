#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
export MACOSX_DEPLOYMENT_TARGET=12.0
export CARGO_TARGET_DIR=target
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo build --release --locked --target aarch64-apple-darwin
cargo build --release --locked --target x86_64-apple-darwin
mkdir -p dist
lipo -create target/aarch64-apple-darwin/release/nelko target/x86_64-apple-darwin/release/nelko -output dist/nelko
codesign --force --sign - --identifier com.nelko-cli.cli dist/nelko
codesign --verify --strict dist/nelko
python3 scripts/package.py --binary dist/nelko --target universal-apple-darwin
