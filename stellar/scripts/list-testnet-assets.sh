#!/usr/bin/env bash
# stellar/scripts/list-testnet-assets.sh
#
# Lists well-known issued assets on Stellar testnet, along with their flag
# profiles and Wraith SAC-compatibility verdict.
#
# Usage:
#   ./stellar/scripts/list-testnet-assets.sh [--json]
#
# Options:
#   --json   Emit newline-delimited JSON instead of a human-readable table.
#
# The script queries the Horizon testnet API for each known asset and prints:
#   - Asset code and issuer
#   - AUTH_REQUIRED / AUTH_REVOCABLE / AUTH_CLAWBACK_ENABLED flag status
#   - Wraith compatibility verdict (SUPPORTED / UNSUPPORTED / BLOCKED)
#
# No credentials required — all queries are public read-only Horizon calls.

set -euo pipefail

HORIZON_TESTNET="https://horizon-testnet.stellar.org"
JSON_MODE=0

for arg in "$@"; do
  [[ "$arg" == "--json" ]] && JSON_MODE=1
done

# ── Known testnet assets ──────────────────────────────────────────────────────
# Format: "CODE:ISSUER_PUBLIC_KEY"
KNOWN_ASSETS=(
  "USDC:GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5"
  "EURC:GDHU6WRG4IEQXM5NZ4BMPKOXHW76MZM4Y2IEMFDVXBSDP6SJY4ITNPP2"
  "AQUA:GBNZILSTVQZ4R7IKQDGHYGY2QXL5QOFJYQMXPKWRRM5PAV7Y4M67AQUA"
  "MOBI:GA6HCMBLTZS5VYYBCATRBRZ3BZJMAFUDKYYF6AH6MVCMGWMRDNSWJPIH"
  "BLND:GDJEHTB7QVKN4BYFCZR7JWKXFCYZSAQM5FXDLBMFBFBWZZPFXNHFMF4"
  "yXLM:GARDNEUQNOU373DOBERTUUNMZVAKJQE4JSKALBMFPFMBLZLA2AVZLWNX"
  "StellarX:GBZX4364PEPQTDICMIQDZ56K4T75QZCR4NBEYKO6PDRJAHZKGUOJPCXB"
)

# ── Helpers ───────────────────────────────────────────────────────────────────

check_deps() {
  for cmd in curl jq; do
    if ! command -v "$cmd" &>/dev/null; then
      echo "Error: '$cmd' is required. Install it and re-run." >&2
      exit 1
    fi
  done
}

# Query Horizon for the issuer account and extract flags.
# Returns a JSON object: {"auth_required":bool,"auth_revocable":bool,"auth_clawback_enabled":bool}
get_flags() {
  local issuer="$1"
  local account
  account=$(curl -sf "${HORIZON_TESTNET}/accounts/${issuer}" 2>/dev/null) || {
    echo '{"auth_required":null,"auth_revocable":null,"auth_clawback_enabled":null}'
    return
  }
  local req rev clawback
  req=$(echo "$account" | jq -r '.flags.auth_required // false')
  rev=$(echo "$account" | jq -r '.flags.auth_revocable // false')
  clawback=$(echo "$account" | jq -r '.flags.auth_clawback_enabled // false')
  printf '{"auth_required":%s,"auth_revocable":%s,"auth_clawback_enabled":%s}' \
    "$req" "$rev" "$clawback"
}

# Determine Wraith compatibility verdict from flag JSON.
verdict() {
  local flags="$1"
  local req rev clawback
  req=$(echo "$flags" | jq -r '.auth_required')
  rev=$(echo "$flags" | jq -r '.auth_revocable')
  clawback=$(echo "$flags" | jq -r '.auth_clawback_enabled')

  if [[ "$req" == "null" ]]; then
    echo "UNKNOWN (account unreachable)"
  elif [[ "$clawback" == "true" ]]; then
    echo "UNSUPPORTED (AUTH_CLAWBACK_ENABLED)"
  elif [[ "$req" == "true" ]]; then
    echo "BLOCKED (AUTH_REQUIRED)"
  elif [[ "$rev" == "true" ]]; then
    echo "UNSUPPORTED (AUTH_REVOCABLE)"
  else
    echo "SUPPORTED"
  fi
}

# ── Main ──────────────────────────────────────────────────────────────────────

check_deps

if [[ $JSON_MODE -eq 0 ]]; then
  printf "%-8s %-58s %-10s %-10s %-10s %s\n" \
    "ASSET" "ISSUER" "REQ" "REVOCABLE" "CLAWBACK" "VERDICT"
  printf '%0.s-' {1..120}; echo
fi

for entry in "${KNOWN_ASSETS[@]}"; do
  code="${entry%%:*}"
  issuer="${entry##*:}"
  flags=$(get_flags "$issuer")
  v=$(verdict "$flags")

  if [[ $JSON_MODE -eq 1 ]]; then
    jq -n \
      --arg code "$code" \
      --arg issuer "$issuer" \
      --argjson flags "$flags" \
      --arg verdict "$v" \
      '{code:$code, issuer:$issuer, flags:$flags, verdict:$verdict}'
  else
    req=$(echo "$flags" | jq -r '.auth_required // "?"')
    rev=$(echo "$flags" | jq -r '.auth_revocable // "?"')
    clawback=$(echo "$flags" | jq -r '.auth_clawback_enabled // "?"')
    printf "%-8s %-58s %-10s %-10s %-10s %s\n" \
      "$code" "$issuer" "$req" "$rev" "$clawback" "$v"
  fi
done
