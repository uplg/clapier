#!/usr/bin/env bash
set -euo pipefail

require_command() {
    local name="$1"
    local hint="${2:-}"
    if ! command -v "$name" >/dev/null 2>&1; then
        echo "Error: required command not found in PATH: $name" >&2
        if [[ -n "$hint" ]]; then
            echo "Install hint: $hint" >&2
        fi
        exit 1
    fi
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "$REPO_ROOT"

require_command cargo
require_command wasm-bindgen "cargo install wasm-bindgen-cli"

echo "Building pocket-tts for wasm32-unknown-unknown (release)..."
cargo build -p pocket-tts --release --target wasm32-unknown-unknown --features wasm

TARGET_BASE="${CARGO_TARGET_DIR:-target}"
candidates=(
    "${TARGET_BASE}/wasm32-unknown-unknown/release/pocket_tts.wasm"
    "target/wasm32-unknown-unknown/release/pocket_tts.wasm"
)

WASM_PATH=""
for candidate in "${candidates[@]}"; do
    if [[ -f "$candidate" ]]; then
        WASM_PATH="$candidate"
        break
    fi
done

if [[ -z "$WASM_PATH" ]]; then
    echo "Error: could not find pocket_tts.wasm after build." >&2
    echo "Checked paths:" >&2
    for candidate in "${candidates[@]}"; do
        echo "  - $candidate" >&2
    done
    exit 1
fi

OUT_DIR="crates/pocket-tts/pkg"
mkdir -p "$OUT_DIR"

echo "Running wasm-bindgen..."
wasm-bindgen --target web --out-dir "$OUT_DIR" "$WASM_PATH"

BG_WASM="${OUT_DIR}/pocket_tts_bg.wasm"
if command -v wasm-opt >/dev/null 2>&1; then
    echo "Running wasm-opt -O3..."
    wasm-opt -O3 --enable-mutable-globals -o "$BG_WASM" "$BG_WASM"
else
    echo "Warning: wasm-opt not found. Skipping optimization step." >&2
fi

echo
echo "WASM build complete."
echo "Artifacts:"
echo "  - crates/pocket-tts/pkg/pocket_tts.js"
echo "  - crates/pocket-tts/pkg/pocket_tts_bg.wasm"
