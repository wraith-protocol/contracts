# Audit Scope Definition

## In-Scope Contracts

This audit covers the Wraith Protocol smart contracts deployed on the Stellar network using the Soroban smart contract platform.

### Core Contracts (MUST AUDIT - Priority 1)

| Contract | Version | LOC | Complexity | Blast Radius |
|---|---|---|---|---|
| `stealth-announcer` | v1.0.0 | ~100 | Low | Low |
| `stealth-registry` | v1.0.0 | ~150 | Low | Medium |
| `stealth-sender` | v1.0.0 | ~200 | Medium | **High** |
| `wraith-names` | v1.0.0 | ~400 | High | Medium |

**Total Core:** ~850 LOC

### Optional Contracts (SHOULD AUDIT - Priority 2)

| Contract | Version | LOC | Complexity | Blast Radius |
|---|---|---|---|---|
| `stealth-splitter` | v1.0.0 | ~300 | Medium | **High** |
| `stealth-batch-sender` | v1.0.0 | ~150 | Medium | **High** |

**Total Optional:** ~450 LOC

### Supporting Libraries (MAY AUDIT - Priority 3)

| Library | Version | LOC | Purpose |
|---|---|---|---|
| `wraith-metrics` | v1.0.0 | ~100 | Event emission for metrics |
| `shared/pausable` | v1.0.0 | ~50 | Pause mechanism utilities |

**Total Libraries:** ~150 LOC

**Grand Total:** ~1,450 LOC across all contracts and libraries

## In-Scope Version

**Git Commit:** [TBD - Will be tagged for audit]  
**Git Tag:** `stellar-audit-v1.0.0`  
**Branch:** `main` (frozen at tag)

All auditors must review the **exact same commit hash** to ensure consistency.

## Contract Descriptions

### 1. stealth-announcer

**Purpose:** Emits stealth address announcement events for recipient scanning.

**Key Functions:**
- `announce(scheme_id, stealth_address, ephemeral_pub_key, metadata)` - Emit announcement event

**Storage:** None (pure event emitter)

**Access Control:** None (permissionless)

**Upgrade Authority:** None (frozen/immutable)

**Blast Radius:** Low
- No assets held
- No user data stored
- Cannot be upgraded
- Events are append-only

**Critical Security Properties:**
- Event emission must be atomic
- Metadata must be preserved exactly
- No censorship possible

---

### 2. stealth-registry

**Purpose:** Registry mapping addresses to stealth meta-addresses (64-byte public key pairs).

**Key Functions:**
- `register_keys(registrant, scheme_id, stealth_meta_address)` - Register keys
- `remove_keys(registrant, scheme_id)` - Remove keys
- `stealth_meta_address_of(registrant, scheme_id)` - Query keys

**Storage:** Persistent (per-user, per-scheme)

**Access Control:** User-only (registrant must authorize)

**Upgrade Authority:** None (frozen/immutable)

**Blast Radius:** Medium
- No assets held
- User privacy data stored
- Cannot be upgraded
- User sovereignty enforced

**Critical Security Properties:**
- Only user can modify their own keys
- No admin can censor or alter registrations
- 64-byte length validation
- TTL management for rent sustainability

---

### 3. stealth-sender

**Purpose:** Atomic "transfer asset + emit announcement" operation for stealth payments.

**Key Functions:**
- `init(announcer)` - Initialize with announcer address
- `send(sender, token, amount, scheme_id, stealth_address, ephemeral_pub_key, metadata)` - Send payment
- `batch_send(...)` - Batch version

**Storage:** Instance (announcer address only)

**Access Control:** Sender authorization required

**Upgrade Authority:** Admin (timelock + multisig)

**Blast Radius:** **HIGH**
- Handles user assets (native XLM + issued tokens)
- Integrates with Stellar Asset Contracts (SAC)
- Atomic failure = no partial transfers

**Critical Security Properties:**
- Atomicity: transfer + announcement or revert
- No reentrancy (Soroban guarantees)
- Auth propagation correct
- No fund loss scenarios
- Init one-shot semantics

**Known Risks:**
- Malicious token contracts (documented, caller must validate)
- Init re-initialization (medium severity, operational concern)

---

### 4. wraith-names

**Purpose:** Human-readable name → stealth meta-address resolution with guardian recovery.

**Key Functions:**
- `register(owner, name, stealth_meta_address)` - Register name
- `update(owner, name, new_meta_address)` - Update address
- `release(owner, name)` - Release name
- `resolve(name)` - Query address
- `name_of(meta_address)` - Reverse lookup
- `set_guardians(name, guardians, threshold)` - Configure guardians
- `propose_recovery(guardian, name, new_owner, new_meta_address)` - Propose recovery
- `approve_recovery(guardian, name)` - Approve recovery
- `cancel_recovery(name)` - Cancel recovery

**Storage:** Persistent (names, reverse lookups, guardian configs, recovery proposals)

**Access Control:** 
- Name operations: Owner authorization
- Recovery operations: Guardian authorization

**Upgrade Authority:** Admin (timelock + multisig, eventually renounceable)

**Blast Radius:** Medium
- No assets held
- Name ownership disputes possible
- Social recovery complexity

**Critical Security Properties:**
- Name uniqueness enforced
- Guardian threshold (N-of-M) honored
- Recovery delay (timelock) enforced
- Signature validation for on-behalf operations
- Replay protection

**Known Risks:**
- Guardian collusion (mitigated by threshold)
- Recovery delay window (7 days for review)

---

### 5. stealth-splitter (Optional)

**Purpose:** Split stealth payments across multiple recipients with predefined shares.

**Key Functions:**
- `create_split(creator, recipients, shares)` - Define split
- `deposit(split_id, sender, token, scheme_id, ephemeral_pub_keys, metadatas)` - Execute split

**Storage:** Persistent (split definitions, funded amounts)

**Access Control:** Creator defines split, anyone can fund

**Upgrade Authority:** Admin (timelock + multisig)

**Blast Radius:** **HIGH**
- Handles user assets
- Complex distribution logic

**Critical Security Properties:**
- Share calculations correct (no rounding errors)
- Atomicity of distribution
- Immutable split definitions

---

### 6. stealth-batch-sender (Optional)

**Purpose:** Optimized batch operations for multiple stealth payments.

**Key Functions:**
- `batch_send_native(recipients, amounts, scheme_id, ephemeral_pub_keys, metadatas)`
- `batch_send_token(token, recipients, amounts, ...)`

**Storage:** Instance (announcer address)

**Access Control:** Sender authorization

**Upgrade Authority:** Admin (timelock + multisig)

**Blast Radius:** **HIGH**
- Handles user assets
- Batch atomicity critical

**Critical Security Properties:**
- Batch atomicity (all or nothing)
- Length matching (recipients, amounts, keys, metadata)
- Gas efficiency vs safety tradeoffs

---

## Out of Scope

### Explicitly Excluded

1. **EVM Contracts** (Solidity)
   - Separate audit planned
   - Different trust model

2. **Solana Contracts** (Rust/Anchor)
   - Separate audit planned
   - Different runtime environment

3. **CKB Contracts** (Rust/RISC-V)
   - Experimental
   - Not planned for mainnet launch

4. **Off-Chain Components**
   - TypeScript SDK
   - Indexers
   - Wallets
   - Client libraries

5. **Stellar Network**
   - Core consensus
   - Soroban runtime
   - Cryptographic primitives (assumed correct)

6. **Testing Infrastructure**
   - Test harnesses
   - Mock contracts
   - CI/CD pipelines

### Assumptions

The audit assumes the following are **correct and secure**:

1. **Soroban SDK** (`soroban-sdk` crate)
   - Memory safety
   - Type safety
   - Authorization framework
   - Storage primitives
   - Cryptographic functions

2. **Stellar Asset Contracts (SAC)**
   - Transfer semantics
   - Balance tracking
   - Authorization propagation

3. **Soroban Runtime**
   - Deterministic execution
   - No reentrancy
   - Atomic transaction semantics
   - Gas metering

4. **Cryptographic Primitives**
   - SHA-256
   - Ed25519
   - Secp256k1
   - ECDH

### Known Limitations (Not Security Issues)

1. **TTL Management**
   - Contracts require periodic TTL extension
   - Storage rent must be paid by users
   - See: [stellar/STORAGE_RENT.md](../stellar/STORAGE_RENT.md)

2. **Governance Trade-offs**
   - Frozen contracts cannot be upgraded (by design)
   - Upgradeable contracts have admin control (mitigated by timelock + multisig)
   - See: [stellar/GOVERNANCE.md](../stellar/GOVERNANCE.md)

3. **Gas Optimization**
   - Contracts prioritize safety over gas efficiency
   - Batch operations have size limits
   - See: [stellar/PERF.md](../stellar/PERF.md)

## Audit Focus Areas

### Priority 1: Critical Security

1. **Fund Safety** (stealth-sender, stealth-splitter, stealth-batch-sender)
   - Can funds be stolen?
   - Can funds be locked?
   - Can transfers be front-run or reordered?

2. **Atomicity** (all asset-handling contracts)
   - Are operations truly atomic?
   - Can partial state be committed?
   - What happens if subcontracts fail?

3. **Authorization** (all contracts)
   - Is auth required where needed?
   - Can auth be bypassed?
   - Is auth context propagated correctly?

4. **Upgrade Safety** (upgradeable contracts)
   - Can non-admin upgrade?
   - Is state preserved post-upgrade?
   - Is timelock enforced?
   - Is multisig threshold honored?

### Priority 2: Functional Correctness

1. **Storage Integrity**
   - Correct data structures
   - No key collisions
   - TTL management correct

2. **Event Emission**
   - All events emitted
   - Event data complete and correct
   - No sensitive data leaked

3. **Input Validation**
   - All inputs validated
   - Edge cases handled
   - Error messages clear

### Priority 3: Code Quality

1. **Gas Optimization**
   - Unnecessary computations
   - Storage access patterns
   - Batch operation efficiency

2. **Code Clarity**
   - Documentation complete
   - Logic easy to follow
   - Test coverage adequate

## Success Criteria

The audit is successful if:

1. **All critical security properties verified**
2. **No Critical or High severity findings** (or remediated)
3. **Medium findings acceptable with mitigation plans**
4. **Reproducible build verified**
5. **Test coverage reviewed and deemed adequate**
6. **Code quality meets mainnet standards**

## FAQ

**Q: Why are some contracts frozen and others upgradeable?**  
A: Per [GOVERNANCE.md](../stellar/GOVERNANCE.md), simple, foundational contracts (announcer, registry) are frozen for maximum trust. Complex contracts (sender, names) are upgradeable to allow bug fixes, with safeguards (timelock, multisig).

**Q: What is the deployment timeline?**  
A: Audit → Remediation → Testnet deployment → 2 week soak period → Mainnet deployment. See [DEPLOYMENT_MANIFEST.md](./DEPLOYMENT_MANIFEST.md).

**Q: What if critical issues are found?**  
A: Pause mainnet launch, remediate, re-audit affected areas, redeploy testnet, repeat soak period.

**Q: How do I reproduce the build?**  
A: See [REPRODUCIBLE_BUILD.md](./REPRODUCIBLE_BUILD.md) for exact toolchain versions and build commands.

**Q: Where are test vectors?**  
A: See [TEST_COVERAGE.md](./TEST_COVERAGE.md) and `stellar/*/tests/*.rs` files.

**Q: What about prior audits?**  
A: See [INTERNAL_AUDITS.md](./INTERNAL_AUDITS.md) for summaries of internal audits and known issues.

---

**Last Updated:** 2026-06-26  
**Document Version:** 1.0.0  
**Audit Coordinator:** [TBD]
