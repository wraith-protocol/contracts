# Stellar contract upgrade migration tests

These tests prove **real v0→v1 WASM upgrades** on a single contract ID using Soroban
`register_contract_wasm`, `upload_contract_wasm`, and `update_current_contract_wasm`.

Fixtures under `migration-fixtures/` are frozen snapshots of the initial v0 layout
(instance-scoped names/registry data; sender with announcer-only `init`). Current
crates produce v1 WASM under test.

## Running

```bash
cd stellar
cargo test -p upgrade-migration-tests
```

The `upgrade-migration-tests` crate builds all fixture and production WASM in
`build.rs` before tests run.

## Coverage matrix

| Contract | Production upgrade path | What tests prove |
|---|---|---|
| `stealth-sender` | Timelock + multisig | v0 `init` state survives v1 WASM; v1 `send` and v1-only `init_multisig` work after swap |
| `wraith-names` | Timelock + multisig | Raw v0→v1 swap without storage migration is **incompatible** (instance → persistent); after operational instance→persistent copy, v1 reads succeed and new registrations use v1 schema (`parent: None`); v0 WASM rollback restores instance reads if no v1 writes occurred |
| `stealth-registry` | **Frozen (no upgrade in governance)** | WASM swap in test env shows v0 `instance()` records are invisible to v1 persistent reader; rolling back to v0 WASM restores reads — documents why production uses deploy-new rather than in-place upgrade |

See also [MIGRATION_V0_TO_V1.md](./MIGRATION_V0_TO_V1.md) for operator checklists and indexer SQL.
