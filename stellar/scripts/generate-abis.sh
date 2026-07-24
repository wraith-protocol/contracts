#!/bin/bash
set -e

# Ensure we are in the stellar directory
cd "$(dirname "$0")/.."

mkdir -p abi

echo "Building contracts..."
cargo build --target wasm32-unknown-unknown --release

CONTRACTS=(
  "stealth_announcer"
  "stealth_registry"
  "stealth_sender"
  "wraith_names"
)

for contract in "${CONTRACTS[@]}"; do
  echo "Generating ABI for $contract..."
  stellar contract inspect \
    --wasm "target/wasm32-unknown-unknown/release/${contract}.wasm" \
    > "abi/${contract}.json"
done

echo "Successfully generated ABIs."
