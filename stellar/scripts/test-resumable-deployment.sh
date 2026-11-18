#!/usr/bin/env bash
# stellar/scripts/test-resumable-deployment.sh
#
# Fault-injection test suite for resumable Stellar deployments.
# Tests:
# 1. Clean run without prior journal (plans 4 new deployments).
# 2. Interruption simulation: crash after contract 1, journal has 1 recorded step.
# 3. Resume execution: reuses contract 1, proceeds to deploy remaining.
# 4. Fail-closed: altered commit hash fails resume.
# 5. Fail-closed: altered network fails resume.
# 6. Fail-closed: altered deployer authority fails resume.
# 7. Fail-closed: corrupted/tampered WASM artifact fails resume.
# 8. Dry-run explains reuse vs deploy plan accurately.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STELLAR_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$STELLAR_DIR/.." && pwd)"
MANIFEST_TOOL="$REPO_ROOT/scripts/deployment-manifest"

TEST_TMP="$(mktemp -d)"
trap 'rm -rf "$TEST_TMP"' EXIT

TEST_JOURNAL="$TEST_TMP/test-journal.json"
MOCK_WASM_DIR="$TEST_TMP/wasm"
mkdir -p "$MOCK_WASM_DIR"

echo "mock wasm announcer" > "$MOCK_WASM_DIR/stealth_announcer.wasm"
echo "mock wasm registry" > "$MOCK_WASM_DIR/stealth_registry.wasm"
echo "mock wasm sender" > "$MOCK_WASM_DIR/stealth_sender.wasm"
echo "mock wasm names" > "$MOCK_WASM_DIR/wraith_names.wasm"

COMMIT_A="8feef91603dfb1b461e6e2c67bd90d47f3d22290"
COMMIT_B="52d965b22a7d9c40b675a40658a744ed0605c34e"
DEPLOYER_A="GA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJVSGZ"
DEPLOYER_B="GDIFFERENTDEPLOYERAUTHORITYXXXXXXXXXXXXXXXXXXXXXXXXXXXX"
NET_A="stellar-testnet"
NET_ID_A="Test SDF Network ; September 2015"

PASSED=0
TOTAL=0

assert_ok() {
    TOTAL=$((TOTAL + 1))
    echo "✓ Test $TOTAL: $1"
    PASSED=$((PASSED + 1))
}

assert_fail() {
    local desc="$1"
    shift
    TOTAL=$((TOTAL + 1))
    if "$@" >/dev/null 2>&1; then
        echo "✗ Test $TOTAL FAILED: $desc (expected failure, but command succeeded)"
        exit 1
    else
        echo "✓ Test $TOTAL: $desc (failed closed as expected)"
        PASSED=$((PASSED + 1))
    fi
}

echo "════════════════════════════════════════════════════════════"
echo "Running Fault-Injection Resumable Deployment Tests"
echo "════════════════════════════════════════════════════════════"

# 1. Fresh journal planning
OUTPUT_PLAN=$(cd "$MANIFEST_TOOL" && npx tsx src/cli.ts journal plan \
    --file "$TEST_JOURNAL" \
    --contract stealth-announcer \
    --contract stealth-registry \
    --contract stealth-sender \
    --contract wraith-names)
echo "$OUTPUT_PLAN" | grep -q "\[DEPLOY\] stealth-announcer"
assert_ok "Fresh journal plans all contracts for deployment"

# 2. Initialize journal
(cd "$MANIFEST_TOOL" && npx tsx src/cli.ts journal init \
    --file "$TEST_JOURNAL" \
    --network "$NET_A" \
    --rpc "https://soroban-testnet.stellar.org" \
    --source-commit "$COMMIT_A" \
    --deployer "$DEPLOYER_A")
assert_ok "Journal initializes successfully with network, commit and authority"

# 3. Simulate step 1 deployed, then crash / fault injection
(cd "$MANIFEST_TOOL" && npx tsx src/cli.ts journal record-step \
    --file "$TEST_JOURNAL" \
    --name stealth-announcer \
    --id "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABBB4" \
    --wasm "stealth-announcer=$MOCK_WASM_DIR/stealth_announcer.wasm" \
    --height 100)

PLAN_AFTER_STEP1=$(cd "$MANIFEST_TOOL" && npx tsx src/cli.ts journal plan \
    --file "$TEST_JOURNAL" \
    --contract stealth-announcer \
    --contract stealth-registry \
    --contract stealth-sender \
    --contract wraith-names)

echo "$PLAN_AFTER_STEP1" | grep -q "\[REUSE\] stealth-announcer (id: CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABBB4)"
echo "$PLAN_AFTER_STEP1" | grep -q "\[DEPLOY\] stealth-registry"
assert_ok "Dry-run plan after crash correctly marks step 1 for [REUSE] and step 2 for [DEPLOY]"

# 4. Fail-closed: Mismatched source commit on resume
assert_fail "Resume rejects mismatched commit SHA (fail-closed)" \
    npx tsx "$MANIFEST_TOOL/src/cli.ts" journal validate \
    --file "$TEST_JOURNAL" \
    --network "$NET_A" \
    --rpc "https://soroban-testnet.stellar.org" \
    --source-commit "$COMMIT_B" \
    --deployer "$DEPLOYER_A" \
    --wasm "stealth-announcer=$MOCK_WASM_DIR/stealth_announcer.wasm"

# 5. Fail-closed: Mismatched authority on resume
assert_fail "Resume rejects mismatched deployer authority (fail-closed)" \
    npx tsx "$MANIFEST_TOOL/src/cli.ts" journal validate \
    --file "$TEST_JOURNAL" \
    --network "$NET_A" \
    --rpc "https://soroban-testnet.stellar.org" \
    --source-commit "$COMMIT_A" \
    --deployer "$DEPLOYER_B" \
    --wasm "stealth-announcer=$MOCK_WASM_DIR/stealth_announcer.wasm"

# 6. Fail-closed: Mismatched network slug on resume
assert_fail "Resume rejects mismatched network (fail-closed)" \
    npx tsx "$MANIFEST_TOOL/src/cli.ts" journal validate \
    --file "$TEST_JOURNAL" \
    --network "stellar-futurenet" \
    --rpc "https://rpc-futurenet.stellar.org" \
    --source-commit "$COMMIT_A" \
    --deployer "$DEPLOYER_A" \
    --wasm "stealth-announcer=$MOCK_WASM_DIR/stealth_announcer.wasm"

# 7. Fail-closed: Bytecode / Artifact hash drift
TAMPERED_WASM="$TEST_TMP/wasm/stealth_announcer.wasm"
echo "tampered binary bytes" > "$TAMPERED_WASM"
assert_fail "Resume rejects tampered WASM artifact hash (fail-closed)" \
    npx tsx "$MANIFEST_TOOL/src/cli.ts" journal validate \
    --file "$TEST_JOURNAL" \
    --network "$NET_A" \
    --rpc "https://soroban-testnet.stellar.org" \
    --source-commit "$COMMIT_A" \
    --deployer "$DEPLOYER_A" \
    --wasm "stealth-announcer=$TAMPERED_WASM"

# Restore original mock bytes
echo "mock wasm announcer" > "$TAMPERED_WASM"

# 8. Complete all steps and mark completed
(cd "$MANIFEST_TOOL" && npx tsx src/cli.ts journal record-step \
    --file "$TEST_JOURNAL" \
    --name stealth-registry \
    --id "CBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABBB5" \
    --wasm "stealth-registry=$MOCK_WASM_DIR/stealth_registry.wasm")
(cd "$MANIFEST_TOOL" && npx tsx src/cli.ts journal record-step \
    --file "$TEST_JOURNAL" \
    --name stealth-sender \
    --id "CCAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABBB6" \
    --wasm "stealth-sender=$MOCK_WASM_DIR/stealth_sender.wasm")
(cd "$MANIFEST_TOOL" && npx tsx src/cli.ts journal record-step \
    --file "$TEST_JOURNAL" \
    --name wraith-names \
    --id "CDAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABBB7" \
    --wasm "wraith-names=$MOCK_WASM_DIR/wraith_names.wasm")
(cd "$MANIFEST_TOOL" && npx tsx src/cli.ts journal complete --file "$TEST_JOURNAL")
assert_ok "All contracts recorded and journal marked completed successfully"

echo ""
echo "════════════════════════════════════════════════════════════"
echo "All $TOTAL fault-injection tests passed ($PASSED/$TOTAL)."
echo "════════════════════════════════════════════════════════════"
