#!/usr/bin/env bash
# Start the simple-chess app on localhost:8080
# Run from the project root: ./startup.sh

set -e

cd "$(dirname "$0")"

echo "Building WASM bundle..."
PATH="$PATH:$HOME/.cargo/bin" trunk build --release

echo "Starting server..."
python3 serve.py
