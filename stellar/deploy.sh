#!/bin/bash

# Stellar Deployment Script for Wraith Protocol
# Usage: ./deploy.sh [testnet|futurenet|mainnet] [secret-key-name]

NETWORK=$1
IDENTITY=$2

if [[ -z "$NETWORK" || -z "$IDENTITY" ]]; then
    echo "Usage: ./deploy.sh [testnet|futurenet|mainnet] [secret-key-name]"
    exit 1
fi

echo "🚀 Deploying Wraith Protocol to $NETWORK using $IDENTITY..."

# 1. Deploy stealth-announcer
echo "--- Deploying stealth-announcer ---"
ANNOUNCER_ID=$(soroban contract deploy \
    --wasm target/wasm32-unknown-unknown/release/stealth_announcer.wasm \
    --source $IDENTITY \
    --network $NETWORK)
echo "✅ stealth-announcer: $ANNOUNCER_ID"

# 2. Deploy stealth-registry
echo "--- Deploying stealth-registry ---"
REGISTRY_ID=$(soroban contract deploy \
    --wasm target/wasm32-unknown-unknown/release/stealth_registry.wasm \
    --source $IDENTITY \
    --network $NETWORK)
echo "✅ stealth-registry: $REGISTRY_ID"

# 3. Deploy stealth-sender
echo "--- Deploying stealth-sender ---"
SENDER_ID=$(soroban contract deploy \
    --wasm target/wasm32-unknown-unknown/release/stealth_sender.wasm \
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
    --wasm target/wasm32-unknown-unknown/release/wraith_names.wasm \
    --source $IDENTITY \
    --network $NETWORK)
echo "✅ wraith-names: $NAMES_ID"

echo ""
echo "🎉 Deployment Complete!"
echo "--------------------------------------"
echo "Announcer: $ANNOUNCER_ID"
echo "Registry:  $REGISTRY_ID"
echo "Sender:    $SENDER_ID"
echo "Names:     $NAMES_ID"
echo "--------------------------------------"
