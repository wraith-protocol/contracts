#!/usr/bin/env bash
# stellar/scripts/validate-deployment.test.sh
#
# Unit tests for validate-deployment.sh
#
# A mock stellar/soroban CLI binary is injected into PATH so that RPC lookups
# are exercised without a live network.  The mock is driven by the environment
# variable MOCK_MISSING_CONTRACT: any contract ID that appears in that
# comma-separated list is treated as non-existent; all others succeed.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
VALIDATE_SCRIPT="$SCRIPT_DIR/validate-deployment.sh"

# Temporary workspace – cleaned up on exit
TEST_DIR=$(mktemp -d)
trap "rm -rf $TEST_DIR" EXIT

RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

TESTS_RUN=0
TESTS_PASSED=0

pass() { printf "${GREEN}✓${NC} %s\n" "$*"; TESTS_PASSED=$((TESTS_PASSED + 1)); }
fail() { printf "${RED}✗${NC} FAIL: %s\n" "$*" >&2; }

# ──────────────────────────────────────────────────────────────────────────────
# Mock stellar CLI
#
# Installed at $TEST_DIR/bin/stellar.
# Behaviour:
#   stellar network add ...       → always succeeds silently
#   stellar network rm  ...       → always succeeds silently
#   stellar contract info wasm-hash --id <ID> ...
#       If $ID is listed in $MOCK_MISSING_CONTRACT  → exit 1 + "not found"
#       Otherwise                                   → exit 0 + fake wasm hash
# ──────────────────────────────────────────────────────────────────────────────

mkdir -p "$TEST_DIR/bin"

cat > "$TEST_DIR/bin/stellar" <<'MOCK'
#!/usr/bin/env bash
# Minimal mock for the stellar CLI used by validate-deployment.sh

# Parse the sub-command path (e.g. "network add", "contract info wasm-hash")
SUBCMD="${1:-} ${2:-} ${3:-}"

case "$SUBCMD" in
    "network add "*|"network rm "*)
        exit 0
        ;;
    "contract info wasm-hash"*)
        # Extract --id value from arguments
        CONTRACT_ID=""
        while [[ $# -gt 0 ]]; do
            if [[ "$1" == "--id" ]]; then
                CONTRACT_ID="$2"
                shift 2
            else
                shift
            fi
        done

        # Check if this ID is in the MOCK_MISSING_CONTRACT list
        MISSING="${MOCK_MISSING_CONTRACT:-}"
        if [ -n "$MISSING" ]; then
            IFS=',' read -ra MISSING_IDS <<< "$MISSING"
            for mid in "${MISSING_IDS[@]}"; do
                if [ "$mid" = "$CONTRACT_ID" ]; then
                    echo "error: contract not found: $CONTRACT_ID" >&2
                    exit 1
                fi
            done
        fi

        # Contract exists – return a fake wasm hash
        echo "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899"
        exit 0
        ;;
    *)
        echo "mock stellar: unhandled command: $*" >&2
        exit 1
        ;;
esac
MOCK

chmod +x "$TEST_DIR/bin/stellar"

# Helper: run validate-deployment.sh with the mock CLI injected
run_validate() {
    local output_file="$1"
    local network="${2:-futurenet}"
    PATH="$TEST_DIR/bin:$PATH" \
        MOCK_MISSING_CONTRACT="${MOCK_MISSING_CONTRACT:-}" \
        RPC_URL="http://mock-rpc.invalid" \
        bash "$VALIDATE_SCRIPT" "$output_file" "$network" 2>&1
}

# ──────────────────────────────────────────────────────────────────────────────
# Shared valid contract IDs (properly formatted Stellar contract addresses)
# ──────────────────────────────────────────────────────────────────────────────

ANNOUNCER_ID="CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
REGISTRY_ID="CBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB"
SENDER_ID="CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC"
NAMES_ID="CDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD"

# Build a standard four-contract output block
make_output() {
    local extra_lines="${1:-}"
    cat <<EOF

═══ Results ═══

  Contract               Contract ID                                             
  ───────                ───────────                                             
  stealth-announcer      $ANNOUNCER_ID
  stealth-registry       $REGISTRY_ID
  stealth-sender         $SENDER_ID
  wraith-names           $NAMES_ID

  Stellar Expert links:
  ├─ Announcer: https://futurenet.stellar.expert/explorer/futurenet/contract/$ANNOUNCER_ID
  ├─ Registry:  https://futurenet.stellar.expert/explorer/futurenet/contract/$REGISTRY_ID
  ├─ Sender:    https://futurenet.stellar.expert/explorer/futurenet/contract/$SENDER_ID
  └─ Names:     https://futurenet.stellar.expert/explorer/futurenet/contract/$NAMES_ID

$extra_lines
EOF
}

# ──────────────────────────────────────────────────────────────────────────────
# Test helpers
# ──────────────────────────────────────────────────────────────────────────────

assert_passes() {
    local label="$1"
    local output_file="$2"
    local network="${3:-futurenet}"
    TESTS_RUN=$((TESTS_RUN + 1))
    if MOCK_MISSING_CONTRACT="${MOCK_MISSING_CONTRACT:-}" run_validate "$output_file" "$network" > /dev/null 2>&1; then
        pass "$label"
    else
        fail "$label (expected pass, got failure)"
    fi
}

assert_fails() {
    local label="$1"
    local output_file="$2"
    local network="${3:-futurenet}"
    TESTS_RUN=$((TESTS_RUN + 1))
    if MOCK_MISSING_CONTRACT="${MOCK_MISSING_CONTRACT:-}" run_validate "$output_file" "$network" > /dev/null 2>&1; then
        fail "$label (expected failure, got pass)"
    else
        pass "$label"
    fi
}

# ──────────────────────────────────────────────────────────────────────────────
# Test 1: All contracts present and all exist on network → passes
# ──────────────────────────────────────────────────────────────────────────────
test_all_contracts_exist() {
    make_output "  ✔ All checks passed. Dry-run complete." \
        > "$TEST_DIR/t1.txt"
    MOCK_MISSING_CONTRACT="" assert_passes \
        "T1: All contracts present and verified on network" \
        "$TEST_DIR/t1.txt"
}

# ──────────────────────────────────────────────────────────────────────────────
# Test 2: All IDs present but one contract missing from network → fails
# ──────────────────────────────────────────────────────────────────────────────
test_contract_missing_from_network() {
    make_output "  ✔ All checks passed. Dry-run complete." \
        > "$TEST_DIR/t2.txt"
    # Mock the announcer as non-existent on the network
    MOCK_MISSING_CONTRACT="$ANNOUNCER_ID" assert_fails \
        "T2: Contract ID present in output but absent from network RPC → fails" \
        "$TEST_DIR/t2.txt"
}

# ──────────────────────────────────────────────────────────────────────────────
# Test 3: Multiple contracts missing from network → fails
# ──────────────────────────────────────────────────────────────────────────────
test_multiple_contracts_missing_from_network() {
    make_output "  ✔ All checks passed. Dry-run complete." \
        > "$TEST_DIR/t3.txt"
    MOCK_MISSING_CONTRACT="$REGISTRY_ID,$SENDER_ID" assert_fails \
        "T3: Multiple contract IDs absent from network RPC → fails" \
        "$TEST_DIR/t3.txt"
}

# ──────────────────────────────────────────────────────────────────────────────
# Test 4: Missing contract in output (not deployed at all) → fails
# ──────────────────────────────────────────────────────────────────────────────
test_missing_contract_in_output() {
    cat > "$TEST_DIR/t4.txt" <<EOF

═══ Results ═══

  Contract               Contract ID                                             
  ───────                ───────────                                             
  stealth-announcer      $ANNOUNCER_ID
  stealth-registry       $REGISTRY_ID
  stealth-sender         $SENDER_ID

  ✔ All checks passed. Dry-run complete.

EOF
    MOCK_MISSING_CONTRACT="" assert_fails \
        "T4: wraith-names absent from output → fails" \
        "$TEST_DIR/t4.txt"
}

# ──────────────────────────────────────────────────────────────────────────────
# Test 5: Invalid contract ID format (does not start with 'C') → fails
# ──────────────────────────────────────────────────────────────────────────────
test_invalid_contract_id_format() {
    cat > "$TEST_DIR/t5.txt" <<EOF

═══ Results ═══

  Contract               Contract ID                                             
  ───────                ───────────                                             
  stealth-announcer      AABCDEFGHIJKLMNOPQRSTUVWXYZ234567ABCDEFGHIJKLMNOPQRS
  stealth-registry       $REGISTRY_ID
  stealth-sender         $SENDER_ID
  wraith-names           $NAMES_ID

  ✔ All checks passed. Dry-run complete.

EOF
    MOCK_MISSING_CONTRACT="" assert_fails \
        "T5: Contract ID not starting with 'C' → fails" \
        "$TEST_DIR/t5.txt"
}

# ──────────────────────────────────────────────────────────────────────────────
# Test 6: Wrong network in stellar.expert URLs → fails
# ──────────────────────────────────────────────────────────────────────────────
test_wrong_network_in_urls() {
    cat > "$TEST_DIR/t6.txt" <<EOF

═══ Results ═══

  Contract               Contract ID                                             
  ───────                ───────────                                             
  stealth-announcer      $ANNOUNCER_ID
  stealth-registry       $REGISTRY_ID
  stealth-sender         $SENDER_ID
  wraith-names           $NAMES_ID

  Stellar Expert links:
  ├─ Announcer: https://testnet.stellar.expert/explorer/testnet/contract/$ANNOUNCER_ID

  ✔ All checks passed. Dry-run complete.

EOF
    MOCK_MISSING_CONTRACT="" assert_fails \
        "T6: stellar.expert URLs reference wrong network → fails" \
        "$TEST_DIR/t6.txt"
}

# ──────────────────────────────────────────────────────────────────────────────
# Test 7: Smoke tests failed → fails
# ──────────────────────────────────────────────────────────────────────────────
test_smoke_failure() {
    cat > "$TEST_DIR/t7.txt" <<EOF

═══ Results ═══

  Contract               Contract ID                                             
  ───────                ───────────                                             
  stealth-announcer      $ANNOUNCER_ID
  stealth-registry       $REGISTRY_ID
  stealth-sender         $SENDER_ID
  wraith-names           $NAMES_ID

  ⚠  Some smoke tests failed. Check output above for details.

EOF
    MOCK_MISSING_CONTRACT="" assert_fails \
        "T7: Smoke tests failed → fails" \
        "$TEST_DIR/t7.txt"
}

# ──────────────────────────────────────────────────────────────────────────────
# Test 8: Missing success marker (deployment incomplete) → fails
# ──────────────────────────────────────────────────────────────────────────────
test_missing_success_marker() {
    make_output "" > "$TEST_DIR/t8.txt"
    # No success or failure marker — deployment ended without a clean exit
    MOCK_MISSING_CONTRACT="" assert_fails \
        "T8: Output lacks success or failure marker → fails" \
        "$TEST_DIR/t8.txt"
}

# ──────────────────────────────────────────────────────────────────────────────
# Run all tests
# ──────────────────────────────────────────────────────────────────────────────

echo "Running validate-deployment.sh tests (with mocked stellar CLI)..."
echo ""

test_all_contracts_exist
test_contract_missing_from_network
test_multiple_contracts_missing_from_network
test_missing_contract_in_output
test_invalid_contract_id_format
test_wrong_network_in_urls
test_smoke_failure
test_missing_success_marker

echo ""
echo "Results: $TESTS_PASSED/$TESTS_RUN tests passed."
echo ""

if [ "$TESTS_PASSED" -ne "$TESTS_RUN" ]; then
    echo "FAIL: $(( TESTS_RUN - TESTS_PASSED )) test(s) failed." >&2
    exit 1
fi

echo "All tests passed."
