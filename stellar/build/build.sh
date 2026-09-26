#!/bin/bash
set -euo pipefail

# This script is meant to be run inside the reproducible Docker container.
# It builds the contracts, optimizes them, and outputs attestation.json.

WORKSPACE_DIR="/workspace"
cd $WORKSPACE_DIR/stellar

echo "Building contracts in $(pwd)..."

# Build all workspace members
cargo build --target wasm32-unknown-unknown --release

OUT_DIR="target/wasm32-unknown-unknown/release"
OPTIMIZED_DIR="target/optimized"
mkdir -p "$OPTIMIZED_DIR"

# Ensure stellar-cli is available
if ! command -v stellar &> /dev/null; then
    echo "Error: stellar-cli not found"
    exit 1
fi

RUST_VERSION=$(rustc --version | awk '{print $2}')
CARGO_VERSION=$(cargo --version | awk '{print $2}')
RUSTUP_VERSION=$(rustup --version 2>/dev/null | awk '{print $2}')
STELLAR_CLI_VERSION=$(stellar --version | grep "stellar-cli" | awk '{print $2}')
SOROBAN_SDK_VERSION=$(awk '/^name = "soroban-sdk"$/ { getline; gsub(/"/, "", $3); print $3; exit }' Cargo.lock)
CARGO_LOCK_SHA256=$(sha256sum Cargo.lock | awk '{print $1}')
BASE_IMAGE=$(awk 'toupper($1) == "FROM" { print $2; exit }' build/Dockerfile)
COMMIT_HASH=${COMMIT_HASH:-"unknown"}
BUILD_DATE=$(date -u +%Y-%m-%d)

ATTESTATION_FILE="build/attestation.json"

echo "{" > $ATTESTATION_FILE
echo "  \"commit\": \"$COMMIT_HASH\"," >> $ATTESTATION_FILE
echo "  \"build_date\": \"$BUILD_DATE\"," >> $ATTESTATION_FILE
echo "  \"toolchain\": {" >> $ATTESTATION_FILE
echo "    \"rust\": \"$RUST_VERSION\"," >> $ATTESTATION_FILE
echo "    \"cargo\": \"$CARGO_VERSION\"," >> $ATTESTATION_FILE
echo "    \"rustup\": \"$RUSTUP_VERSION\"," >> $ATTESTATION_FILE
echo "    \"stellar-cli\": \"$STELLAR_CLI_VERSION\"," >> $ATTESTATION_FILE
echo "    \"soroban-sdk\": \"$SOROBAN_SDK_VERSION\"," >> $ATTESTATION_FILE
echo "    \"target\": \"wasm32-unknown-unknown\"," >> $ATTESTATION_FILE
echo "    \"base_image\": \"$BASE_IMAGE\"," >> $ATTESTATION_FILE
echo "    \"cargo_lock_sha256\": \"$CARGO_LOCK_SHA256\"" >> $ATTESTATION_FILE
echo "  }," >> $ATTESTATION_FILE
echo "  \"contracts\": [" >> $ATTESTATION_FILE

FIRST=true

# Find all wasm files in the release directory
for WASM in "$OUT_DIR"/*.wasm; do
    if [ ! -f "$WASM" ]; then
        continue
    fi
    
    FILENAME=$(basename "$WASM")
    CONTRACT_NAME="${FILENAME%.*}"
    
    echo "Optimizing $CONTRACT_NAME..."
    stellar contract optimize --wasm "$WASM" --wasm-out "$OPTIMIZED_DIR/$FILENAME"
    
    # Calculate SHA256 of optimized WASM
    WASM_SHA256=$(sha256sum "$OPTIMIZED_DIR/$FILENAME" | awk '{print $1}')
    WASM_SIZE=$(stat -c%s "$OPTIMIZED_DIR/$FILENAME")
    
    if [ "$FIRST" = true ]; then
        FIRST=false
    else
        echo "    ," >> $ATTESTATION_FILE
    fi
    
    echo "    { \"name\": \"$CONTRACT_NAME\", \"wasm_sha256\": \"$WASM_SHA256\", \"wasm_size\": $WASM_SIZE }" >> $ATTESTATION_FILE
done

echo "  ]" >> $ATTESTATION_FILE
echo "}" >> $ATTESTATION_FILE

echo "Build complete. Attestation generated at $ATTESTATION_FILE."
cat $ATTESTATION_FILE
