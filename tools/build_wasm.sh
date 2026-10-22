#!/usr/bin/env bash
# Builds the WebAssembly package: crates/ord-wasm for wasm32, then wasm-bindgen.
#   bash tools/build_wasm.sh [target] [out]     target: nodejs (default) or web; out: dist/wasm-<target>
# The wasm-bindgen CLI must be the version that crates/ord-wasm/Cargo.toml pins.
set -euo pipefail
cd "$(dirname "$0")/.."
target="${1:-nodejs}"
out="${2:-dist/wasm-$target}"
want=$(sed -n 's/^wasm-bindgen = "=\([0-9.]*\)".*/\1/p' crates/ord-wasm/Cargo.toml)
have=$(wasm-bindgen --version | awk '{print $2}')
[ "$want" = "$have" ] || { echo "build_wasm: wasm-bindgen $have, but crates/ord-wasm pins $want" >&2; exit 1; }
cargo build --locked --release --target wasm32-unknown-unknown -p ord-wasm
dir="$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
wasm-bindgen --target "$target" --out-dir "$out" --out-name ord "$dir/wasm32-unknown-unknown/release/ord_wasm.wasm"
ls -l "$out/ord_bg.wasm" | awk '{printf "build_wasm: ord_bg.wasm is %.1f MB\n", $5 / 1048576}'
