#!/usr/bin/env bash
set -euo pipefail

# Navigate to the stellar workspace root
cd "$(dirname "$0")/.."

echo "Building contracts..."
cargo build --target wasm32-unknown-unknown --release

echo "Extracting ABI snapshots..."
for contract in stealth_announcer stealth_registry stealth_sender wraith_names; do
    stellar contract info interface \
        --wasm "target/wasm32-unknown-unknown/release/${contract}.wasm" \
        --output json-formatted > "abi/${contract}.json"
    echo "Saved abi/${contract}.json"
done

echo "Success! ABI snapshots updated."
