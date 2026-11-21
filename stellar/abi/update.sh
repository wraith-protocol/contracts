#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
pnpm exec tsx stellar/scripts/generate-bindings.ts --abi-only
