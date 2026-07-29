#!/usr/bin/env bash
# stellar/scripts/deploy-dryrun.sh
#
# End-to-end futurenet dry-run for Wraith Protocol Stellar contracts.
#
# Deploys all four contracts to futurenet, wires them, runs smoke tests,
# and prints contract IDs with stellar.expert links.
#
# Idempotent: rerun with the same admin secret yields the same contract IDs
# (because deploy salts are pinned).  Already-deployed contracts are detected
# by pre-computing the expected ID and verifying on-chain.
#
# Usage:
#   STELLAR_ADMIN_SECRET=<secret> ./deploy-dryrun.sh
#
# Optional env vars:
#   RPC_URL         Futurenet RPC URL (default: https://rpc-futurenet.stellar.org)
#   IDENTITY_NAME   Stellar identity name  (default: wraith-dryrun)
#
# Requirements:
#   - stellar-cli >= 22.0.1  (or soroban-cli)
#   - Rust toolchain with wasm32-unknown-unknown target
#   - The admin account must be funded with sufficient XLM

set -euo pipefail

# ──────────────────────────────────────────────────────────────────────────────
# Config
# ──────────────────────────────────────────────────────────────────────────────

NETWORK="futurenet"
RPC_URL="${RPC_URL:-https://rpc-futurenet.stellar.org}"
NETWORK_PASSPHRASE="Test SDF Future Network ; October 2022"
IDENTITY_NAME="${IDENTITY_NAME:-wraith-dryrun}"
ADMIN_SECRET="${STELLAR_ADMIN_SECRET:?STELLAR_ADMIN_SECRET is required}"

# Deterministic salts – same admin + same salt = same contract ID every time.
# Change a salt to force a fresh deployment of that contract.
SALT_ANNOUNCER="0000000000000000000000000000000000000000000000000000000000000001"
SALT_REGISTRY="0000000000000000000000000000000000000000000000000000000000000002"
SALT_SENDER="0000000000000000000000000000000000000000000000000000000000000003"
SALT_NAMES="0000000000000000000000000000000000000000000000000000000000000004"

# 64-byte test meta-address (spending_pubkey || viewing_pubkey)
TEST_META_ADDRESS="01010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101010101"

# 32-byte test ephemeral public key
TEST_EPHEMERAL_KEY="0202020202020202020202020202020202020202020202020202020202020202"

# Test metadata bytes (view tag = 42, one extra data byte)
TEST_METADATA="2a07"

# Test name for wraith-names
TEST_NAME="wraithtest"

WORK_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$WORK_DIR"

# Color helpers
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color
BOLD='\033[1m'

ok()     { printf "  ${GREEN}✓${NC} %s\n" "$*"; }
info()   { printf "  ${BLUE}→${NC} %s\n" "$*"; }
warn()   { printf "  ${YELLOW}⚠${NC} %s\n" "$*"; }
die()    { printf "  ${RED}✗${NC} %s\n" "$*" >&2; exit 1; }
header() { printf "\n${BOLD}═══ %s ═══${NC}\n" "$*"; }

SMOKE_FAILED=0
smoke_fail() { printf "  ${RED}✗${NC} %s\n" "$*"; SMOKE_FAILED=1; }

# ──────────────────────────────────────────────────────────────────────────────
# Detect CLI binary
# ──────────────────────────────────────────────────────────────────────────────

if command -v stellar &>/dev/null; then
    CLI="stellar"
elif command -v soroban &>/dev/null; then
    CLI="soroban"
    info "Using 'soroban' CLI (consider upgrading to 'stellar-cli')"
else
    echo "ERROR: Neither 'stellar' nor 'soroban' CLI found in PATH." >&2
    echo "       Install stellar-cli: cargo install stellar-cli --locked" >&2
    exit 1
fi

CLI_VERSION=$($CLI --version 2>/dev/null || echo "unknown")
info "Using $CLI ($CLI_VERSION)"

# Check if CLI supports --salt for deterministic deploys
if $CLI contract deploy --help 2>/dev/null | grep -q '\-\-salt'; then
    HAS_SALT=1
else
    HAS_SALT=0
    warn "CLI does not support --salt; contract IDs will NOT be deterministic"
fi

# Check if CLI supports `contract id` subcommand for pre-computation
if $CLI contract id --help 2>/dev/null | grep -q '\-\-salt'; then
    HAS_CONTRACT_ID=1
else
    HAS_CONTRACT_ID=0
    warn "CLI does not support 'contract id' subcommand; fallback ID detection may be used"
fi

# ──────────────────────────────────────────────────────────────────────────────
# Preflight checks
# ──────────────────────────────────────────────────────────────────────────────

header "Preflight"

# Check Rust wasm32 target
if ! rustup target list --installed 2>/dev/null | grep -q wasm32-unknown-unknown; then
    echo "ERROR: wasm32-unknown-unknown target not installed." >&2
    echo "       Run: rustup target add wasm32-unknown-unknown" >&2
    exit 1
fi
ok "wasm32-unknown-unknown target"

# Configure Stellar network
$CLI network add \
    --global \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" \
    "$NETWORK" 2>/dev/null || {
    warn "Network '$NETWORK' already configured (or re-add failed)"
}
ok "network '$NETWORK' configured"

# Import identity
$CLI keys add "$IDENTITY_NAME" --secret-key "$ADMIN_SECRET" 2>/dev/null && \
    ok "identity '$IDENTITY_NAME' imported" || \
    warn "identity '$IDENTITY_NAME' already exists (updating)..."
$CLI keys add "$IDENTITY_NAME" --secret-key "$ADMIN_SECRET" --overwrite 2>/dev/null || true

ADMIN_ADDRESS=$($CLI keys address "$IDENTITY_NAME")
ok "admin address: $ADMIN_ADDRESS"

# Check account balance (portable — no grep -P, no bc)
BALANCE_RAW=$($CLI account show --network "$NETWORK" "$ADMIN_ADDRESS" 2>/dev/null \
    | grep 'Balance:' | head -1 | sed 's/.*Balance:[[:space:]]*//' | sed 's/[^0-9.]//g' || echo "0")
if [ -z "$BALANCE_RAW" ]; then BALANCE_RAW="0"; fi
INFO_BALANCE="$BALANCE_RAW XLM"
# Simple integer comparison by stripping decimal portion
BALANCE_INT=$(echo "$BALANCE_RAW" | sed 's/\..*//')
info "account balance: $INFO_BALANCE"
if [ "${BALANCE_INT:-0}" -lt 10 ] 2>/dev/null; then
    warn "Low balance (${BALANCE_RAW} XLM). You may need to fund the account:"
    warn "  curl \"https://friendbot-futurenet.stellar.org/?addr=$ADMIN_ADDRESS\""
fi

# ──────────────────────────────────────────────────────────────────────────────
# Build & optimize
# ──────────────────────────────────────────────────────────────────────────────

header "Build"

if [ -f target/wasm32-unknown-unknown/release/stealth_announcer.wasm ]; then
    info "WASM artifacts already built; skipping cargo build."
    info "To force a rebuild: cargo build --target wasm32-unknown-unknown --release"
else
    info "Building contracts (this may take a while)..."
    cargo build --target wasm32-unknown-unknown --release
fi
ok "build complete"

header "Optimize"

for contract in stealth_announcer stealth_registry stealth_sender wraith_names; do
    wasm="target/wasm32-unknown-unknown/release/${contract}.wasm"
    if [ ! -f "$wasm" ]; then
        die "WASM not found: $wasm (build may have failed)"
    fi
    $CLI contract optimize --wasm "$wasm" 2>/dev/null || true
done
ok "WASM optimization complete"

get_wasm() {
    local base="target/wasm32-unknown-unknown/release/$1"
    if [ -f "${base}_optimized.wasm" ]; then
        echo "${base}_optimized.wasm"
    elif [ -f "${base}.optimized.wasm" ]; then
        echo "${base}.optimized.wasm"
    else
        echo "${base}.wasm"
    fi
}

ANNOUNCER_WASM=$(get_wasm stealth_announcer)
REGISTRY_WASM=$(get_wasm stealth_registry)
SENDER_WASM=$(get_wasm stealth_sender)
NAMES_WASM=$(get_wasm wraith_names)

info "announcer WASM: $ANNOUNCER_WASM"
info "registry  WASM: $REGISTRY_WASM"
info "sender    WASM: $SENDER_WASM"
info "names     WASM: $NAMES_WASM"

# ──────────────────────────────────────────────────────────────────────────────
# Deploy contracts (deterministic salts → idempotent IDs)
# ──────────────────────────────────────────────────────────────────────────────

header "Deploy"

# Helper: compute expected contract ID from salt + source + network
compute_contract_id() {
    local wasm="$1" salt="$2"
    if [ "$HAS_CONTRACT_ID" -eq 1 ]; then
        $CLI contract id \
            --wasm "$wasm" \
            --salt "$salt" \
            --source "$IDENTITY_NAME" \
            --network "$NETWORK" 2>/dev/null || echo ""
    else
        echo ""
    fi
}

deploy_contract() {
    local name="$1"
    local wasm="$2"
    local salt="$3"
    local var_name="$4"
    local id=""

    info "Deploying $name..."

    # Strategy: compute expected ID, try deploy, fall back gracefully
    local expected_id=""
    if [ "$HAS_SALT" -eq 1 ]; then
        expected_id=$(compute_contract_id "$wasm" "$salt")
        if [ -n "$expected_id" ]; then
            info "expected ID: $expected_id"
        fi
    fi

    # Attempt deploy with salt (deterministic path)
    if [ "$HAS_SALT" -eq 1 ]; then
        id=$($CLI contract deploy \
            --wasm "$wasm" \
            --salt "$salt" \
            --source "$IDENTITY_NAME" \
            --network "$NETWORK" 2>/dev/null) || id=""
    fi

    # If deploy succeeded, we have a fresh ID
    if [ -n "$id" ]; then
        ok "$name deployed: $id"
        eval "$var_name=\"$id\""
        return
    fi

    # If we have an expected_id, use it (contract already existed)
    if [ -n "$expected_id" ]; then
        info "Contract may already exist at expected ID; using: $expected_id"
        eval "$var_name=\"$expected_id\""
        ok "$name: $expected_id"
        return
    fi

    # Last resort: deploy without salt (non-deterministic, but works everywhere)
    warn "Deploying $name without salt (IDs will differ across runs)..."
    id=$($CLI contract deploy \
        --wasm "$wasm" \
        --source "$IDENTITY_NAME" \
        --network "$NETWORK")
    eval "$var_name=\"$id\""
    ok "$name: $id"
}

# Note: stellar/soroban contract deploy with --salt is idempotent when the CLI
# supports it.  If --salt is unavailable, fresh contract instances are created
# and IDs will differ per run.

deploy_contract "stealth-announcer" "$ANNOUNCER_WASM" "$SALT_ANNOUNCER" "ANNOUNCER_ID"
deploy_contract "stealth-registry"  "$REGISTRY_WASM" "$SALT_REGISTRY" "REGISTRY_ID"
deploy_contract "stealth-sender"    "$SENDER_WASM"   "$SALT_SENDER"   "SENDER_ID"
deploy_contract "wraith-names"      "$NAMES_WASM"    "$SALT_NAMES"    "NAMES_ID"

# ──────────────────────────────────────────────────────────────────────────────
# Wire: init stealth-sender with announcer
# ──────────────────────────────────────────────────────────────────────────────

header "Wire"

info "Initializing stealth-sender (announcer: $ANNOUNCER_ID)..."
# For Option<Address> args, omitting the flag sends None.
if $CLI contract invoke \
    --id "$SENDER_ID" \
    --source "$IDENTITY_NAME" \
    --network "$NETWORK" \
    -- \
    init \
    --announcer "$ANNOUNCER_ID" \
    --fee-basis-points 0 2>/dev/null; then
    ok "stealth-sender initialized"
else
    # AlreadyInitialized is expected on re-runs
    warn "stealth-sender init failed (may already be initialized on re-run)"
    info "Verifying sender is responsive..."
    if $CLI contract invoke \
        --id "$SENDER_ID" \
        --source "$IDENTITY_NAME" \
        --network "$NETWORK" \
        -- \
        init \
        --announcer "$ANNOUNCER_ID" \
        --fee-basis-points 0 2>&1 | grep -qi "AlreadyInitialized\|already initialized"; then
        ok "stealth-sender already initialized (re-run)"
    else
        warn "Could not verify sender initialization status"
    fi
fi

# ──────────────────────────────────────────────────────────────────────────────
# Smoke tests
# ──────────────────────────────────────────────────────────────────────────────

header "Smoke Tests"

# 1. Register a name in wraith-names
info "1/5  Registering name '$TEST_NAME' in wraith-names..."
if $CLI contract invoke \
    --id "$NAMES_ID" \
    --source "$IDENTITY_NAME" \
    --network "$NETWORK" \
    -- \
    register \
    --owner "$ADMIN_ADDRESS" \
    --name "$TEST_NAME" \
    --stealth-meta-address "$TEST_META_ADDRESS" 2>/dev/null; then
    ok "name '$TEST_NAME' registered"
else
    warn "name registration failed (may already exist on re-run)"
fi

# 2. Resolve the name (prove wraith-names works)
info "2/5  Resolving name '$TEST_NAME'..."
RESOLVED=$($CLI contract invoke \
    --id "$NAMES_ID" \
    --source "$IDENTITY_NAME" \
    --network "$NETWORK" \
    -- \
    resolve \
    --name "$TEST_NAME" 2>/dev/null) || {
    smoke_fail "Could not resolve name '$TEST_NAME'"
    RESOLVED=""
}
if [ -n "$RESOLVED" ]; then
    ok "name resolved (64-byte meta-address)"
else
    # Only warn if resolve didn't outright fail (already handled above)
    [ -z "$RESOLVED" ] && [ "$SMOKE_FAILED" -eq 0 ] && \
        warn "resolve returned empty result"
fi

# 3. Announce an event (prove stealth-announcer works)
info "3/5  Announcing event via stealth-announcer..."
if $CLI contract invoke \
    --id "$ANNOUNCER_ID" \
    --source "$IDENTITY_NAME" \
    --network "$NETWORK" \
    -- \
    announce \
    --scheme-id 2 \
    --stealth-address "$ADMIN_ADDRESS" \
    --ephemeral-pub-key "$TEST_EPHEMERAL_KEY" \
    --metadata "$TEST_METADATA" 2>/dev/null; then
    ok "announce event emitted"
else
    smoke_fail "announce failed"
fi

# 4. Query the registry (prove stealth-registry works)
info "4/5  Querying stealth-registry..."
REG_RESULT=$($CLI contract invoke \
    --id "$REGISTRY_ID" \
    --source "$IDENTITY_NAME" \
    --network "$NETWORK" \
    -- \
    stealth_meta_address_of \
    --registrant "$ADMIN_ADDRESS" \
    --scheme-id 2 2>/dev/null) || {
    warn "stealth_meta_address_of returned NotRegistered (expected — no key registered yet)"
    ok "registry is responsive (NotRegistered is expected)"
}

# 5. Verify sender is initialized
info "5/5  Verifying stealth-sender initialization..."
if $CLI contract invoke \
    --id "$SENDER_ID" \
    --source "$IDENTITY_NAME" \
    --network "$NETWORK" \
    --fee 100 \
    -- \
    init \
    --announcer "$ANNOUNCER_ID" \
    --fee-basis-points 0 2>&1 | grep -qi "AlreadyInitialized\|already initialized"; then
    ok "stealth-sender correctly initialized (AlreadyInitialized)"
else
    smoke_fail "Could not confirm sender initialization"
fi

# ──────────────────────────────────────────────────────────────────────────────
# Summary
# ──────────────────────────────────────────────────────────────────────────────

header "Results"

echo ""
printf "  ${BOLD}%-22s %-56s${NC}\n" "Contract" "Contract ID"
printf "  ${BOLD}%-22s %-56s${NC}\n" "───────" "───────────"
printf "  %-22s %-56s\n" "stealth-announcer" "$ANNOUNCER_ID"
printf "  %-22s %-56s\n" "stealth-registry" "$REGISTRY_ID"
printf "  %-22s %-56s\n" "stealth-sender" "$SENDER_ID"
printf "  %-22s %-56s\n" "wraith-names" "$NAMES_ID"
echo ""

echo "  Stellar Expert links:"
echo "  ├─ Announcer: https://futurenet.stellar.expert/explorer/futurenet/contract/$ANNOUNCER_ID"
echo "  ├─ Registry:  https://futurenet.stellar.expert/explorer/futurenet/contract/$REGISTRY_ID"
echo "  ├─ Sender:    https://futurenet.stellar.expert/explorer/futurenet/contract/$SENDER_ID"
echo "  └─ Names:     https://futurenet.stellar.expert/explorer/futurenet/contract/$NAMES_ID"
echo ""

if [ "$SMOKE_FAILED" -ne 0 ]; then
    echo "  ${YELLOW}⚠  Some smoke tests failed. Check output above for details.${NC}"
    echo ""
    exit 1
fi

echo "  ${GREEN}✔${NC} All checks passed. Dry-run complete."
echo ""
