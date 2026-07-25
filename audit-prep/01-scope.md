# Audit Scope

## Objective

Obtain an independent third-party security review of the Wraith Protocol Stellar smart contracts before mainnet deployment. The audit should focus on authorization, stealth-address privacy assumptions, event integrity, storage/rent behavior, asset compatibility, upgrade governance, and reproducible build/deployment verification.

## Repository and version under review

| Item | Value |
| --- | --- |
| Repository | `wraith-protocol/contracts` |
| Branch prepared for audit pack | `codex/issue-62-audit-prep-pack` |
| Review target | Current branch head at the time the audit firm receives the pack |
| Stellar SDK | `soroban-sdk = "22.0.0"` from `stellar/Cargo.toml` |
| Rust toolchain for deterministic builds | `1.81.0`, `wasm32-unknown-unknown` target from `stellar/build/rust-toolchain.toml` |

Auditors should record the exact commit hash they review in their report. If code changes after the audit starts, the delta should be reviewed explicitly.

## In-scope contracts

| Contract | Path | Purpose | Expected mainnet governance |
| --- | --- | --- | --- |
| `stealth-announcer` | `stellar/stealth-announcer/src/lib.rs` | Emits stealth-address announcement events. No storage and no access control by design. | Frozen/no upgrade path. |
| `stealth-registry` | `stellar/stealth-registry/src/lib.rs` | Maps registrants and scheme IDs to 64-byte stealth meta-addresses. | Frozen/no upgrade path. |
| `stealth-sender` | `stellar/stealth-sender/src/lib.rs` | Atomically transfers a token and emits a stealth announcement. | 3-of-5 multisig plus 7-day timelock if upgraded; pause policy documented in governance plan. |
| `wraith-names` | `stellar/wraith-names/src/lib.rs` | Maps lowercase `.wraith` names to 64-byte meta-addresses with forward/reverse lookup and on-behalf operations. | 3-of-5 multisig plus 7-day timelock, eventually renounceable. |

## In-scope supporting code

| Area | Paths | Reason |
| --- | --- | --- |
| Workspace dependencies | `stellar/Cargo.toml`, `stellar/Cargo.lock` | Dependency and feature review. |
| Tests | `stellar/*/tests/*.rs`, contract-local `#[cfg(test)]` modules | Security regression and property coverage review. |
| Build verification | `stellar/build/*`, `.github/workflows/stellar-verification.yml`, `stellar/verification/status.json` | Reproducible build and deployment attestation review. |
| Governance/readiness docs | `stellar/GOVERNANCE.md`, `stellar/MAINNET_READINESS.md`, `stellar/UPGRADE_AUTH_TESTS.md` | Operational assumptions and mainnet gates. |
| Asset policy integration | `stellar/wraith-asset-policy/src/lib.rs`, policy references from `stealth-sender` | Relevant to SAC compatibility risks identified in audit #43. |

## Out of scope unless explicitly added

| Area | Rationale |
| --- | --- |
| EVM, Solana, and CKB contracts | This audit pack is for Stellar mainnet readiness. |
| Frontend, SDK UX, and indexer implementation | Review only where contract event semantics create hard requirements for consumers. |
| Wallet key generation and stealth address derivation libraries | Contract audit should validate on-chain assumptions, not external wallet cryptography implementations. |
| Centralized infrastructure, RPC providers, and monitoring operations | Covered only as trust assumptions in the threat model. |
| Economic token design and fee strategy | Not needed for contract safety except resource exhaustion and unsupported asset classes. |

## Security properties to verify

- Announcements are emitted with unambiguous event topics/data for scanners.
- Registry writes require the registrant's authorization and cannot be squatted by third parties.
- Sender transfers and announcements are atomic: no token transfer should persist without the expected announcement when the transaction reverts.
- Sender rejects or gates assets that would break stealth guarantees, especially clawback, revocable, auth-required, and fee-on-transfer variants.
- Names registration/update/release enforces owner authorization and maintains forward/reverse lookup consistency.
- On-behalf names operations are replay-protected and domain-separated.
- Storage choices are safe for expected mainnet usage, including TTL/rent behavior for registry, names, replay keys, and governance state.
- Upgrade authority, timelocks, pause controls, and frozen-contract assumptions match the governance plan.
- Reproducible build outputs can be tied to a reviewed commit and deployed contract IDs.

## Known assumptions for auditors

- Permissionless announcement is intentional.
- Stellar event data is public; privacy comes from stealth-address cryptography and off-chain scanning, not event secrecy.
- Unsupported assets may break privacy or liveness; auditors should validate that the current policy/checking model makes that risk explicit and enforceable.
- Mainnet deployment IDs are currently blank in `stellar/contract-ids.json`; final deployment verification must happen after IDs are assigned.
