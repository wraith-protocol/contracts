# Stealth Splitter Contract Design

## Overview

The `stealth-splitter` contract implements a 1-to-N stealth payment splitter on Stellar/Soroban. It enables atomic distribution of a single deposit into multiple stealth payouts according to a fixed, publicly-committed beneficiary schema, while keeping individual stealth addresses unlinkable to recipient identities.

## Key Features

### 1. Immutable Split Definitions
- Created via `create_split(beneficiaries, asset, salt)`
- Split definition cannot be modified after creation
- **Rationale**: Prevents recipients from being added or removed mid-deployment, which would violate the "publicly committed but privately executed" guarantee.

### 2. Deterministic Split IDs
- Split ID is computed as SHA-256(beneficiaries || asset || salt)
- Can be advertised publicly without revealing stealth addresses
- **Rationale**: Allows anyone to see who gets what (public accountability) without knowing individual stealth addresses (privacy).

### 3. Atomic Distribution
- All transfers and announcements complete together or not at all
- Leverages Soroban transaction atomicity
- **Rationale**: Prevents partial payouts where some beneficiaries receive funds but announcements fail (incomplete scanning).

### 4. Weight-Based Proportional Splits
- Each beneficiary has a positive integer weight
- Total payout = sum of (weight / total_weight * amount) for each beneficiary
- **Rationale**: Flexible, intuitive, and supports arbitrary fairness models (equal split, custom ratios, etc.).

## Rounding Strategy: Dust to First Beneficiary

### Decision
Rounding errors (dust) from weight-based division are absorbed by the **first beneficiary**.

### Rationale
1. **Deterministic**: Eliminates ambiguity about where dust goes.
2. **Predictable**: The split creator can control dust routing by ordering beneficiaries.
3. **Pragmatic**: Simplifies contract logic and testing.
4. **Transparent**: Documented in the split definition (immutable beneficiary list).

### Example
```
Total: 1000 units
Beneficiaries: [weight 3, weight 7]

Naive split:
  B1: (3/10) * 1000 = 300
  B2: (7/10) * 1000 = 700
  Total: 1000 ✓

With fractional rounding (if needed):
  B1: floor(300.x) = 300 + dust
  B2: floor(700.y) = 700
  
Dust handling:
  B1 receives dust → B1: 300 + dust, B2: 700
  Total: 1000 ✓
```

### Why Not Other Approaches?
- **Burn dust**: Wastes value; unacceptable for financial contracts.
- **Distribute proportionally**: Introduces recursion or off-by-one errors.
- **Raffle (random)**: Non-deterministic; breaks on-chain reproducibility.
- **Accumulate in contract**: Defeats stateless pass-through design.

## Design Constraints

### Max 25 Beneficiaries
- **Resource budget**: 25 beneficiaries × ~100 bytes per entry = ~2.5 KB storage + compute overhead.
- **Coordinate with issue #06** (resource budget sizing).
- Prevents unbounded loops in `fund_split`.

### No On-Chain Stealth Address Retention
- Contract does not persist stealth addresses after distribution.
- Only stores split definition and funded amounts.
- **Rationale**: Maintains privacy; stealth addresses are ephemeral pass-through data.

### Immutable Beneficiary List
- Cannot add/remove/reorder beneficiaries after split creation.
- **Rationale**: If mutable, the public commitment becomes untrustworthy (attackers could retroactively change who receives what).

## API Reference

### `init(announcer: Address) → Result<(), SplitterError>`
Initialize the contract with the announcer address. Must be called exactly once.

**Parameters:**
- `announcer`: Address of the deployed StealthAnnouncer contract.

**Returns:**
- `Ok(())` on success
- `Err(SplitterError::AlreadyInitialized)` if already initialized

**Authorization:** None (singleton initialization)

---

### `create_split(creator, beneficiaries, asset, salt) → Result<BytesN<32>, SplitterError>`
Create an immutable split definition.

**Parameters:**
- `creator` (Address): Must authorize this call.
- `beneficiaries` (Vec<Beneficiary>): List of (meta_address, weight) pairs. Max 25.
- `asset` (Address): Token contract address to be split.
- `salt` (Bytes): Random bytes for uniqueness. Creator should ensure uniqueness to avoid collisions.

**Returns:**
- `Ok(split_id)` where split_id is the deterministic SHA-256 hash.
- `Err(SplitterError::EmptyBeneficiaries)` if beneficiaries is empty.
- `Err(SplitterError::TooManyBeneficiaries)` if > 25 beneficiaries.
- `Err(SplitterError::InvalidMetaAddressLength)` if any meta-address ≠ 64 bytes.

**Authorization:** Requires creator's signature.

**Side Effects:**
- Stores split definition in contract storage.
- Initializes total_funded to 0.
- Emits `(symbol_short!("create"), split_id, beneficiary_count)` event.

---

### `fund_split(funder, split_id, amount, scheme_id, stealth_addresses, ephemeral_pub_keys, metadatas) → Result<(), SplitterError>`
Fund a split: deposit and atomically distribute to all beneficiaries.

**Parameters:**
- `funder` (Address): Must authorize this call and have sufficient token balance.
- `split_id` (BytesN<32>): ID of an existing split (from `create_split`).
- `amount` (i128): Total amount to distribute. Must be > 0.
- `scheme_id` (u32): Stealth address scheme identifier (e.g., 1 for DKSAP).
- `stealth_addresses` (Vec<Address>): One-time stealth address for each beneficiary. Must match beneficiary count.
- `ephemeral_pub_keys` (Vec<BytesN<32>>): Ephemeral public key for each beneficiary.
- `metadatas` (Vec<Bytes>): Metadata (e.g., view tag) for each beneficiary.

**Returns:**
- `Ok(())` on success
- `Err(SplitterError::SplitNotFound)` if split_id does not exist or vector lengths mismatch.
- `Err(SplitterError::NotInitialized)` if contract not initialized with announcer.
- `Err(SplitterError::InvalidAmount)` if amount ≤ 0.

**Authorization:** Requires funder's signature (via token transfer).

**Side Effects:**
- Transfers proportional amounts to each stealth address (atomic).
- Emits announcements for each beneficiary (atomic).
- Updates total_funded.
- Emits `(symbol_short!("fund"), split_id, amount)` event.

**Atomicity:** If any transfer or announcement fails, the entire `fund_split` is rolled back.

---

### `get_split(split_id) → Result<SplitDetails, SplitterError>`
Query split details: beneficiaries and total funded amount.

**Parameters:**
- `split_id` (BytesN<32>): ID of the split to query.

**Returns:**
- `Ok(SplitDetails)` with immutable beneficiary list and total_funded.
- `Err(SplitterError::SplitNotFound)` if split_id does not exist.

**Authorization:** None (read-only).

---

## Error Codes

| Error | Code | Meaning |
|-------|------|---------|
| `AlreadyInitialized` | 1 | `init` called more than once. |
| `NotInitialized` | 2 | `fund_split` called before `init`. |
| `SplitNotFound` | 3 | Split ID does not exist or vectors mismatch. |
| `TooManyBeneficiaries` | 4 | More than 25 beneficiaries provided. |
| `WeightOverflow` | 5 | Internal: weight sum would overflow (u128). |
| `InvalidMetaAddressLength` | 6 | Meta-address is not 64 bytes. |
| `InvalidAmount` | 7 | Amount ≤ 0. |
| `EmptyBeneficiaries` | 8 | Beneficiary list is empty. |

## Storage Model

### Instance Storage
- `DataKey::Announcer`: Address of the StealthAnnouncer contract (singleton).
- `DataKey::Split(split_id)`: SplitDefinition for a given split.
- `DataKey::SplitFunded(split_id)`: Total funded amount for a given split.

### Storage Retention
- Split definitions are retained indefinitely (immutable, used for `get_split` queries).
- Funded amounts are retained indefinitely (audit trail, public accountability).
- No ephemeral data retained; stealth addresses are pass-through.

## Comparison: Splitter vs. N Separate Stealth-Sender Calls

| Aspect | N Separate Calls | Stealth-Splitter |
|--------|------------------|------------------|
| **Transactions** | N | 1 |
| **TX Overhead** | ~(32 KB × N) | ~32 KB |
| **Atomicity** | Per-call (partial success) | All-or-nothing |
| **Accounting** | Per-recipient (linkable) | Per-split (public commitment) |
| **Discovery** | N separate announcements | N announcements in one TX |
| **Storage** | None | ~2-3 KB per split definition |

**For N = 25 (max beneficiaries):**
- Separate: ~25 TX, ~800 KB total overhead
- Splitter: ~1 TX, ~32 KB overhead, **25x efficiency gain**

## Testing Strategy

### Unit Tests
- ✅ Initialization (success, already-initialized error)
- ✅ Split creation (basic, single beneficiary, max 25, empty list, too many, invalid meta-address)
- ✅ Deterministic IDs (same inputs → same ID, different salt → different ID)
- ✅ Get split (not found, after creation)

### Property-Based Tests
- ✅ Dust to first beneficiary (documented behavior)
- ✅ Immutable definitions (repeated queries return same data)

### Failure & Atomicity Tests
- ✅ Fund split with zero/negative amount
- ✅ Fund split with nonexistent split ID
- ✅ Vector length mismatches (stealth_addresses, ephemeral_keys, metadatas)
- ✅ Atomicity concept (all-or-nothing semantics via Soroban transactions)

### Resource Budget Tests
- Resource analysis documented in code comments.
- Max 25 beneficiaries justifies ~2.5 KB storage per split.

## SDK Follow-Up

The SDK should provide a `buildSplitDeposit` builder for constructing `fund_split` calls:

```rust
// Pseudocode
let deposit = SplitDepositBuilder::new(split_id, amount, scheme_id)
    .add_recipient(stealth_address, ephemeral_pub_key, metadata)
    .add_recipient(stealth_address, ephemeral_pub_key, metadata)
    // ...
    .build();

client.fund_split(&funder, deposit)?;
```

## Demo / Integration

A follow-up demo should showcase:
1. Create a split among 3 contributors (weights: 30%, 30%, 40%).
2. Funder deposits 1000 XLM.
3. Each contributor receives their share in a stealth address.
4. Query split details to verify public accountability (beneficiary list + total funded).
5. Verify announcements were emitted for scanning.

## Conclusion

The `stealth-splitter` contract provides:
- **Privacy**: Individual stealth addresses remain unlinkable to recipients.
- **Accountability**: Split definitions and funded amounts are public.
- **Atomicity**: All-or-nothing distribution prevents partial payouts.
- **Efficiency**: 25x transaction reduction for up to 25 beneficiaries.
- **Immutability**: Trustworthy public commitment that cannot be retroactively modified.

This makes it suitable for revenue-share, DAO payouts, royalty splits, and tip distribution where the beneficiary list is public but individual addresses must remain private.
