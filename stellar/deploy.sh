#!/bin/bash

# Stellar Deployment Script for Wraith Protocol
# Usage: ./deploy.sh [testnet|futurenet|mainnet] [secret-key-name] [--force] [--dry-run] [--resume] [--journal <path>]

NETWORK=$1
IDENTITY=$2

if [[ -z "$NETWORK" || -z "$IDENTITY" ]]; then
    echo "Usage: ./deploy.sh <network> <identity> [--force] [--dry-run] [--resume] [--journal <path>]"
    exit 1
fi

FORCE=0
DRY_RUN=0
RESUME=0
JOURNAL_FILE=""

shift 2
while [[ $# -gt 0 ]]; do
  case "$1" in
    --force)
      FORCE=1
      shift
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --resume)
      RESUME=1
      shift
      ;;
    --journal)
      JOURNAL_FILE="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done

# Default journal path if not explicitly provided
if [[ -z "$JOURNAL_FILE" ]]; then
    JOURNAL_FILE=".deployment-journal-${NETWORK}.json"
fi

# Deployments are recorded in the repository-wide manifest (deployments/README.md).
RECORD_FILE="../deployments/stellar/stellar-${NETWORK}.json"
MANIFEST_TOOL="../scripts/deployment-manifest"

case "$NETWORK" in
    testnet)   DEFAULT_RPC="https://soroban-testnet.stellar.org" ;;
    futurenet) DEFAULT_RPC="https://rpc-futurenet.stellar.org" ;;
    *)         DEFAULT_RPC="" ;;
esac
RPC_URL="${RPC_URL:-$DEFAULT_RPC}"

if [[ -f "$RECORD_FILE" && $FORCE -eq 0 && $DRY_RUN -eq 0 && $RESUME -eq 0 ]]; then
    echo "Error: Deployment record $RECORD_FILE already exists."
    echo "Use --force to overwrite or --resume to continue an incomplete deployment."
    exit 1
fi

if [[ -z "$RPC_URL" && $DRY_RUN -eq 0 ]]; then
    echo "Error: set RPC_URL for network $NETWORK (used to record deployment ledgers)."
    exit 1
fi

if [[ ! -d "$MANIFEST_TOOL/node_modules" && $DRY_RUN -eq 0 ]]; then
    echo "Error: run 'npm ci' in scripts/deployment-manifest first."
    exit 1
fi

STELLAR_DIR=$(pwd)
crate_version() {
    grep -m1 -E '^version' "$STELLAR_DIR/$1/Cargo.toml" | sed -E 's/.*"(.*)".*/\1/'
}

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

REQUIRED_CONTRACTS=("stealth-announcer" "stealth-registry" "stealth-sender" "wraith-names")

echo "🚀 Deploying Wraith Protocol to $NETWORK using $IDENTITY..."

# Obtain deployer address
if command -v soroban &>/dev/null; then
    DEPLOYER_PUBKEY=$(soroban keys address "$IDENTITY" 2>/dev/null || echo "DRYRUN_DEPLOYER_PUBKEY")
elif command -v stellar &>/dev/null; then
    DEPLOYER_PUBKEY=$(stellar keys address "$IDENTITY" 2>/dev/null || echo "DRYRUN_DEPLOYER_PUBKEY")
else
    DEPLOYER_PUBKEY="DRYRUN_DEPLOYER_PUBKEY"
fi

SOURCE_COMMIT=$(git rev-parse HEAD 2>/dev/null || echo "0000000000000000000000000000000000000000")

# If resuming, validate journal strictly (Fail-Closed)
if [[ $RESUME -eq 1 ]]; then
    if [[ ! -f "$JOURNAL_FILE" ]]; then
        echo "Error: --resume specified but journal file $JOURNAL_FILE does not exist."
        exit 1
    fi
    echo "--- Validating Existing Run Journal for Resume (Fail-Closed) ---"
    if ! (cd "$MANIFEST_TOOL" && npx tsx src/cli.ts journal validate \
        --file "$STELLAR_DIR/$JOURNAL_FILE" \
        --network "stellar-${NETWORK}" \
        --rpc "$RPC_URL" \
        --source-commit "$SOURCE_COMMIT" \
        --deployer "$DEPLOYER_PUBKEY" \
        --wasm "stealth-announcer=$STELLAR_DIR/$ANNOUNCER_WASM" \
        --wasm "stealth-registry=$STELLAR_DIR/$REGISTRY_WASM" \
        --wasm "stealth-sender=$STELLAR_DIR/$SENDER_WASM" \
        --wasm "wraith-names=$STELLAR_DIR/$NAMES_WASM"); then
        echo "❌ Resume validation failed: partial state rejected due to inconsistency."
        exit 1
    fi
    echo "✅ Run journal verified safely."
fi

# Handle --dry-run
if [[ $DRY_RUN -eq 1 ]]; then
    echo "--- Dry-Run Mode ---"
    echo "[DRY-RUN] Will build and optimize contracts"
    (cd "$MANIFEST_TOOL" && npx tsx src/cli.ts journal plan \
        --file "$STELLAR_DIR/$JOURNAL_FILE" \
        --contract stealth-announcer \
        --contract stealth-registry \
        --contract stealth-sender \
        --contract wraith-names) || true
    echo "[DRY-RUN] Canonical record will be written to $RECORD_FILE only after full verification."
    exit 0
fi

# Initialize journal if fresh run and does not exist
if [[ ! -f "$JOURNAL_FILE" ]]; then
    echo "--- Initializing Run Journal ---"
    (cd "$MANIFEST_TOOL" && npx tsx src/cli.ts journal init \
        --file "$STELLAR_DIR/$JOURNAL_FILE" \
        --network "stellar-${NETWORK}" \
        --rpc "$RPC_URL" \
        --source-commit "$SOURCE_COMMIT" \
        --deployer "$DEPLOYER_PUBKEY")
fi

# Build and optimize
echo "--- Building Contracts ---"
cargo build --target wasm32-unknown-unknown --release

echo "--- Optimizing Contracts ---"
for contract in stealth_announcer stealth_registry stealth_sender wraith_names; do
    if command -v stellar &>/dev/null; then
        stellar contract optimize --wasm target/wasm32-unknown-unknown/release/${contract}.wasm
    fi
done

# Check which contracts can be reused from journal
get_journal_contract_id() {
    local cname="$1"
    if [[ -f "$JOURNAL_FILE" ]]; then
        node -e "
            try {
                const j = JSON.parse(require('fs').readFileSync('$JOURNAL_FILE', 'utf8'));
                process.stdout.write(j.contracts['$cname']?.contractId || '');
            } catch (e) {}
        "
    fi
}

# 1. stealth-announcer
EXISTING_ANNOUNCER=$(get_journal_contract_id "stealth-announcer")
if [[ -n "$EXISTING_ANNOUNCER" ]]; then
    echo "♻️ [REUSE] stealth-announcer: $EXISTING_ANNOUNCER"
    ANNOUNCER_ID="$EXISTING_ANNOUNCER"
else
    echo "--- Deploying stealth-announcer ---"
    ANNOUNCER_ID=$(soroban contract deploy \
        --wasm "$ANNOUNCER_WASM" \
        --source "$IDENTITY" \
        --network "$NETWORK")
    echo "✅ stealth-announcer: $ANNOUNCER_ID"
    (cd "$MANIFEST_TOOL" && npx tsx src/cli.ts journal record-step \
        --file "$STELLAR_DIR/$JOURNAL_FILE" \
        --name stealth-announcer \
        --id "$ANNOUNCER_ID" \
        --wasm "stealth-announcer=$STELLAR_DIR/$ANNOUNCER_WASM")
fi

# 2. stealth-registry
EXISTING_REGISTRY=$(get_journal_contract_id "stealth-registry")
if [[ -n "$EXISTING_REGISTRY" ]]; then
    echo "♻️ [REUSE] stealth-registry: $EXISTING_REGISTRY"
    REGISTRY_ID="$EXISTING_REGISTRY"
else
    echo "--- Deploying stealth-registry ---"
    REGISTRY_ID=$(soroban contract deploy \
        --wasm "$REGISTRY_WASM" \
        --source "$IDENTITY" \
        --network "$NETWORK")
    echo "✅ stealth-registry: $REGISTRY_ID"
    (cd "$MANIFEST_TOOL" && npx tsx src/cli.ts journal record-step \
        --file "$STELLAR_DIR/$JOURNAL_FILE" \
        --name stealth-registry \
        --id "$REGISTRY_ID" \
        --wasm "stealth-registry=$STELLAR_DIR/$REGISTRY_WASM")
fi

# 3. stealth-sender
EXISTING_SENDER=$(get_journal_contract_id "stealth-sender")
if [[ -n "$EXISTING_SENDER" ]]; then
    echo "♻️ [REUSE] stealth-sender: $EXISTING_SENDER"
    SENDER_ID="$EXISTING_SENDER"
else
    echo "--- Deploying stealth-sender ---"
    SENDER_ID=$(soroban contract deploy \
        --wasm "$SENDER_WASM" \
        --source "$IDENTITY" \
        --network "$NETWORK")
    echo "✅ stealth-sender: $SENDER_ID"
    (cd "$MANIFEST_TOOL" && npx tsx src/cli.ts journal record-step \
        --file "$STELLAR_DIR/$JOURNAL_FILE" \
        --name stealth-sender \
        --id "$SENDER_ID" \
        --wasm "stealth-sender=$STELLAR_DIR/$SENDER_WASM")
fi

# 4. wraith-names
EXISTING_NAMES=$(get_journal_contract_id "wraith-names")
if [[ -n "$EXISTING_NAMES" ]]; then
    echo "♻️ [REUSE] wraith-names: $EXISTING_NAMES"
    NAMES_ID="$EXISTING_NAMES"
else
    echo "--- Deploying wraith-names ---"
    NAMES_ID=$(soroban contract deploy \
        --wasm "$NAMES_WASM" \
        --source "$IDENTITY" \
        --network "$NETWORK")
    echo "✅ wraith-names: $NAMES_ID"
    (cd "$MANIFEST_TOOL" && npx tsx src/cli.ts journal record-step \
        --file "$STELLAR_DIR/$JOURNAL_FILE" \
        --name wraith-names \
        --id "$NAMES_ID" \
        --wasm "wraith-names=$STELLAR_DIR/$NAMES_WASM")
fi

# Record canonical manifest ONLY when all required contracts are present and verified
echo "--- Recording Deployment to Canonical Manifest ---"
if (cd "$MANIFEST_TOOL" && npx tsx src/cli.ts record stellar \
    --network "stellar-${NETWORK}" \
    --rpc "$RPC_URL" \
    ${DEFAULT_RPC:+--public-rpc "$DEFAULT_RPC"} \
    --recorded-by deploy-script \
    --source-commit HEAD \
    --deployer "$DEPLOYER_PUBKEY" \
    --contract "stealth-announcer=$ANNOUNCER_ID" --wasm "stealth-announcer=$STELLAR_DIR/$ANNOUNCER_WASM" \
    --contract "stealth-registry=$REGISTRY_ID" --wasm "stealth-registry=$STELLAR_DIR/$REGISTRY_WASM" \
    --contract "stealth-sender=$SENDER_ID" --wasm "stealth-sender=$STELLAR_DIR/$SENDER_WASM" \
    --contract "wraith-names=$NAMES_ID" --wasm "wraith-names=$STELLAR_DIR/$NAMES_WASM" \
    --version "stealth-announcer=$(crate_version stealth-announcer)" \
    --version "stealth-registry=$(crate_version stealth-registry)" \
    --version "stealth-sender=$(crate_version stealth-sender)" \
    --version "wraith-names=$(crate_version wraith-names)"); then
    echo "✅ Recorded to $RECORD_FILE and rebuilt deployments/manifest.json"
    (cd "$MANIFEST_TOOL" && npx tsx src/cli.ts journal complete --file "$STELLAR_DIR/$JOURNAL_FILE")
    RECORD_FAILED=0
else
    echo "❌ Recording failed. Partial state rejected."
    RECORD_FAILED=1
fi

# Initialize stealth-sender with announcer ID
echo "Initializing stealth-sender..."
soroban contract invoke \
    --id "$SENDER_ID" \
    --source "$IDENTITY" \
    --network "$NETWORK" \
    -- \
    init \
    --admin "$IDENTITY" \
    --announcer "$ANNOUNCER_ID" || true

# Verification step
echo "--- Verifying Deployments ---"
echo "Checking stealth-sender admin..."
soroban contract invoke \
    --id "$SENDER_ID" \
    --source "$IDENTITY" \
    --network "$NETWORK" \
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

if [[ $RECORD_FAILED -eq 1 ]]; then
    echo "❌ Deployment succeeded but was not recorded in deployments/. See the error above."
    exit 1
fi
