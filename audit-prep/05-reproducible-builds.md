# Reproducible Build and Attestation Instructions

## Goal

Auditors and users should be able to prove that deployed Stellar Wasm artifacts correspond to the audited source commit.

## Relevant files

| File | Purpose |
| --- | --- |
| `stellar/build/Dockerfile` | Deterministic build container. |
| `stellar/build/build.sh` | Build and attestation generation script used in the container. |
| `stellar/build/rust-toolchain.toml` | Pins Rust `1.81.0`, `wasm32-unknown-unknown`, and minimal profile. |
| `stellar/build/verify.js` | Compares attestation data with deployed contract information. |
| `.github/workflows/stellar-verification.yml` | Scheduled and manual CI verification workflow. |
| `stellar/build/THREAT_MODEL.md` | Existing threat model for the attestation pipeline. |
| `stellar/verification/status.json` | Published verification status. Currently `pending`. |

## Local deterministic build

From the repository root:

```bash
docker build \
  --build-arg COMMIT_HASH="$(git rev-parse HEAD)" \
  -t stellar-attestation-builder \
  -f stellar/build/Dockerfile .

docker create --name builder stellar-attestation-builder
docker start -a builder
docker cp builder:/workspace/contracts/stellar/build/attestation.json ./attestation.json
docker rm builder
```

The resulting `attestation.json` should list the built Wasm outputs and their hashes for the selected commit.

## Deployment verification

Once final contract IDs are available, run verification for each in-scope contract:

```bash
cd stellar
npm ci
node build/verify.js \
  --contract stealth-announcer \
  --id <STEALTH_ANNOUNCER_CONTRACT_ID> \
  --network mainnet \
  --commit <AUDITED_COMMIT_HASH> \
  --attestation ../attestation.json \
  --output verification/results/stealth-announcer.json
```

Repeat for `stealth-registry`, `stealth-sender`, and `wraith-names`.

## CI workflow

The `Stellar Reproducible Build Verification` workflow can be run manually with `network` set to `testnet`, `futurenet`, or `mainnet`.

The workflow checks out the repository, builds the deterministic Docker image, generates `attestation.json`, runs `stellar/build/verify.js` for each contract ID supplied through secrets, writes `stellar/verification/status.json`, and uploads verification artifacts.

## Known limitations

- Attestation proves deployed Wasm matches a commit; it does not prove the source code is safe.
- Compromised CI, OIDC, Docker base images, or RPC endpoints can undermine verification.
- Final mainnet verification is blocked until deployed contract IDs are available.

## Auditor checklist

- Record the exact reviewed commit hash.
- Rebuild the in-scope contracts from a clean checkout.
- Compare local Wasm hashes to CI attestation output.
- After deployment, compare deployed Wasm hashes to the reviewed commit attestation.
- Confirm final `stellar/contract-ids.json` and `stellar/verification/status.json` are updated before mainnet sign-off.
