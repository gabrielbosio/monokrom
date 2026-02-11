#!/bin/sh
set -e

cargo build --release --target wasm32-unknown-unknown

mkdir -p web/dist
cp target/wasm32-unknown-unknown/release/monokrom.wasm web/dist/
cp web/index.html web/dist/
cp web/gl.js web/dist/
cp web/sapp_jsutils.js web/dist/
cp web/quad-storage.js web/dist/
cp web/monokrom.js web/dist/

echo "Build complete. Serve web/dist/ to test."
echo "  python3 -m http.server -d web/dist 8080"
