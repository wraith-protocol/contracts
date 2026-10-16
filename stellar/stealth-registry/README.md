# Stealth Registry Contract (`stealth-registry`)

The `stealth-registry` contract manages the storage and resolution of stealth meta-addresses (`spending_pubkey || viewing_pubkey`) on Soroban.

Error codes are tracked in the [Stellar error catalog](../ERRORS.md#stealth-registry).

## SDK 22 lifetime and event tests

From `stellar/`, run `cargo test --workspace` for the normal workspace suite,
or `cargo test -p stealth-registry --test upgrade_auth` for the registry's
immutability and lifetime tests. The three lifetime/event regression tests
run normally without `--ignored`.

The lifetime fixtures configure ledger TTL bounds before registering the
contract. They advance to renewal thresholds and call the public lookup to
renew both persistent registrations and the contract instance. They check
stored values and the actual TTL extensions across one million ledgers and
ten years of simulated activity. Immutability does not exempt data from
storage rent: these tests model active registrations, not perpetual storage
without renewal. Expiration boundaries are covered separately by
`integration-tests/tests/storage_rent_ttl.rs`.

Event assertions capture `env.events().all()` immediately after each SDK 22
invocation. They check each operation's contract address, topics and payload,
and compare repeated registrations, rather than assuming cumulative events.

## Formal Verification with Kani

Formal verification harnesses are located in `src/proofs/mod.rs` and can be verified using [Kani](https://model-checking.github.io/kani/).

### Running Verification Locally

```bash
cargo kani --package stealth-registry
```

---

## Formally Proven Invariants

### 1. Register-Then-Resolve Roundtrip (`proof_register_then_resolve`)
* **Claim**: For any valid 64-byte payload registered under a `(registrant, scheme_id)` key, resolving that key via `stealth_meta_address_of` immediately returns the exact registered payload.
* **Non-Goals**: Does not verify the cryptographic validity or key quality of the underlying 64-byte payload (e.g., verifying secp256k1 or ed25519 point validity), nor does it verify off-chain RPC node network transport.

### 2. Key Uniqueness / No Double-Registration (`proof_no_duplicate_keys`)
* **Claim**: The persistent storage map maintains strict key uniqueness. No two active registrations in storage share the same key `(Address, u32)`. Any new registration for an existing key safely replaces the previous entry.
* **Non-Goals**: Does not model host-level disk persistence corruption or out-of-memory errors on ledger nodes.

### 3. Expiry Monotonicity (`proof_expiry_monotonicity`)
* **Claim**: Any state-mutating operation (`register_keys`) or read operation (`stealth_meta_address_of`) that extends entry Time-To-Live (TTL) results in an expiry ledger number that is monotonically non-decreasing (`new_expiry >= old_expiry`).
* **Non-Goals**: Does not prevent entry expiration if an entry is left unaccessed past its TTL threshold, nor does it model host ledger clock skew bugs.
