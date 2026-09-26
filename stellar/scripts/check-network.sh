#!/usr/bin/env bash
# stellar/scripts/check-network.sh
#
# Preflight validation for a Soroban deployment network.
# Usage: ./check-network.sh <testnet|futurenet|mainnet>
# Exit codes: 0 = all checks passed, 1 = one or more checks failed.

set -uo pipefail

NETWORK="${1:-}"
FAILED=0

declare -A PASSPHRASES=(
  [testnet]="Test SDF Network ; September 2015"
  [futurenet]="Test SDF Future Network ; October 2022"
  [mainnet]="Public Global Stellar Network ; September 2015"
)

declare -A RPC_URLS=(
  [testnet]="https://soroban-testnet.stellar.org"
  [futurenet]="https://rpc-futurenet.stellar.org"
  [mainnet]="https://mainnet.sorobanrpc.com"
)

declare -A FRIENDBOT_URLS=(
  [testnet]="https://friendbot.stellar.org"
  [futurenet]="https://friendbot-futurenet.stellar.org"
)

IDENTITY_NAME="${STELLAR_IDENTITY:-default}"

print_row() {
  printf "%-12s %-30s %-8s\n" "$1" "$2" "$3"
}

fail() {
  print_row "$1" "$2" "FAIL"
  echo "    -> $3" >&2
  FAILED=1
}

pass() {
  print_row "$1" "$2" "OK"
}

if [[ -z "$NETWORK" ]]; then
  echo "Usage: $0 <testnet|futurenet|mainnet>" >&2
  exit 1
fi

if [[ -z "${PASSPHRASES[$NETWORK]+x}" ]]; then
  echo "ERROR: unknown network '$NETWORK' (expected testnet|futurenet|mainnet)" >&2
  exit 1
fi

echo "Preflight checks for network: $NETWORK"
echo "----------------------------------------------"

# 1. Network passphrase is defined
if [[ -n "${PASSPHRASES[$NETWORK]}" ]]; then
  pass "$NETWORK" "passphrase"
else
  fail "$NETWORK" "passphrase" "no passphrase configured for $NETWORK"
fi

# 2. RPC URL reachable
RPC_URL="${RPC_URLS[$NETWORK]}"
if curl -sf -o /dev/null --max-time 5 \
     -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' \
     "$RPC_URL"; then
  pass "$NETWORK" "rpc ($RPC_URL)"
else
  fail "$NETWORK" "rpc ($RPC_URL)" "could not reach RPC endpoint"
fi

# 3. Friendbot reachable (testnet/futurenet only)
if [[ -n "${FRIENDBOT_URLS[$NETWORK]+x}" ]]; then
  FRIENDBOT_URL="${FRIENDBOT_URLS[$NETWORK]}"
  if curl -sf -o /dev/null --max-time 5 "$FRIENDBOT_URL"; then
    pass "$NETWORK" "friendbot"
  else
    fail "$NETWORK" "friendbot" "could not reach friendbot at $FRIENDBOT_URL"
  fi
else
  print_row "$NETWORK" "friendbot" "SKIP"
fi

# 4. Identity exists
if stellar keys address "$IDENTITY_NAME" >/dev/null 2>&1; then
  pass "$NETWORK" "identity ($IDENTITY_NAME)"
else
  fail "$NETWORK" "identity ($IDENTITY_NAME)" \
    "identity '$IDENTITY_NAME' not found - run: stellar keys generate $IDENTITY_NAME"
fi

# 5. Required env vars (extend as deploy.sh actually needs)
REQUIRED_VARS=()
if [[ "$NETWORK" == "mainnet" ]]; then
  REQUIRED_VARS+=("STELLAR_ACCOUNT")
fi
for var in "${REQUIRED_VARS[@]+"${REQUIRED_VARS[@]}"}"; do
  if [[ -n "${!var:-}" ]]; then
    pass "$NETWORK" "env:$var"
  else
    fail "$NETWORK" "env:$var" "required env var $var is not set"
  fi
done

echo "----------------------------------------------"
if [[ "$FAILED" -ne 0 ]]; then
  echo "Preflight FAILED for $NETWORK." >&2
  exit 1
fi

echo "Preflight passed for $NETWORK."
exit 0
