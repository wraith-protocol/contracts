#!/bin/bash

# Stellar Deployment Script for Wraith Protocol
# Usage: ./deploy.sh [testnet|futurenet|mainnet] [secret-key-name] [--force] [--dry-run]

NETWORK=$1
IDENTITY=$2

if [[ -z "$NETWORK" || -z "$IDENTITY" ]]; then
    echo "Usage: ./deploy.sh <network> <identity> [--force] [--dry-run]"
    exit 1
fi

FORCE=0
DRY_RUN=0

for arg in "$@"; do
  if [[ "$arg" == "--force" ]]; then
    FORCE=1
  fi
  if [[ "$arg" == "--dry-run" ]]; then
    DRY_RUN=1
  fi
done

MANIFEST_DIR="deployments"
MANIFEST_FILE="${MANIFEST_DIR}/${NETWORK}.json"

mkdir -p "$MANIFEST_DIR"

if [[ -f "$MANIFEST_FILE" && $FORCE -eq 0 && $DRY_RUN -eq 0 ]]; then
    echo "Error: Deployment manifest $MANIFEST_FILE already exists."
    echo "Use --force to overwrite."
    exit 1
fi

echo "🚀 Deploying Wraith Protocol to $NETWORK using $IDENTITY..."

if [[ $DRY_RUN -eq 1 ]]; then
    echo "[DRY-RUN] Will execute: cargo build --target wasm32-unknown-unknown --release"
    echo "[DRY-RUN] Will execute: stellar contract optimize on all contracts"
    echo "[DRY-RUN] Will deploy: stealth-announcer"
    echo "[DRY-RUN] Will deploy: stealth-registry"
    echo "[DRY-RUN] Will deploy: stealth-sender"
    echo "[DRY-RUN] Will invoke: stealth-sender init"
    echo "[DRY-RUN] Will deploy: wraith-names"
    echo "[DRY-RUN] Will write manifest to $MANIFEST_FILE"
    echo "[DRY-RUN] Will verify deployment status"
    exit 0
fi

# Build and optimize
echo "--- Building Contracts ---"
cargo build --target wasm32-unknown-unknown --release

echo "--- Optimizing Contracts ---"
# Optimize each contract
for contract in stealth_announcer stealth_registry stealth_sender wraith_names; do
    stellar contract optimize --wasm target/wasm32-unknown-unknown/release/${contract}.wasm
done

# The optimizer usually produces an optimized.wasm file or we can just use the original if not specified, 
# but stellar contract optimize by default creates a file in the same dir or `target/wasm32-unknown-unknown/release/${contract}.optimized.wasm`.
# Let's check if the optimized version exists, otherwise fallback to the standard release build.
get_wasm_path() {
    local base="target/wasm32-unknown-unknown/release/$1"
    if [[ -f "${base}.optimized.wasm" ]]; then
        echo "${base}.optimized.wasm"
    else
        echo "${base}.wasm"
    fi
}

ANNOUNCER_WASM=$(get_wasm_path "stealth_announcer")
REGISTRY_WASM=$(get_wasm_path "stealth_registry")
SENDER_WASM=$(get_wasm_path "stealth_sender")
NAMES_WASM=$(get_wasm_path "wraith_names")

# 1. Deploy stealth-announcer
echo "--- Deploying stealth-announcer ---"
ANNOUNCER_ID=$(soroban contract deploy \
    --wasm "$ANNOUNCER_WASM" \
    --source $IDENTITY \
    --network $NETWORK)
echo "✅ stealth-announcer: $ANNOUNCER_ID"

# 2. Deploy stealth-registry
echo "--- Deploying stealth-registry ---"
REGISTRY_ID=$(soroban contract deploy \
    --wasm "$REGISTRY_WASM" \
    --source $IDENTITY \
    --network $NETWORK)
echo "✅ stealth-registry: $REGISTRY_ID"

# 3. Deploy stealth-sender
echo "--- Deploying stealth-sender ---"
SENDER_ID=$(soroban contract deploy \
    --wasm "$SENDER_WASM" \
    --source $IDENTITY \
    --network $NETWORK)
echo "✅ stealth-sender: $SENDER_ID"

# Initialize stealth-sender with announcer ID
echo "Initializing stealth-sender..."
soroban contract invoke \
    --id $SENDER_ID \
    --source $IDENTITY \
    --network $NETWORK \
    -- \
    init \
    --admin $IDENTITY \
    --announcer $ANNOUNCER_ID

# 4. Deploy wraith-names
echo "--- Deploying wraith-names ---"
NAMES_ID=$(soroban contract deploy \
    --wasm "$NAMES_WASM" \
    --source $IDENTITY \
    --network $NETWORK)
echo "✅ wraith-names: $NAMES_ID"

# Write JSON manifest
echo "--- Writing Deployment Manifest ---"
DEPLOYER_PUBKEY=$(soroban keys address $IDENTITY 2>/dev/null || echo "$IDENTITY")
DATE_STR=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

cat > "$MANIFEST_FILE" <<EOF
{
  "network": "$NETWORK",
  "deployer": "$DEPLOYER_PUBKEY",
  "deployedAt": "$DATE_STR",
  "contracts": {
    "stealthAnnouncer": "$ANNOUNCER_ID",
    "stealthRegistry": "$REGISTRY_ID",
    "stealthSender": "$SENDER_ID",
    "wraithNames": "$NAMES_ID"
  }
}
EOF
echo "✅ Manifest written to $MANIFEST_FILE"

# Verification step (optional but nice)
echo "--- Verifying Deployments ---"
echo "Checking stealth-sender admin..."
soroban contract invoke \
    --id $SENDER_ID \
    --source $IDENTITY \
    --network $NETWORK \
    -- \
    admin || echo "⚠️ Could not read admin"

echo ""
echo "🎉 Deployment Complete!"
echo "--------------------------------------"
echo "Announcer: $ANNOUNCER_ID"
echo "Registry:  $REGISTRY_ID"
echo "Sender:    $SENDER_ID"
echo "Names:     $NAMES_ID"
echo "--------------------------------------"
