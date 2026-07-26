# Soroban Storage Rent Audit Report

This report presents a thorough analysis of the on-chain storage rent and Time-to-Live (TTL) sustainability for the Wraith stealth address protocol smart contracts on the Stellar network.

---

## 1. Storage Write Enumeration

The protocol manages on-chain storage across three main stateful contracts (excluding `stealth-announcer`, which is pure event-emission and state-free).

| Contract | Operation / Function | Storage Key | Storage Tier | Data Type / Serialized Structure | Estimated Size (Payload + Key) | Entry Size with Overhead | Creator / Submitter | Renewal Responsibility |
|---|---|---|---|---|---|---|---|---|
| **stealth-sender** | `init` | `DataKey::Announcer` | **Instance** | `Address` | ~36 bytes | ~100 bytes (shared) | Deployer | Deployer / Contract Admins |
| **stealth-registry** | `register_keys` | `DataKey::MetaAddress(Address, u16)` | **Persistent** | `Bytes` (64-byte payload) | ~110 bytes | ~174 bytes (separate) | Registrant (User) | User / Client (Permissionless) |
| **wraith-names** | `register` / `update` | `DataKey::Name(BytesN<32>)` | **Persistent** | `NameEntry` (`name: String`, `stealth_meta_address: Bytes`, `owner: Address`) | ~160 bytes | ~224 bytes (separate) | Registrant (User) | User / Client (Permissionless) |
| **wraith-names** | `register` / `update` | `DataKey::Reverse(BytesN<32>)` | **Persistent** | `BytesN<32>` (32-byte name hash) | ~72 bytes | ~136 bytes (separate) | Registrant (User) | User / Client (Permissionless) |

### Key Observations:
1. **Event-Only Transfers**: Transfers and announcements executed via `stealth-sender` and `stealth-announcer` do not write to persistent ledger state. They emit on-chain events that are indexed off-chain. This keeps transaction throughput high and avoids compounding storage rent costs for transfer history.
2. **Key & Value Overhead**: Soroban serializes storage keys and values using XDR. A baseline overhead of approximately 64 bytes is added to each separate ledger entry in the `BucketList` (for metadata, sequence number, and XDR wrappers).

---

## 2. Cost Model at Three Scale Tiers

The annual storage rent is modeled under two configuration settings:
* **High-Fee Model (Protocol 20/21 baseline values)**: `fee_write_1kb` = 2,000,000 Stroops (0.2 XLM) and `persistentRentRateDenominator` = 1,215. This yields an annual rent rate of **~1.0138 XLM per byte**.
* **Low-Fee Model (Protocol 23 optimized values)**: `feeWrite1KB` = 3,500 Stroops (0.00035 XLM) and `persistentRentRateDenominator` = 1,402. This yields an annual rent rate of **~0.001538 XLM per byte**.

### Scale Tier 1: Small / MVP
* **Metrics**: 1,000 names (and reverse mappings), 5,000 registry entries, 50,000 transfers.
* **Persistent Storage Footprint**:
  * Registry Entries: $5,000 \times 176 \text{ bytes} = 880,000 \text{ bytes}$
  * Name Entries: $1,000 \times 224 \text{ bytes} = 224,000 \text{ bytes}$
  * Reverse Lookups: $1,000 \times 136 \text{ bytes} = 136,000 \text{ bytes}$
  * **Total Footprint**: $1,240,000 \text{ bytes} \approx 1.24 \text{ MB}$
* **Annual Rent Costs**:
  * **High-Fee Model (Protocol 20)**: **$1,257,112.00 \text{ XLM}$**
  * **Low-Fee Model (Protocol 23)**: **$1,907.12 \text{ XLM}$**
  * *Transfers*: $50,000 \times 0 \text{ XLM} = 0 \text{ XLM}$ ongoing rent.

### Scale Tier 2: Medium / Target
* **Metrics**: 10,000 names, 100,000 registry entries, 1,000,000 transfers.
* **Persistent Storage Footprint**:
  * Registry Entries: $100,000 \times 176 \text{ bytes} = 17,600,000 \text{ bytes}$
  * Name Entries: $10,000 \times 224 \text{ bytes} = 2,240,000 \text{ bytes}$
  * Reverse Lookups: $10,000 \times 136 \text{ bytes} = 1,360,000 \text{ bytes}$
  * **Total Footprint**: $21,200,000 \text{ bytes} \approx 21.2 \text{ MB}$
* **Annual Rent Costs**:
  * **High-Fee Model (Protocol 20)**: **$21,492,560.00 \text{ XLM}$**
  * **Low-Fee Model (Protocol 23)**: **$32,605.60 \text{ XLM}$**
  * *Transfers*: $1,000,000 \times 0 \text{ XLM} = 0 \text{ XLM}$ ongoing rent.

### Scale Tier 3: Large / Mass Adoption
* **Metrics**: 100,000 names, 1,000,000 registry entries, 10,000,000 transfers.
* **Persistent Storage Footprint**:
  * Registry Entries: $1,000,000 \times 176 \text{ bytes} = 176,000,000 \text{ bytes}$
  * Name Entries: $100,000 \times 224 \text{ bytes} = 22,400,000 \text{ bytes}$
  * Reverse Lookups: $100,000 \times 136 \text{ bytes} = 13,600,000 \text{ bytes}$
  * **Total Footprint**: $212,000,000 \text{ bytes} \approx 212 \text{ MB}$
* **Annual Rent Costs**:
  * **High-Fee Model (Protocol 20)**: **$214,925,600.00 \text{ XLM}$**
  * **Low-Fee Model (Protocol 23)**: **$326,056.00 \text{ XLM}$**
  * *Transfers*: $10,000,000 \times 0 \text{ XLM} = 0 \text{ XLM}$ ongoing rent.

---

## 3. Core Recommendations & Architectural Changes

### 1. Move User Data from Instance to Persistent Storage
* **Problem**: The current contracts store all registry entries and name registrations in `instance` storage (`env.storage().instance().set()`). This serializes all user data inside the single Contract Instance ledger entry. Since ledger entries are strictly capped at 64 KB, the contract will fail to accept new registrations after a few dozen entries, leading to immediate denial-of-service.
* **Solution**: Transition all mapping entries (`MetaAddress`, `Name`, and `Reverse` lookup) to `persistent` storage (`env.storage().persistent()`). This moves each registration into its own separate ledger entry, allowing infinite scaling subject only to standard fee limits.

### 2. Implement Active TTL Management
* **Problem**: Currently, no contract manages TTL, putting the code and active state at risk of expiration and archival (after ~5.7 hours on default network configurations if inactive).
* **Solution**: Implement `extend_ttl` during contract calls:
  * **Registry & Name Entries (Persistent)**: Set target TTL to **~30 days** (518,400 ledgers) with a threshold of **~1 day** (17,280 ledgers) to prevent frequent, micro-renewal overhead and transaction bloat.
  * **Contract Code/Instance (Instance)**: Set target TTL to **~30 days** (518,400 ledgers) with a threshold of **~1 day** (17,280 ledgers) on every write and read path.

### 3. Maintain Permissionless Renewal
* The protocol relies on the permissionless nature of TTL extensions: any wallet, relayer, or user client can bump the TTL of any entry.
* Client-side applications (dApps and wallets) should automatically check registry/name TTLs and append `ExtendFootprintTTLOp` where appropriate during interactions to keep entries live.
