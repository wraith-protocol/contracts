#!/usr/bin/env bash
# stellar/scripts/setup-multisig.sh
#
# Idempotent Stellar multisig signer setup.
#
# Usage:
#   ./setup-multisig.sh [OPTIONS]
#
# Required options:
#   --network     <testnet|futurenet|mainnet>
#   --account     <G... source account to configure>
#   --signers     <comma-separated G... signer addresses>
#   --threshold   <M-of-N integer threshold>
#
# Optional flags:
#   --dry-run     Print resulting account config without submitting
#   --identity    <stellar identity name for signing; default: $STELLAR_IDENTITY or "default">
#   --log-file    <path to append audit log; default: multisig-setup.log>
#
# Examples:
#   # 3-of-5 dry-run on futurenet
#   ./setup-multisig.sh \
#     --network futurenet \
#     --account GABC... \
#     --signers "GA1...,GA2...,GA3...,GA4...,GA5..." \
#     --threshold 3 \
#     --dry-run
#
#   # Live setup
#   ./setup-multisig.sh \
#     --network futurenet \
#     --account GABC... \
#     --signers "GA1...,GA2...,GA3...,GA4...,GA5..." \
#     --threshold 3 \
#     --identity deployer

set -euo pipefail

# ---------- defaults --------------------------------------------------------
NETWORK=""
ACCOUNT=""
SIGNERS_RAW=""
THRESHOLD=""
DRY_RUN=0
IDENTITY="${STELLAR_IDENTITY:-default}"
LOG_FILE="multisig-setup.log"

# ---------- helpers ---------------------------------------------------------
log() {
  local level="$1"; shift
  local msg="[$(date -u +"%Y-%m-%dT%H:%M:%SZ")] [$level] $*"
  echo "$msg"
  echo "$msg" >> "$LOG_FILE"
}

die() {
  local msg="[$(date -u +"%Y-%m-%dT%H:%M:%SZ")] [ERROR] $*"
  echo "$msg" >&2
  echo "$msg" >> "$LOG_FILE"
  exit 1
}

# Validates that a string is a Stellar G... public key (56 chars, base32).
is_valid_gaddress() {
  local addr="$1"
  [[ "$addr" =~ ^G[A-Z2-7]{55}$ ]]
}

# ---------- arg parsing -----------------------------------------------------
while [[ $# -gt 0 ]]; do
  case "$1" in
    --network)   NETWORK="$2";   shift 2 ;;
    --account)   ACCOUNT="$2";   shift 2 ;;
    --signers)   SIGNERS_RAW="$2"; shift 2 ;;
    --threshold) THRESHOLD="$2"; shift 2 ;;
    --dry-run)   DRY_RUN=1;      shift   ;;
    --identity)  IDENTITY="$2";  shift 2 ;;
    --log-file)  LOG_FILE="$2";  shift 2 ;;
    *) die "Unknown argument: $1" ;;
  esac
done

# ---------- validate required args ------------------------------------------
[[ -n "$NETWORK" ]]    || die "--network is required"
[[ -n "$ACCOUNT" ]]    || die "--account is required"
[[ -n "$SIGNERS_RAW" ]] || die "--signers is required"
[[ -n "$THRESHOLD" ]]  || die "--threshold is required"

declare -A PASSPHRASES=(
  [testnet]="Test SDF Network ; September 2015"
  [futurenet]="Test SDF Future Network ; October 2022"
  [mainnet]="Public Global Stellar Network ; September 2015"
)
[[ -n "${PASSPHRASES[$NETWORK]+x}" ]] || die "Unknown network '$NETWORK'"

# ---------- parse + validate signers ----------------------------------------
IFS=',' read -ra SIGNERS <<< "$SIGNERS_RAW"
declare -a VALID_SIGNERS=()

for s in "${SIGNERS[@]}"; do
  s="${s// /}"  # strip spaces
  if is_valid_gaddress "$s"; then
    VALID_SIGNERS+=("$s")
  else
    die "Invalid signer address: '$s' (must be a 56-char G... Stellar public key)"
  fi
done

N="${#VALID_SIGNERS[@]}"
[[ "$N" -gt 0 ]] || die "At least one signer is required"

if ! [[ "$THRESHOLD" =~ ^[0-9]+$ ]] || [[ "$THRESHOLD" -lt 1 ]]; then
  die "--threshold must be a positive integer"
fi

if [[ "$THRESHOLD" -gt "$N" ]]; then
  die "Threshold ($THRESHOLD) cannot exceed number of signers ($N)"
fi

# Validate the target account address
is_valid_gaddress "$ACCOUNT" || die "Invalid --account address: '$ACCOUNT'"

# ---------- compute signer weight -------------------------------------------
# Each signer gets weight 1. Master weight set to 0 (disabled).
# low/med/high thresholds all set to THRESHOLD for uniform policy.
WEIGHT_PER_SIGNER=1
MASTER_WEIGHT=0

# ---------- print plan -------------------------------------------------------
log "INFO" "=== Multisig Setup Plan ==="
log "INFO" "Network:       $NETWORK"
log "INFO" "Account:       $ACCOUNT"
log "INFO" "Signers (N=$N):"
for s in "${VALID_SIGNERS[@]}"; do
  log "INFO" "  $s (weight=$WEIGHT_PER_SIGNER)"
done
log "INFO" "Threshold:     $THRESHOLD-of-$N"
log "INFO" "Master weight: $MASTER_WEIGHT (disabled)"
log "INFO" "Low threshold: $THRESHOLD"
log "INFO" "Med threshold: $THRESHOLD"
log "INFO" "High threshold: $THRESHOLD"

if [[ "$DRY_RUN" -eq 1 ]]; then
  log "INFO" "[DRY-RUN] No transactions submitted."
  exit 0
fi

# ---------- check stellar CLI available -------------------------------------
command -v stellar >/dev/null 2>&1 || die "'stellar' CLI not found in PATH"

# ---------- build set_options transaction -----------------------------------
# Stellar CLI: set_options per signer, then master weight + thresholds.
# We compose a single XDR envelope with all operations for atomicity.

log "INFO" "Building set_options transaction..."

# Temp file for XDR ops
TMPXDR=$(mktemp /tmp/multisig-setup.XXXXXX.xdr)
trap 'rm -f "$TMPXDR"' EXIT

# Build signer add operations (one per signer)
SIGNER_ARGS=()
for s in "${VALID_SIGNERS[@]}"; do
  SIGNER_ARGS+=(--signer "${s}:${WEIGHT_PER_SIGNER}")
done

stellar tx new set-options \
  --network "$NETWORK" \
  --source "$ACCOUNT" \
  --build-only \
  --master-weight "$MASTER_WEIGHT" \
  --low-threshold "$THRESHOLD" \
  --med-threshold "$THRESHOLD" \
  --high-threshold "$THRESHOLD" \
  "${SIGNER_ARGS[@]}" \
  --xdr-out "$TMPXDR" \
  2>&1 | while IFS= read -r line; do log "STELLAR" "$line"; done

log "INFO" "Transaction built. Signing with identity '$IDENTITY'..."

stellar tx sign \
  --network "$NETWORK" \
  --sign-with-key "$IDENTITY" \
  --xdr-in "$TMPXDR" \
  --xdr-out "$TMPXDR" \
  2>&1 | while IFS= read -r line; do log "STELLAR" "$line"; done

log "INFO" "Submitting transaction..."

stellar tx submit \
  --network "$NETWORK" \
  --xdr-in "$TMPXDR" \
  2>&1 | while IFS= read -r line; do log "STELLAR" "$line"; done

# ---------- verification step -----------------------------------------------
log "INFO" "Verifying account configuration..."

ACCOUNT_JSON=$(stellar account show \
  --network "$NETWORK" \
  --account "$ACCOUNT" \
  --json 2>/dev/null) || die "Failed to read back account state"

# Check master weight is 0
ACTUAL_MASTER=$(echo "$ACCOUNT_JSON" | grep -o '"master_weight":[^,}]*' | grep -o '[0-9]*' || echo "")
if [[ "$ACTUAL_MASTER" == "$MASTER_WEIGHT" ]]; then
  log "INFO" "✓ Master weight confirmed: $ACTUAL_MASTER"
else
  log "WARN" "Master weight mismatch: expected $MASTER_WEIGHT, got $ACTUAL_MASTER"
fi

# Check threshold
ACTUAL_HIGH=$(echo "$ACCOUNT_JSON" | grep -o '"high_threshold":[^,}]*' | grep -o '[0-9]*' || echo "")
if [[ "$ACTUAL_HIGH" == "$THRESHOLD" ]]; then
  log "INFO" "✓ High threshold confirmed: $ACTUAL_HIGH"
else
  log "WARN" "High threshold mismatch: expected $THRESHOLD, got $ACTUAL_HIGH"
fi

# Count signers
SIGNER_COUNT=$(echo "$ACCOUNT_JSON" | grep -o '"key":"G[A-Z2-7]*"' | wc -l || echo "0")
log "INFO" "Signers found on-chain: $SIGNER_COUNT (expected $N)"

log "INFO" "=== Setup complete. Audit log: $LOG_FILE ==="
