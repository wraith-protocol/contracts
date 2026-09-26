# Threat Model for Reproducible Build Attestation

This document outlines the security guarantees, limitations, and assumptions of the reproducible build attestation pipeline for the Wraith Names Stellar contracts.

## Goal
The primary goal of this pipeline is to provide mathematical proof to users that the compiled Wasm contract deployed on the Stellar network corresponds exactly to a specific commit of the open-source code in this repository.

## Guarantees (What it provides)

1. **Deterministic Builds**: Given the same commit hash, base Docker image, and pinned toolchain (Rust, `stellar-cli`), the compilation will always yield a byte-for-byte identical optimized Wasm file with the same SHA256 hash.
2. **Attestation Integrity**: The `attestation.json` file is generated inside an isolated Docker container and signed using Sigstore/Cosign. This proves that the GitHub Actions runner built the code and produced those hashes.
3. **Public Verifiability**: Any user can run `pnpm verify:stellar-deployment` locally. This script fetches the deployed Wasm hash from the Stellar RPC and compares it against the published `attestation.json`. 

## Limitations (What it does NOT provide)

1. **Protection against malicious source code**: This pipeline does not guarantee that the source code is safe, audited, or free of vulnerabilities. It only guarantees that *what is deployed* matches *what is in the repository*.
2. **Protection against compromised signing keys/OIDC**: If the maintainers' GitHub Actions OIDC token or Cosign keys are compromised, an attacker could publish a malicious `attestation.json` and sign it. Users verifying the signature would believe it is legitimate. (Keyless signing via GitHub Actions OIDC mitigates long-lived key compromise, but an attacker with write access to the CI workflow could still forge attestations).
3. **Protection against compromised Docker base images**: If the `debian:bookworm-slim` base image or the Rust toolchain downloaded during the build is compromised by an advanced attacker to inject a backdoor deterministically, the attestation would cover the backdoored binary. We mitigate this by pinning the SHA256 of the base image.
4. **Protection against GitHub/RPC spoofing**: If a user runs the verification script on a compromised network where DNS resolves `api.github.com` or `soroban-rpc.mainnet.stellar.org` to malicious endpoints, the script could be fed a fake attestation or fake on-chain Wasm hash.

## Assumptions
- The `stellar-cli`'s `contract optimize` command is deterministic (this is a known property of Soroban's optimization tool).
- Rust's `cargo build` is deterministic when dependencies are locked (via `Cargo.lock`) and no build scripts (`build.rs`) leak timestamps or local paths into the final binary.
- The user verifying the deployment trusts the Sigstore transparency log and the GitHub repository owner.

## Verification Steps
1. Find the deployed contract ID on Stellar mainnet.
2. Run the verification script:
   `pnpm verify:stellar-deployment --contract wraith-names --id <CONTRACT_ID> --network mainnet --commit <COMMIT_HASH>`
3. The script will fetch the on-chain Wasm hash and the signed attestation from GitHub, ensuring they match.
