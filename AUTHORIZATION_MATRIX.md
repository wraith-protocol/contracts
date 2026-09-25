# Cross-Chain Authorization Matrix

**Version**: 1.0.0  
**Date**: 2026-09-25  
**Scope**: Wraith Protocol smart contracts across Stellar, EVM, Solana, and CKB

---

## Overview

This matrix documents the authorization model for every administrative operation across all 19 Wraith Protocol contracts:

| Chain | Contracts |
|-------|-----------|
| Stellar | 9 (stealth-announcer, stealth-registry, stealth-sender, stealth-batch-sender, stealth-splitter, stealth-vault, wraith-names, wraith-asset-policy, governance) |
| EVM | 5 (ERC5564Announcer, ERC6538Registry, WraithSender, WraithNames, WraithWithdrawer) |
| Solana | 3 (wraith-announcer, wraith-sender, wraith-names) |
| CKB | 2 (wraith-stealth-lock, wraith-names-type) |

It covers:
- **Expected caller** for each operation
- **Error codes** for unauthorized, replay, zero-address, and stale-authority cases
- **Governance layer** (multisig, timelock, token voting)
- **Chain-specific enforcement mechanisms**

**Note**: The Stellar contracts (sender, batch-sender, vault, names) have no upgrade function and no `renounce_admin`. The `init_multisig` function has no caller authorization check (anyone can call it once). The upgrade authority table reflects the current source code.

---

## Legend

| Symbol | Meaning |
|--------|---------|
| ✅ | Operation exists and is authorized |
| ❌ | Operation does not exist (frozen/immutable contract) |
| 🔐 | Requires authorization (caller must sign) |
| 👑 | Admin-only (stored admin address) |
| 🗳️ | Token-weighted governance vote |
| 👥 | Multisig threshold (M-of-N) |
| ⏱️ | Timelock delay enforced |
| 🔄 | Signer rotation proposal flow |
| — | Not applicable / no storage |

---

## Stellar Contracts

### 1. `stealth-announcer` (Frozen)

| Operation | Expected Caller | Auth Required | Errors | Governance |
|-----------|----------------|---------------|--------|------------|
| `announce` | Anyone (permissionless) | — | `scheme_id != 2` → panic<br>`metadata empty` → panic | None (immutable) |
| `init` | — | — | — | ❌ Not implemented |
| `pause` | — | — | — | ❌ Not implemented |
| `upgrade` | — | — | — | ❌ Frozen |

**Notes**: Stateless event emitter. No storage, no admin, no upgrade path. `scheme_id` must be `2` (Stellar v2).

---

### 2. `stealth-registry` (Frozen)

| Operation | Expected Caller | Auth Required | Errors | Governance |
|-----------|----------------|---------------|--------|------------|
| `register_keys` | Registrant | 🔐 `registrant.require_auth()` | `InvalidMetaAddressLength` (≠64 bytes) | None (immutable) |
| `remove_keys` | Registrant | 🔐 `registrant.require_auth()` | `NotRegistered` | None (immutable) |
| `stealth_meta_address_of` | Anyone (view) | — | `NotRegistered` | None |
| `init` | — | — | — | ❌ Not implemented |
| `pause` | — | — | — | ❌ Not implemented |
| `upgrade` | — | — | — | ❌ Frozen |

**Notes**: User-sovereign key storage. No admin can censor or alter registrations.

---

### 3. `stealth-sender` (Admin-pausable, Multisig Rotation)

| Operation | Expected Caller | Auth Required | Errors | Governance |
|-----------|----------------|---------------|--------|------------|
| `init` | Deployer | — | `AlreadyInitialized` | One-time |
| `send` | Sender | 🔐 `sender.require_auth()` | `NotInitialized`, `Paused`, `TokenNotAllowed`, `LengthMismatch` | — |
| `batch_send` | Sender | 🔐 `sender.require_auth()` | `NotInitialized`, `Paused`, `TokenNotAllowed`, `LengthMismatch` | — |
| `withdraw_many` | Withdrawer | 🔐 `withdrawer.require_auth()` | `BatchTooLarge` | — (not paused) |
| `pause` | **Admin** | 👑 `caller == admin` + 🔐 | `unauthorized: only admin can pause` (panic) | 👑 Admin |
| `unpause` | **Admin** | 👑 `caller == admin` + 🔐 | `unauthorized: only admin can unpause` (panic) | 👑 Admin |
| `is_paused` | Anyone (view) | — | — | — |
| `init_multisig` | Anyone | — | `MultisigAlreadyInitialized`, `InvalidThreshold` | One-time setup |
| `propose_rotate_signers` | **Current Signer** | 👥 `caller ∈ signers` + 🔐 | `MultisigNotInitialized`, `NotSigner`, `RotationAlreadyPending`, `InvalidThreshold` | 👥 + ⏱️ 7d |
| `approve_rotate_signers` | **Current Signer** | 👥 `caller ∈ signers` + 🔐 | `NoPendingRotation`, `AlreadyApprovedRotation`, `NotSigner` | 👥 |
| `execute_rotate_signers` | **Current Signer** | 👥 `caller ∈ signers` + 🔐 | `NoPendingRotation`, `QuorumNotMet`, `TimelockNotElapsed`, `NotSigner` | 👥 + ⏱️ 7d |
| `cancel_rotate_signers` | **Current Signer** | 👥 `caller ∈ signers` + 🔐 | `NoPendingRotation`, `NotSigner` | 👥 |

**Paused Operations**: `send`, `batch_send`  
**Available During Pause**: `withdraw_many` (exits always allowed)

---

### 4. `stealth-batch-sender` (Admin-pausable, Multisig Rotation)

| Operation | Expected Caller | Auth Required | Errors | Governance |
|-----------|----------------|---------------|--------|------------|
| `init` | Deployer | — | `AlreadyInitialized` | One-time |
| `batch_send` | Sender | 🔐 `sender.require_auth()` | `NotInitialized`, `Paused`, `TokenNotAllowed`, `LengthMismatch` | — |
| `pause` | **Admin** | 👑 `caller == admin` + 🔐 | `unauthorized` (panic) | 👑 Admin |
| `unpause` | **Admin** | 👑 `caller == admin` + 🔐 | `unauthorized` (panic) | 👑 Admin |
| `init_multisig` | Anyone | — | `MultisigAlreadyInitialized`, `InvalidThreshold` | One-time setup |
| `propose_rotate_signers` | **Current Signer** | 👥 `caller ∈ signers` + 🔐 | `MultisigNotInitialized`, `NotSigner`, `RotationAlreadyPending`, `InvalidThreshold` | 👥 + ⏱️ 7d |
| `approve_rotate_signers` | **Current Signer** | 👥 `caller ∈ signers` + 🔐 | `NoPendingRotation`, `AlreadyApprovedRotation`, `NotSigner` | 👥 |
| `execute_rotate_signers` | **Current Signer** | 👥 `caller ∈ signers` + 🔐 | `NoPendingRotation`, `QuorumNotMet`, `TimelockNotElapsed`, `NotSigner` | 👥 + ⏱️ 7d |
| `cancel_rotate_signers` | **Current Signer** | 👥 `caller ∈ signers` + 🔐 | `NoPendingRotation`, `NotSigner` | 👥 |

**Paused Operations**: `batch_send`  
**No withdrawal path** — users exit via `stealth-sender`

---

### 5. `stealth-vault` (Admin-pausable)

| Operation | Expected Caller | Auth Required | Errors | Governance |
|-----------|----------------|---------------|--------|------------|
| `init` | Deployer | — | `AlreadyInitialized` | One-time |
| `deposit` | Sender | 🔐 `sender.require_auth()` | `NotInitialized`, `Paused`, `InvalidWindow` | — |
| `claim` | Recipient | 🔐 `recipient.require_auth()` | `DepositNotFound`, `NotYetUnlocked`, `WrongRecipient` | — (not paused) |
| `refund` | Depositor | 🔐 `sender.require_auth()` | `DepositNotFound`, `NotYetRefundable` | — (not paused) |
| `refund_permissionless` | Anyone | 🔐 `caller.require_auth()` | `DepositNotFound`, `NotYetPermissionless` | — (not paused) |
| `pause` | **Admin** | 👑 `caller == admin` + 🔐 | `unauthorized: only admin can pause` (panic) | 👑 Admin |
| `unpause` | **Admin** | 👑 `caller == admin` + 🔐 | `unauthorized: only admin can unpause` (panic) | 👑 Admin |
| `set_grace_period` | **Admin** | 👑 `caller == admin` + 🔐 | `InvalidGracePeriod` (zero), `unauthorized` (panic) | 👑 Admin |
| `get_deposit` | Anyone (view) | — | `DepositNotFound` | — |
| `is_paused` | Anyone (view) | — | — | — |
| `admin` | Anyone (view) | — | `NotInitialized` | — |
| `grace_period` | Anyone (view) | — | — | — |

**Paused Operations**: `deposit`  
**Available During Pause**: `claim`, `refund`, `refund_permissionless`, all views

---

### 6. `stealth-splitter` (Immutable)

| Operation | Expected Caller | Auth Required | Errors | Governance |
|-----------|----------------|---------------|--------|------------|
| `init` | Deployer | — | `AlreadyInitialized` | One-time |
| `create_split` | Creator | 🔐 `creator.require_auth()` | `AlreadyInitialized`, `EmptyBeneficiaries`, `TooManyBeneficiaries`, `InvalidMetaAddressLength` | None (immutable) |
| `fund_split` | Funder | 🔐 `funder.require_auth()` | `NotInitialized`, `SplitNotFound`, `InvalidAmount`, vector length mismatch | None (immutable) |
| `get_split` | Anyone (view) | — | `SplitNotFound` | None |

**Notes**: Immutable split definition storage. Max 25 beneficiaries with 64-byte meta-addresses each. Atomic fund + distribute + announce. No admin, no pause, no upgrade, no multisig.

---

### 7. `wraith-names` (Admin-pausable, Multisig Rotation + Auction Admin)

| Operation | Expected Caller | Auth Required | Errors | Governance |
|-----------|----------------|---------------|--------|------------|
| `init` | Deployer | — | Idempotent (first admin sticks) | One-time |
| `register` | Owner | 🔐 `owner.require_auth()` | `Paused`, `NameTaken`, `NameTooShort/Long`, `InvalidNameCharacter`, `InvalidMetaAddress`, `NameTooDeep`, `PremiumAuctionRequired` | — |
| `register_on_behalf` | Relayer | 🔐 (off-chain sig by owner) | `Paused`, `SignatureExpired`, `SignatureReplay`, `InvalidSigner`, + above | — |
| `update` | Owner / Parent Owner | 🔐 `owner.require_auth()` | `Paused`, `NameNotFound`, `NotOwner`, `InvalidMetaAddress` | — |
| `update_on_behalf` | Relayer | 🔐 (off-chain sig by owner) | `Paused`, `SignatureExpired`, `SignatureReplay`, `InvalidSigner`, + above | — |
| `release` | Owner / Parent Owner | 🔐 `owner.require_auth()` | `Paused`, `NameNotFound`, `NotOwner` | — |
| `release_on_behalf` | Relayer | 🔐 (off-chain sig by owner) | `Paused`, `SignatureExpired`, `SignatureReplay`, `InvalidSigner`, + above | — |
| `bulk_register` | Owner | 🔐 `owner.require_auth()` | `Paused`, `BulkLimitExceeded`, + above | — |
| `bulk_renew` | Anyone | — | `BulkLimitExceeded`, `InvalidExtendLedger`, `NameNotFound` | — |
| `extend_name_ttl` | Anyone | — | `Paused`, `InvalidExtendLedger`, `NameNotFound` | — |
| `resolve` | Anyone (view) | — | `NameNotFound` | — |
| `name_of` | Anyone (view) | — | `NameNotFound` | — |
| `pause` | **Admin** | 👑 `caller == admin` + 🔐 | `unauthorized: only admin can pause` (panic) | 👑 Admin |
| `unpause` | **Admin** | 👑 `caller == admin` + 🔐 | `unauthorized: only admin can unpause` (panic) | 👑 Admin |
| `is_paused` | Anyone (view) | — | — | — |
| `init_multisig` | Anyone | — | `MultisigAlreadyInitialized`, `InvalidThreshold` | One-time setup |
| `propose_rotate_signers` | **Current Signer** | 👥 `caller ∈ signers` + 🔐 | `MultisigNotInitialized`, `NotSigner`, `RotationAlreadyPending`, `InvalidThreshold` | 👥 + ⏱️ 7d |
| `approve_rotate_signers` | **Current Signer** | 👥 `caller ∈ signers` + 🔐 | `NoPendingRotation`, `AlreadyApprovedRotation`, `NotSigner` | 👥 |
| `execute_rotate_signers` | **Current Signer** | 👥 `caller ∈ signers` + 🔐 | `NoPendingRotation`, `QuorumNotMet`, `TimelockNotElapsed`, `NotSigner` | 👥 + ⏱️ 7d |
| `cancel_rotate_signers` | **Current Signer** | 👥 `caller ∈ signers` + 🔐 | `NoPendingRotation`, `NotSigner` | 👥 |
| `propose_rotate_auction_admin` | **Current Signer** | 👥 `caller ∈ signers` + 🔐 | `MultisigNotInitialized`, `AuctionsNotInitialized`, `NotSigner`, `RotationAlreadyPending` | 👥 + ⏱️ 7d |
| `approve_rotate_auction_admin` | **Current Signer** | 👥 `caller ∈ signers` + 🔐 | `NoPendingRotation`, `AlreadyApprovedRotation`, `NotSigner` | 👥 |
| `execute_rotate_auction_admin` | **Current Signer** | 👥 `caller ∈ signers` + 🔐 | `NoPendingRotation`, `QuorumNotMet`, `TimelockNotElapsed`, `AuctionInProgress`, `NotSigner` | 👥 + ⏱️ 7d |
| `cancel_rotate_auction_admin` | **Current Signer** | 👥 `caller ∈ signers` + 🔐 | `NoPendingRotation`, `NotSigner` | 👥 |
| `init_auctions` | Admin | 🔐 `admin.require_auth()` | `AuctionError` variants | One-time |
| `start_auction` | Anyone | — | `NotPremiumName`, `NameAlreadyRegistered` | — |
| `commit_bid` | Bidder | 🔐 `bidder.require_auth()` | `AuctionError` variants | — |
| `reveal_bid` | Bidder | 🔐 `bidder.require_auth()` | `AuctionError` variants | — |
| `settle_auction` | Anyone (permissionless) | — | `AuctionError` variants | — |
| `withdraw_bid` | Bidder | 🔐 `bidder.require_auth()` | `AuctionError` variants | — |
| `claim_name` | Winner | 🔐 `winner.require_auth()` | `AuctionError` variants | — |

**Paused Operations**: `register`, `register_on_behalf`, `update`, `update_on_behalf`, `release`, `release_on_behalf`, `bulk_register`, `extend_name_ttl`  
**Available During Pause**: `resolve`, `name_of`, `bulk_renew`, all auction ops

---

### 8. `wraith-asset-policy` (Admin-controlled allowlist)

| Operation | Expected Caller | Auth Required | Errors | Governance |
|-----------|----------------|---------------|--------|------------|
| `init` | Deployer | — | `already initialized` (panic) | One-time |
| `add_asset` | **Admin** | 👑 `admin.require_auth()` | `not initialized` (panic) | 👑 Admin |
| `remove_asset` | **Admin** | 👑 `admin.require_auth()` | `not initialized` (panic) | 👑 Admin |
| `check_asset` | Anyone (view) | — | — | — |

**Notes**: Used by `stealth-sender` for token allowlisting. Admin is single address (multisig recommended).

---

### 9. `governance` (On-Chain Token Voting PoC)

| Operation | Expected Caller | Auth Required | Errors | Governance |
|-----------|----------------|---------------|--------|------------|
| `init` | Deployer | — | `AlreadyInitialized` | One-time |
| `get_config` | Anyone (view) | — | `NotInitialized` | — |
| `propose` | Any token holder | 🔐 `proposer.require_auth()` | `NotInitialized` | 🗳️ Token-weighted |
| `get_proposal` | Anyone (view) | — | `ProposalNotFound` | — |
| `vote` | Token holder | 🔐 `voter.require_auth()` | `ProposalNotFound`, `AlreadyExecuted`, `AlreadyCancelled`, `VotingNotActive`, `AlreadyVoted`, `NoVotingPower` | 🗳️ Token-weighted |
| `get_vote` | Anyone (view) | — | `ProposalNotFound` | — |
| `execute` | Anyone | — | `ProposalNotFound`, `AlreadyExecuted`, `AlreadyCancelled`, `VotingStillActive`, `TimelockNotElapsed`, `QuorumNotMet`, `ProposalDefeated`, `ExecutionFailed` | 🗳️ + ⏱️ |
| `cancel` | **Admin** (during voting) / Anyone (post-voting, no quorum) | 👑 `admin.require_auth()` (during voting) | `ProposalNotFound`, `AlreadyExecuted`, `AlreadyCancelled` | 👑 / 🗳️ |

**Notes**: Proof of Concept — not production ready. Admin is single address. Production would replace admin with governance-controlled multisig.

---

## EVM Contracts

### 1. `ERC5564Announcer` (Immutable)

| Operation | Expected Caller | Auth Required | Errors | Governance |
|-----------|----------------|---------------|--------|------------|
| `announce` | Anyone | — | None (emits event) | None (immutable) |

**Notes**: No storage, no access control, no admin. Singleton.

---

### 2. `ERC6538Registry` (Immutable)

| Operation | Expected Caller | Auth Required | Errors | Governance |
|-----------|----------------|---------------|--------|------------|
| `registerKeys` | Registrant | 🔐 `msg.sender` | None (overwrites) | None (immutable) |
| `registerKeysOnBehalf` | Relayer | 🔐 EIP-712 sig by registrant | `ERC6538Registry__InvalidSignature`, replay (nonce) | None |
| `incrementNonce` | Registrant | 🔐 `msg.sender` | None | None |
| `stealthMetaAddressOf` | Anyone (view) | — | Returns empty bytes | None |
| `nonceOf` | Anyone (view) | — | Returns 0 | None |
| `DOMAIN_SEPARATOR` | Anyone (view) | — | Returns domain separator | None |

**Notes**: User-sovereign. No admin, no pause, no upgrade. EIP-712 delegation with nonce replay protection.

---

### 3. `WraithSender` (Immutable)

| Operation | Expected Caller | Auth Required | Errors | Governance |
|-----------|----------------|---------------|--------|------------|
| `sendETH` | Sender | 🔐 `msg.sender` (payable) | `InsufficientValue` (batch), `LengthMismatch` (batch) | None (immutable) |
| `sendERC20` | Sender | 🔐 `msg.sender` (approval required) | `LengthMismatch`, `TipTransferFailed` | None (immutable) |
| `batchSendETH` | Sender | 🔐 `msg.sender` (payable) | `LengthMismatch`, `InsufficientValue` | None (immutable) |
| `batchSendERC20` | Sender | 🔐 `msg.sender` (approval required) | `LengthMismatch`, `TipTransferFailed` | None (immutable) |

**Notes**: Constructor takes `announcer` address. No admin, no pause, no upgrade. ReentrancyGuard on all functions.

---

### 4. `WraithNames` (Owner-based via spending key)

| Operation | Expected Caller | Auth Required | Errors | Governance |
|-----------|----------------|---------------|--------|------------|
| `register` | Owner | 🔐 Signature by spending key | `NameTaken`, `NameTooShort/Long`, `InvalidNameCharacter`, `InvalidMetaAddress`, `InvalidSignature` | Owner (spending key) |
| `registerOnBehalf` | Relayer | 🔐 Signature by spending key + nonce | `NameTaken`, `InvalidSignature`, replay (nonce) | Owner (spending key) |
| `update` | Current Owner | 🔐 Signature by current spending key | `NameNotFound`, `NotOwner`, `InvalidMetaAddress`, `InvalidSignature` | Owner (spending key) |
| `release` | Current Owner | 🔐 Signature by current spending key | `NameNotFound`, `NotOwner`, `InvalidSignature` | Owner (spending key) |
| `resolve` | Anyone (view) | — | Returns empty bytes | — |
| `nameOf` | Anyone (view) | — | Returns empty string | — |

**Notes**: Ownership proven via secp256k1 signature from spending private key (first 33 bytes of meta-address). No admin, no pause, no upgrade. Nonce replay protection on `registerOnBehalf`.

---

### 5. `WraithWithdrawer` (EIP-7702 Delegation Target)

| Operation | Expected Caller | Auth Required | Errors | Governance |
|-----------|----------------|---------------|--------|------------|
| `withdrawETH` | Sponsor (msg.sender) | 🔐 `msg.sender` (EIP-7702 delegated) | `InsufficientBalance`, `FeeTooHigh`, `TransferFailed` | None (immutable) |
| `withdrawERC20` | Sponsor (msg.sender) | 🔐 `msg.sender` (EIP-7702 delegated) | `InsufficientBalance`, `FeeTooHigh`, `TransferFailed` | None (immutable) |
| `withdrawETHDirect` | Self (stealth EOA) | 🔐 `msg.sender` | `InsufficientBalance`, `TransferFailed` | None (immutable) |
| `withdrawERC20Direct` | Self (stealth EOA) | 🔐 `msg.sender` | `InsufficientBalance`, `TransferFailed` | None (immutable) |

**Notes**: Designed as EIP-7702 delegation target. When delegated, `address(this)` = stealth EOA. Sponsor pays gas, receives `sponsorFee` from withdrawal. No admin, no pause, no upgrade.

---

## Solana Contracts

### 1. `wraith-announcer` (Immutable)

| Operation | Expected Caller | Auth Required | Errors | Governance |
|-----------|----------------|---------------|--------|------------|
| `announce` | Caller (Signer) | 🔐 `Signer<'info>` | None (emits event) | None (immutable) |

**Notes**: Stateless. No storage, no admin, no upgrade.

---

### 2. `wraith-sender` (Immutable)

| Operation | Expected Caller | Auth Required | Errors | Governance |
|-----------|----------------|---------------|--------|------------|
| `send_sol` | Sender (Signer) | 🔐 `Signer<'info>` | SystemProgram errors | None (immutable) |
| `send_spl` | Sender (Signer) | 🔐 `Signer<'info>` | TokenProgram errors | None (immutable) |

**Notes**: Atomic transfer + announcement. No admin, no pause, no upgrade. Sender must sign.

---

### 3. `wraith-names` (Owner-based via PDA)

| Operation | Expected Caller | Auth Required | Errors | Governance |
|-----------|----------------|---------------|--------|------------|
| `register` | Owner (Signer, payer) | 🔐 `Signer<'info>` | `InvalidNameLength`, `InvalidNameCharacter` | Owner (wallet) |
| `update` | Owner (Signer) | 🔐 `Signer<'info>` + owner check | `NotOwner` | Owner (wallet) |
| `release` | Owner (Signer) | 🔐 `Signer<'info>` + owner check | `NotOwner` | Owner (wallet) |
| `resolve` | Anyone (view) | — | Returns meta-address | — |

**Notes**: PDA derived from `["name", name]`. Owner = wallet that paid for registration. No admin, no pause, no upgrade.

---

## CKB Contracts

### 1. `wraith-stealth-lock` (Lock Script)

| Operation | Expected Caller | Auth Required | Errors | Governance |
|-----------|----------------|---------------|--------|------------|
| `program_entry` (unlock) | Spender | 🔐 secp256k1 sig via ckb-auth | `SignatureLengthNotEnough`, `ArgsLengthNotEnough`, `AuthError` | None (immutable script) |

**Notes**: Validates secp256k1 signature against `blake160(stealth_pubkey)` in lock args. Delegates to on-chain `ckb-auth` cell. No admin, no upgrade (script hash fixed).

---

### 2. `wraith-names-type` (Type Script)

| Operation | Expected Caller | Auth Required | Errors | Governance |
|-----------|----------------|---------------|--------|------------|
| `program_entry` (validate) | Tx builder | — (cell lock proves ownership) | `InvalidDataLength` (≠66 bytes) | None (immutable script) |

**Notes**: Validates name cell data = 66 bytes (spending_pubkey + viewing_pubkey). Create/Update require 66-byte output; Destroy (release) always allowed. Ownership via cell's lock script. No admin, no upgrade.

---

## Cross-Chain Comparison

### Initialization

| Chain | Contract | Init Auth | Re-init Protection |
|-------|----------|-----------|-------------------|
| Stellar | stealth-sender | Deployer (arg) | `AlreadyInitialized` |
| Stellar | stealth-splitter | Deployer (arg) | `AlreadyInitialized` |
| Stellar | stealth-vault | Deployer (arg) | `AlreadyInitialized` |
| Stellar | wraith-names | Deployer (arg) | Idempotent (first wins) |
| Stellar | wraith-asset-policy | Deployer (arg) | `already initialized` (panic) |
| Stellar | governance | Deployer (arg) | `AlreadyInitialized` |
| EVM | WraithSender | Constructor arg | Constructor (once) |
| EVM | ERC6538Registry | Constructor | Constructor (once) |
| EVM | WraithNames | Constructor | Constructor (once) |
| EVM | WraithWithdrawer | Constructor | Constructor (once) |
| Solana | wraith-names | First `register` (PDA init) | PDA `init` constraint |
| CKB | (scripts) | Deploy (cell creation) | Type/lock hash fixed |

---

### Pause/Unpause

| Chain | Contract | Pause Auth | Unpause Auth | Paused Ops | Exit Ops During Pause |
|-------|----------|------------|--------------|------------|----------------------|
| Stellar | stealth-sender | Admin (👑 + 🔐) | Admin (👑 + 🔐) | send, batch_send | withdraw_many |
| Stellar | stealth-batch-sender | Admin (👑 + 🔐) | Admin (👑 + 🔐) | batch_send | (none) |
| Stellar | stealth-vault | Admin (👑 + 🔐) | Admin (👑 + 🔐) | deposit | claim, refund, refund_permissionless |
| Stellar | wraith-names | Admin (👑 + 🔐) | Admin (👑 + 🔐) | register, update, release, bulk_*, extend_ttl | resolve, name_of, bulk_renew, auctions |
| EVM | (none) | — | — | — | — |
| Solana | (none) | — | — | — | — |
| CKB | (none) | — | — | — | — |

---

### Signer Rotation (Multisig Governance)

| Chain | Contract | Propose | Approve | Execute | Cancel | Timelock | Threshold |
|-------|----------|---------|---------|---------|--------|----------|-----------|
| Stellar | stealth-sender | Signer (👥) | Signer (👥) | Signer (👥 + ⏱️) | Signer (👥) | 7 days | Configurable |
| Stellar | stealth-batch-sender | Signer (👥) | Signer (👥) | Signer (👥 + ⏱️) | Signer (👥) | 7 days | Configurable |
| Stellar | wraith-names | Signer (👥) | Signer (👥) | Signer (👥 + ⏱️) | Signer (👥) | 7 days | Configurable |
| EVM | (none) | — | — | — | — | — | — |
| Solana | (none) | — | — | — | — | — | — |
| CKB | (none) | — | — | — | — | — | — |

---

### Upgrade Authority

| Chain | Contract | Upgradeable | Admin | Multisig | Timelock | Renunciation Path |
|-------|----------|-------------|-------|----------|----------|-------------------|
| Stellar | stealth-announcer | ❌ Frozen | — | — | — | N/A |
| Stellar | stealth-registry | ❌ Frozen | — | — | — | N/A |
| Stellar | stealth-sender | ❌ No upgrade function | 👑 Admin (pause only) | 👥 Rotation only | — | N/A |
| Stellar | stealth-batch-sender | ❌ No upgrade function | 👑 Admin (pause only) | 👥 Rotation only | — | N/A |
| Stellar | stealth-splitter | ❌ Immutable | — | — | — | N/A |
| Stellar | stealth-vault | ❌ No upgrade function | 👑 Admin (pause only) | — | — | N/A |
| Stellar | wraith-names | ❌ No upgrade function | 👑 Admin (pause only) | 👥 Rotation only | — | N/A |
| Stellar | wraith-asset-policy | ❌ (admin only) | 👑 Admin | — | — | ❌ |
| Stellar | governance | ❌ PoC | 👑 Admin | — | ⏱️ (vote) | — |
| EVM | All | ❌ Immutable | — | — | — | N/A |
| Solana | All | ❌ Immutable | — | — | — | N/A |
| CKB | All | ❌ Immutable | — | — | — | N/A |

---

## Error Code Reference

### Stellar Errors (by Contract)

#### `stealth-sender` (`SenderError`)
| Code | Error | Condition |
|------|-------|-----------|
| 1 | AlreadyInitialized | `init` called twice |
| 2 | NotInitialized | Operation before `init` |
| 3 | LengthMismatch | Batch vector lengths differ |
| 4 | TokenNotAllowed | Asset policy rejects token |
| 5 | InvalidFeeConfig | Fee > 50bps or fee>0 w/o recipient |
| 16 | Paused | Contract paused |
| 6 | BatchTooLarge | >30 withdrawals |
| 7 | MultisigNotInitialized | `init_multisig` not called |
| 8 | MultisigAlreadyInitialized | `init_multisig` called twice |
| 9 | NotSigner | Caller not in signer set |
| 10 | InvalidThreshold | Threshold 0 or > signers |
| 11 | RotationAlreadyPending | Proposal exists |
| 12 | NoPendingRotation | No proposal to act on |
| 13 | AlreadyApprovedRotation | Signer already approved |
| 14 | QuorumNotMet | Approvals < threshold |
| 15 | TimelockNotElapsed | <7 days since propose |

---

#### `stealth-splitter` (`SplitterError`)

| Code | Error | Condition |
|------|-------|-----------|
| 1 | AlreadyInitialized | `init` called twice |
| 2 | NotInitialized | Operation before `init` |
| 3 | SplitNotFound | Split ID not in storage |
| 4 | TooManyBeneficiaries | >25 beneficiaries |
| 5 | WeightOverflow | Weight sum overflow |
| 6 | InvalidMetaAddressLength | Meta-address ≈64 bytes |
| 7 | InvalidAmount | Amount ≤0 |
| 8 | EmptyBeneficiaries | No beneficiaries provided |

---

#### `stealth-vault` (`VaultError`)
| Code | Error | Condition |
|------|-------|-----------|
| 1 | AlreadyInitialized | `init` called twice |
| 2 | NotInitialized | Operation before `init` |
| 3 | InvalidWindow | `refund_after ≤ unlock_ledger + grace` |
| 4 | DepositNotFound | Deposit ID not in storage |
| 5 | NotYetUnlocked | `ledger < unlock_ledger` |
| 6 | NotYetRefundable | `ledger < refund_after` |
| 7 | WrongRecipient | Claimer ≠ recipient |
| 8 | Paused | Contract paused |
| 9 | NotYetPermissionless | `ledger < refund_after + grace` |
| 10 | InvalidGracePeriod | `grace_period == 0` |

#### `wraith-names` (`NamesError`)
| Code | Error | Condition |
|------|-------|-----------|
| 1 | NameTaken | Name already registered |
| 2 | NameTooShort | <3 chars |
| 3 | NameTooLong | >32 chars |
| 4 | InvalidNameCharacter | Not lowercase alnum or hyphen |
| 5 | InvalidMetaAddress | ≠64 bytes |
| 6 | NameNotFound | Name not registered |
| 7 | NotOwner | Caller ≠ owner (or parent owner) |
| 8 | SignatureExpired | `ledger ≥ expiry` |
| 9 | SignatureReplay | Replay key already used |
| 10 | InvalidSigner | Not Ed25519 account |
| 11 | NotGuardian | Caller not guardian |
| 12 | NoProposal | Recovery proposal missing |
| 13 | ProposalAlreadyExists | Recovery proposal exists |
| 14 | AlreadyApproved | Guardian already approved |
| 15 | DelayNotElapsed | Recovery delay not met |
| 16 | ThresholdNotMet | Guardian approvals < threshold |
| 17 | TooManyGuardians | >max guardians |
| 18 | InvalidThreshold | Guardian threshold invalid |
| 19 | InvalidExtendLedger | `extend_to ≤ current_ledger` |
| 20 | ParentNotFound | Subdomain parent missing |
| 32 | Paused | Contract paused |
| 21 | MultisigNotInitialized | `init_multisig` not called |
| 22 | MultisigAlreadyInitialized | `init_multisig` called twice |
| 23 | NotSigner | Caller not in signer set |
| 24 | RotationAlreadyPending | Proposal exists |
| 25 | NoPendingRotation | No proposal to act on |
| 26 | AlreadyApprovedRotation | Signer already approved |
| 27 | QuorumNotMet | Approvals < threshold |
| 28 | TimelockNotElapsed | <7 days since propose |
| 29 | NameTooDeep | Subdomain depth >1 |
| 30 | BulkLimitExceeded | >20 names in bulk |
| 31 | PremiumAuctionRequired | ≤4 chars during premium window |

#### `governance` (`GovernanceError`)
| Code | Error | Condition |
|------|-------|-----------|
| 1 | AlreadyInitialized | `init` called twice |
| 2 | NotInitialized | Operation before `init` |
| 3 | NotAdmin | Caller ≠ admin |
| 4 | ProposalNotFound | Proposal ID missing |
| 5 | AlreadyVoted | Voter already voted |
| 6 | VotingNotActive | `ledger < start` or `ledger > end` |
| 7 | VotingStillActive | `execute` before voting ends |
| 8 | QuorumNotMet | `total_votes < quorum` |
| 9 | ProposalDefeated | `for_votes ≤ against_votes` |
| 10 | TimelockNotElapsed | `ledger < end + timelock` |
| 11 | AlreadyExecuted | Proposal executed |
| 12 | AlreadyCancelled | Proposal cancelled |
| 13 | ExecutionFailed | Target call reverted |
| 14 | NoVotingPower | `balance == 0` |

---

### EVM Errors

#### `ERC6538Registry`
| Error | Condition |
|-------|-----------|
| `ERC6538Registry__InvalidSignature` | `ecrecover` fails & EIP-1271 fails |

#### `WraithSender`
| Error | Condition |
|-------|-----------|
| `LengthMismatch` | Batch array lengths differ |
| `InsufficientValue` | `msg.value ≠ sum(amounts)` (batch ETH) |
| `TipTransferFailed` | ETH tip transfer reverted |

#### `WraithNames`
| Error | Condition |
|-------|-----------|
| `NameTaken` | Name already registered |
| `NameTooShort` | <3 chars |
| `NameTooLong` | >32 chars |
| `InvalidNameCharacter` | Not lowercase alnum |
| `InvalidMetaAddress` | ≠66 bytes |
| `InvalidSignature` | `ecrecover` ≠ spending address |
| `NameNotFound` | Name not registered |
| `NotOwner` | `ecrecover` ≠ entry.spendingAddress |

#### `WraithWithdrawer`
| Error | Condition |
|-------|-----------|
| `InsufficientBalance` | `balance == 0` |
| `FeeTooHigh` | `sponsorFee ≥ balance` |
| `TransferFailed` | Low-level call reverted |

---

### Solana Errors (`WraithError`)
| Error | Condition |
|-------|-----------|
| `InvalidNameLength` | Name not 3-32 chars |
| `InvalidNameCharacter` | Not lowercase alnum or hyphen |
| `NotOwner` | `name_record.owner ≠ owner.key()` |

---

### CKB Errors

#### `wraith-stealth-lock` (`Error`)
| Code | Error | Condition |
|------|-------|-----------|
| 1 | IndexOutOfBound | Witness/index missing |
| 2 | ItemMissing | Required cell missing |
| 3 | LengthNotEnough | Data too short |
| 4 | Encoding | Serialization error |
| 5 | ArgsLengthNotEnough | Lock args ≠53 bytes |
| 6 | SignatureLengthNotEnough | Sig ≠65 bytes |
| 7 | AuthError | ckb-auth verification failed |

#### `wraith-names-type` (`Error`)
| Code | Error | Condition |
|------|-------|-----------|
| 1 | IndexOutOfBound | Cell index missing |
| 2 | ItemMissing | Required cell missing |
| 3 | LengthNotEnough | Data too short |
| 4 | Encoding | Serialization error |
| 5 | InvalidDataLength | Cell data ≠66 bytes |

---

## Replay Protection Summary

| Chain | Contract | Mechanism |
|-------|----------|-----------|
| Stellar | stealth-registry | None (registrant auth only) |
| Stellar | wraith-names | Replay key (sha256 of auth message) stored in `DataKey::Replay` |
| Stellar | stealth-sender | Multisig rotation: `PendingRotation` state + approvals vec |
| Stellar | wraith-names | Multisig rotation: `PendingRotation` state + approvals vec |
| Stellar | stealth-splitter | None (immutable; deterministic split_id from beneficiaries + salt) |
| EVM | ERC6538Registry | Nonce per registrant (`_nonces[registrant]++`) |
| EVM | WraithNames | Nonce per spending address (`nonces[spendingAddr]++`) |
| Solana | wraith-names | PDA uniqueness (one per name) |
| CKB | (scripts) | Cell uniqueness (type script hash) |

---

## Zero-Address / Stale Authority Checks

| Chain | Contract | Check |
|-------|----------|-------|
| Stellar | stealth-sender | `caller != admin` → panic |
| Stellar | stealth-vault | `caller != admin` → panic |
| Stellar | wraith-names | `caller != admin` → panic |
| Stellar | wraith-asset-policy | `admin.require_auth()` (implicit) |
| Stellar | governance | `admin.require_auth()` for cancel during voting |
| EVM | (none) | N/A (no admin) |
| Solana | (none) | N/A (no admin) |
| CKB | (none) | N/A (no admin) |

**Note**: Stellar contracts panic on unauthorized admin calls rather than returning errors, as the admin is expected to be a multisig where failed auth = invalid transaction.

---

## Release Artifacts

This matrix is published alongside contract releases:

| Artifact | Location |
|----------|----------|
| Authorization Matrix (this doc) | `AUTHORIZATION_MATRIX.md` |
| Stellar upgrade auth tests | `stellar/*/tests/upgrade_auth.rs` |
| EVM test suite | `evm/test/*.test.ts` |
| Solana test suite | `solana/programs/*/tests/` |
| CKB test suite | `ckb/contracts/*/src/main.rs` (embedded) |
| Governance docs | `stellar/GOVERNANCE.md`, `stellar/PAUSE.md`, `stellar/MULTISIG.md` |

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2026-09-25 | @collinsezedike | Initial matrix covering all 4 chains |

---

## Contact

For questions about authorization models:
- GitHub Issues: [wraith-protocol/contracts](https://github.com/wraith-protocol/contracts)
- Security Contact: `security@usewraith.xyz`