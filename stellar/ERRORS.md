# Stellar Contract Error Catalog

This catalog maps Soroban `#[contracterror]` codes to their source variants for SDKs, indexers, and integration tests.

`Introduced in` is `pre-catalog` for variants that already existed when this catalog was added on 2026-08-25. New variants should name the PR, issue, or release that introduced the code.

## Code allocation policy

Error codes are only unique within a contract enum at the Soroban ABI layer, but Wraith keeps disjoint reserved ranges so logs and SDK helpers can map raw codes without re-deriving context from Rust source.

| Contract or enum | Reserved range | Notes |
|---|---:|---|
| `stealth-announcer` | none | Panic-only/stateless event emitter today. Reserve `1000-1099` if it gains `#[contracterror]`. |
| `stealth-registry` / `RegistryError` | `1100-1199` | Existing deployed codes are `1-2`; do not renumber them. Add future variants in the reserved range unless a breaking ABI migration is planned. |
| `stealth-sender` / `SenderError` | `1200-1299` | Existing deployed codes are `1-16`; do not renumber them. |
| `stealth-batch-sender` | `1300-1399` | Panic-only today; use this range when issue #1 converts panics to `#[contracterror]`. |
| `stealth-vault` / `VaultError` | `1400-1499` | Existing codes are `1-7`; do not renumber them. |
| `stealth-splitter` / `SplitterError` | `1500-1599` | Existing codes are `1-8`; do not renumber them. |
| `wraith-names` / `NamesError` | `1600-1699` | Existing codes are `1-32`; do not renumber them. New variants use the reserved range, starting at `1600`. |
| `wraith-names` / `AuctionError` | `1700-1799` | Existing codes are `100-123` and intentionally disjoint from `NamesError`; do not renumber them. |
| `wraith-asset-policy` | `1800-1899` | Panic-only today; use this range if it gains `#[contracterror]`. |
| `governance` / `GovernanceError` | `1900-1999` | Proof-of-concept governance contract; existing codes are `1-14`. |

For existing public enums, numeric codes are part of the contract ABI. Append new variants; never recycle or renumber old codes.

## stealth-announcer

No `#[contracterror]` enum is defined. Current validation failures are panics/assertions in [`stealth-announcer/src/lib.rs`](stealth-announcer/src/lib.rs).

| Code | Name | Meaning | Introduced in |
|---:|---|---|---|
| N/A | Panic-only | `announce` rejects non-v2 scheme IDs or missing view-tag metadata by panic/assertion. | pre-catalog |

## stealth-registry

| Code | Name | Meaning | Introduced in |
|---:|---|---|---|
| 1 | [`RegistryError::InvalidMetaAddressLength`](stealth-registry/src/lib.rs#L59) | Supplied stealth meta-address is not exactly 64 bytes. | pre-catalog |
| 2 | [`RegistryError::NotRegistered`](stealth-registry/src/lib.rs#L61) | No meta-address is registered for the requested address and scheme. | pre-catalog |

## stealth-sender

| Code | Name | Meaning | Introduced in |
|---:|---|---|---|
| 1 | [`SenderError::AlreadyInitialized`](stealth-sender/src/lib.rs#L42) | Contract initialization was attempted more than once. | pre-catalog |
| 2 | [`SenderError::NotInitialized`](stealth-sender/src/lib.rs#L44) | Contract state required for the call has not been initialized. | pre-catalog |
| 3 | [`SenderError::LengthMismatch`](stealth-sender/src/lib.rs#L46) | Batch input vectors do not have the same length. | pre-catalog |
| 4 | [`SenderError::TokenNotAllowed`](stealth-sender/src/lib.rs#L48) | Configured asset policy rejected the token. | pre-catalog |
| 5 | [`SenderError::InvalidFeeConfig`](stealth-sender/src/lib.rs#L50) | Fee basis points or fee-recipient configuration is invalid. | pre-catalog |
| 6 | [`SenderError::BatchTooLarge`](stealth-sender/src/lib.rs#L54) | Withdrawal batch exceeds the supported size cap. | pre-catalog |
| 7 | [`SenderError::MultisigNotInitialized`](stealth-sender/src/lib.rs#L56) | Governance multisig has not been initialized. | pre-catalog |
| 8 | [`SenderError::MultisigAlreadyInitialized`](stealth-sender/src/lib.rs#L58) | Governance multisig initialization was attempted more than once. | pre-catalog |
| 9 | [`SenderError::NotSigner`](stealth-sender/src/lib.rs#L60) | Caller is not a current governance signer. | pre-catalog |
| 10 | [`SenderError::InvalidThreshold`](stealth-sender/src/lib.rs#L62) | Requested multisig threshold is zero or exceeds signer count. | pre-catalog |
| 11 | [`SenderError::RotationAlreadyPending`](stealth-sender/src/lib.rs#L64) | A signer-rotation proposal already exists. | pre-catalog |
| 12 | [`SenderError::NoPendingRotation`](stealth-sender/src/lib.rs#L66) | No signer-rotation proposal exists for this action. | pre-catalog |
| 13 | [`SenderError::AlreadyApprovedRotation`](stealth-sender/src/lib.rs#L68) | Caller already approved the pending rotation. | pre-catalog |
| 14 | [`SenderError::QuorumNotMet`](stealth-sender/src/lib.rs#L70) | Pending rotation lacks enough approvals. | pre-catalog |
| 15 | [`SenderError::TimelockNotElapsed`](stealth-sender/src/lib.rs#L72) | Rotation timelock has not elapsed. | pre-catalog |
| 16 | [`SenderError::Paused`](stealth-sender/src/lib.rs#L52) | Contract is paused. | pre-catalog |

## stealth-batch-sender

No `#[contracterror]` enum is defined. This contract is currently panic-only; issue #1 tracks conversion to structured errors.

| Code | Name | Meaning | Introduced in |
|---:|---|---|---|
| N/A | Panic-only | Empty batches, oversized batches, non-positive amounts, empty ephemeral keys, and token failures currently abort by panic/host error. | pre-catalog |

## stealth-vault

| Code | Name | Meaning | Introduced in |
|---:|---|---|---|
| 1 | [`VaultError::AlreadyInitialized`](stealth-vault/src/lib.rs#L33) | Contract initialization was attempted more than once. | pre-catalog |
| 2 | [`VaultError::NotInitialized`](stealth-vault/src/lib.rs#L34) | Announcer address has not been initialized. | pre-catalog |
| 3 | [`VaultError::InvalidWindow`](stealth-vault/src/lib.rs#L35) | Refund window is not strictly after unlock plus grace period. | pre-catalog |
| 4 | [`VaultError::DepositNotFound`](stealth-vault/src/lib.rs#L36) | Deposit ID is unknown or already consumed. | pre-catalog |
| 5 | [`VaultError::NotYetUnlocked`](stealth-vault/src/lib.rs#L37) | Claim attempted before the unlock ledger. | pre-catalog |
| 6 | [`VaultError::NotYetRefundable`](stealth-vault/src/lib.rs#L38) | Refund attempted before the refund ledger. | pre-catalog |
| 7 | [`VaultError::WrongRecipient`](stealth-vault/src/lib.rs#L39) | Claim signer does not match the deposit recipient. | pre-catalog |
| 8 | [`VaultError::Paused`](stealth-vault/src/lib.rs#L98) | Vault operations are paused. | PR #169 |
| 9 | [`VaultError::NotYetPermissionless`](stealth-vault/src/lib.rs#L100) | Operation attempted before the permissionless threshold has elapsed. | PR #169 |
| 10 | [`VaultError::InvalidGracePeriod`](stealth-vault/src/lib.rs#L102) | Specified grace period parameter is invalid. | PR #169 |

## stealth-splitter

| Code | Name | Meaning | Introduced in |
|---:|---|---|---|
| 1 | [`SplitterError::AlreadyInitialized`](stealth-splitter/src/lib.rs#L60) | Contract initialization was attempted more than once. | pre-catalog |
| 2 | [`SplitterError::NotInitialized`](stealth-splitter/src/lib.rs#L62) | Announcer address has not been initialized. | pre-catalog |
| 3 | [`SplitterError::SplitNotFound`](stealth-splitter/src/lib.rs#L64) | Split ID is unknown, or a fund-split vector count did not match the split definition. | pre-catalog |
| 4 | [`SplitterError::TooManyBeneficiaries`](stealth-splitter/src/lib.rs#L66) | Split creation exceeded the 25-beneficiary cap. | pre-catalog |
| 5 | [`SplitterError::WeightOverflow`](stealth-splitter/src/lib.rs#L68) | Total beneficiary weight overflowed. | pre-catalog |
| 6 | [`SplitterError::InvalidMetaAddressLength`](stealth-splitter/src/lib.rs#L70) | Beneficiary stealth meta-address is not exactly 64 bytes. | pre-catalog |
| 7 | [`SplitterError::InvalidAmount`](stealth-splitter/src/lib.rs#L72) | Fund amount is zero or negative. | pre-catalog |
| 8 | [`SplitterError::EmptyBeneficiaries`](stealth-splitter/src/lib.rs#L74) | Split definition has no beneficiaries. | pre-catalog |

## wraith-names

| Code | Name | Meaning | Introduced in |
|---:|---|---|---|
| 1 | [`NamesError::NameTaken`](wraith-names/src/lib.rs#L87) | Name is already registered. | pre-catalog |
| 2 | [`NamesError::NameTooShort`](wraith-names/src/lib.rs#L88) | Name is shorter than the minimum length. | pre-catalog |
| 3 | [`NamesError::NameTooLong`](wraith-names/src/lib.rs#L89) | Name exceeds the maximum length. | pre-catalog |
| 4 | [`NamesError::InvalidNameCharacter`](wraith-names/src/lib.rs#L90) | Name contains a character outside the allowed set. | pre-catalog |
| 5 | [`NamesError::InvalidMetaAddress`](wraith-names/src/lib.rs#L91) | Stealth meta-address is invalid, usually not 64 bytes. | pre-catalog |
| 6 | [`NamesError::NameNotFound`](wraith-names/src/lib.rs#L92) | Name or reverse lookup entry was not found. | pre-catalog |
| 7 | [`NamesError::NotOwner`](wraith-names/src/lib.rs#L93) | Caller is not authorized as owner or parent owner. | pre-catalog |
| 8 | [`NamesError::SignatureExpired`](wraith-names/src/lib.rs#L94) | On-behalf signature expiry ledger has passed. | pre-catalog |
| 9 | [`NamesError::SignatureReplay`](wraith-names/src/lib.rs#L95) | On-behalf signature was already used. | pre-catalog |
| 10 | [`NamesError::InvalidSigner`](wraith-names/src/lib.rs#L96) | Owner address could not be converted to a supported signer key. | pre-catalog |
| 11 | [`NamesError::NotGuardian`](wraith-names/src/lib.rs#L97) | Caller is not one of the configured guardians. | pre-catalog |
| 12 | [`NamesError::NoProposal`](wraith-names/src/lib.rs#L98) | No recovery proposal exists. | pre-catalog |
| 13 | [`NamesError::ProposalAlreadyExists`](wraith-names/src/lib.rs#L99) | A recovery proposal already exists. | pre-catalog |
| 14 | [`NamesError::AlreadyApproved`](wraith-names/src/lib.rs#L100) | Guardian already approved the proposal. | pre-catalog |
| 15 | [`NamesError::DelayNotElapsed`](wraith-names/src/lib.rs#L101) | Recovery delay has not elapsed. | pre-catalog |
| 16 | [`NamesError::ThresholdNotMet`](wraith-names/src/lib.rs#L102) | Recovery guardian threshold has not been met. | pre-catalog |
| 17 | [`NamesError::TooManyGuardians`](wraith-names/src/lib.rs#L103) | Guardian set exceeds the supported size. | pre-catalog |
| 18 | [`NamesError::InvalidThreshold`](wraith-names/src/lib.rs#L104) | Guardian or multisig threshold is invalid. | pre-catalog |
| 19 | [`NamesError::InvalidExtendLedger`](wraith-names/src/lib.rs#L105) | Requested TTL extension ledger is not in the future. | pre-catalog |
| 20 | [`NamesError::ParentNotFound`](wraith-names/src/lib.rs#L106) | Subdomain parent name does not exist. | pre-catalog |
| 21 | [`NamesError::MultisigNotInitialized`](wraith-names/src/lib.rs#L110) | Protocol governance multisig has not been initialized. | pre-catalog |
| 22 | [`NamesError::MultisigAlreadyInitialized`](wraith-names/src/lib.rs#L112) | Protocol governance multisig initialization was attempted more than once. | pre-catalog |
| 23 | [`NamesError::NotSigner`](wraith-names/src/lib.rs#L114) | Caller is not a current protocol governance signer. | pre-catalog |
| 24 | [`NamesError::RotationAlreadyPending`](wraith-names/src/lib.rs#L116) | A signer-rotation proposal already exists. | pre-catalog |
| 25 | [`NamesError::NoPendingRotation`](wraith-names/src/lib.rs#L118) | No signer-rotation proposal exists for this action. | pre-catalog |
| 26 | [`NamesError::AlreadyApprovedRotation`](wraith-names/src/lib.rs#L120) | Caller already approved the pending rotation. | pre-catalog |
| 27 | [`NamesError::QuorumNotMet`](wraith-names/src/lib.rs#L122) | Pending rotation lacks enough approvals. | pre-catalog |
| 28 | [`NamesError::TimelockNotElapsed`](wraith-names/src/lib.rs#L124) | Rotation timelock has not elapsed. | pre-catalog |
| 29 | [`NamesError::NameTooDeep`](wraith-names/src/lib.rs#L125) | Name exceeds the supported subdomain depth. | pre-catalog |
| 30 | [`NamesError::BulkLimitExceeded`](wraith-names/src/lib.rs#L126) | Bulk operation exceeds the supported item cap. | pre-catalog |
| 31 | [`NamesError::PremiumAuctionRequired`](wraith-names/src/lib.rs#L129) | Premium top-level name must be obtained through auction during the launch window. | pre-catalog |
| 32 | [`NamesError::Paused`](wraith-names/src/lib.rs#L108) | Contract is paused. | pre-catalog |
| 1600 | [`NamesError::AuctionsNotInitialized`](wraith-names/src/lib.rs#L135) | Auction subsystem is not initialized, so there is no auction admin to rotate. | [#165](https://github.com/wraith-protocol/contracts/issues/165) |
| 1601 | [`NamesError::AuctionInProgress`](wraith-names/src/lib.rs#L138) | An auction has a revealed winner and has not settled, so the auction admin cannot be rotated. | [#165](https://github.com/wraith-protocol/contracts/issues/165) |

## wraith-names auctions

| Code | Name | Meaning | Introduced in |
|---:|---|---|---|
| 100 | [`AuctionError::NotInitialized`](wraith-names/src/auction.rs#L107) | Auction subsystem has not been initialized. | pre-catalog |
| 101 | [`AuctionError::AlreadyInitialized`](wraith-names/src/auction.rs#L108) | Auction subsystem initialization was attempted more than once. | pre-catalog |
| 102 | [`AuctionError::InvalidConfig`](wraith-names/src/auction.rs#L109) | Reserve price or phase durations are invalid. | pre-catalog |
| 103 | [`AuctionError::WindowClosed`](wraith-names/src/auction.rs#L110) | Premium-name launch auction window has closed. | pre-catalog |
| 104 | [`AuctionError::NotPremiumName`](wraith-names/src/auction.rs#L111) | Name is not eligible for premium-name auction handling. | pre-catalog |
| 105 | [`AuctionError::NameAlreadyRegistered`](wraith-names/src/auction.rs#L112) | Name was already registered before auction start or claim. | pre-catalog |
| 106 | [`AuctionError::AuctionExists`](wraith-names/src/auction.rs#L113) | Auction already exists for the name. | pre-catalog |
| 107 | [`AuctionError::NoAuction`](wraith-names/src/auction.rs#L114) | No auction exists for the name. | pre-catalog |
| 108 | [`AuctionError::CommitPhaseOver`](wraith-names/src/auction.rs#L115) | Commit attempted after the commit phase ended. | pre-catalog |
| 109 | [`AuctionError::AlreadyCommitted`](wraith-names/src/auction.rs#L116) | Bidder already committed for this auction. | pre-catalog |
| 110 | [`AuctionError::DepositBelowReserve`](wraith-names/src/auction.rs#L117) | Bid deposit is below the configured reserve price. | pre-catalog |
| 111 | [`AuctionError::RevealPhaseNotActive`](wraith-names/src/auction.rs#L118) | Reveal attempted outside the reveal phase. | pre-catalog |
| 112 | [`AuctionError::NoBid`](wraith-names/src/auction.rs#L119) | No bid exists for the bidder and auction. | pre-catalog |
| 113 | [`AuctionError::AlreadyRevealed`](wraith-names/src/auction.rs#L120) | Bid was already revealed. | pre-catalog |
| 114 | [`AuctionError::CommitmentMismatch`](wraith-names/src/auction.rs#L121) | Revealed amount/salt does not match the stored commitment. | pre-catalog |
| 115 | [`AuctionError::BidBelowReserve`](wraith-names/src/auction.rs#L122) | Revealed bid amount is below reserve. | pre-catalog |
| 116 | [`AuctionError::BidExceedsDeposit`](wraith-names/src/auction.rs#L123) | Revealed bid amount exceeds locked deposit. | pre-catalog |
| 117 | [`AuctionError::RevealPhaseNotOver`](wraith-names/src/auction.rs#L124) | Settle or withdraw attempted before reveal phase ended. | pre-catalog |
| 118 | [`AuctionError::AlreadySettled`](wraith-names/src/auction.rs#L125) | Auction has already been settled. | pre-catalog |
| 119 | [`AuctionError::NotSettled`](wraith-names/src/auction.rs#L126) | Winner tried to claim before settlement. | pre-catalog |
| 120 | [`AuctionError::NotWinner`](wraith-names/src/auction.rs#L127) | Caller is not the winning bidder. | pre-catalog |
| 121 | [`AuctionError::WinnerCannotWithdraw`](wraith-names/src/auction.rs#L128) | Highest bidder cannot self-withdraw before settlement. | pre-catalog |
| 122 | [`AuctionError::InvalidMetaAddress`](wraith-names/src/auction.rs#L129) | Winner claim supplied an invalid stealth meta-address. | pre-catalog |
| 123 | [`AuctionError::RegistrationFailed`](wraith-names/src/auction.rs#L130) | Auction claim could not complete name registration for an unmapped reason. | pre-catalog |

## wraith-asset-policy

No `#[contracterror]` enum is defined. This contract is currently panic-only.

| Code | Name | Meaning | Introduced in |
|---:|---|---|---|
| N/A | Panic-only | Double initialization and missing admin state currently abort by panic/host error. | pre-catalog |

## governance

| Code | Name | Meaning | Introduced in |
|---:|---|---|---|
| 1 | [`GovernanceError::AlreadyInitialized`](contracts/governance/src/lib.rs#L109) | Governance initialization was attempted more than once. | pre-catalog |
| 2 | [`GovernanceError::NotInitialized`](contracts/governance/src/lib.rs#L110) | Governance configuration has not been initialized. | pre-catalog |
| 3 | [`GovernanceError::NotAdmin`](contracts/governance/src/lib.rs#L111) | Caller is not the admin. | pre-catalog |
| 4 | [`GovernanceError::ProposalNotFound`](contracts/governance/src/lib.rs#L112) | Proposal ID does not exist. | pre-catalog |
| 5 | [`GovernanceError::AlreadyVoted`](contracts/governance/src/lib.rs#L113) | Voter already voted on the proposal. | pre-catalog |
| 6 | [`GovernanceError::VotingNotActive`](contracts/governance/src/lib.rs#L114) | Vote attempted outside the active voting window. | pre-catalog |
| 7 | [`GovernanceError::VotingStillActive`](contracts/governance/src/lib.rs#L115) | Execute/cancel path requires voting to have ended. | pre-catalog |
| 8 | [`GovernanceError::QuorumNotMet`](contracts/governance/src/lib.rs#L116) | Proposal did not receive enough total voting power. | pre-catalog |
| 9 | [`GovernanceError::ProposalDefeated`](contracts/governance/src/lib.rs#L117) | Proposal received insufficient support to pass. | pre-catalog |
| 10 | [`GovernanceError::TimelockNotElapsed`](contracts/governance/src/lib.rs#L118) | Proposal timelock has not elapsed after voting closed. | pre-catalog |
| 11 | [`GovernanceError::AlreadyExecuted`](contracts/governance/src/lib.rs#L119) | Proposal has already been executed. | pre-catalog |
| 12 | [`GovernanceError::AlreadyCancelled`](contracts/governance/src/lib.rs#L120) | Proposal has already been cancelled. | pre-catalog |
| 13 | [`GovernanceError::ExecutionFailed`](contracts/governance/src/lib.rs#L121) | Target execution failed. | pre-catalog |
| 14 | [`GovernanceError::NoVotingPower`](contracts/governance/src/lib.rs#L122) | Voter has no token balance to vote with. | pre-catalog |

## test-only mock contracts

These enums are compiled only for tests or live under test fixtures. They are included so the CI coverage check catches every `#[contracterror]` variant in the repository.

| Code | Name | Meaning | Introduced in |
|---:|---|---|---|
| 1 | [`MockTokenError::InsufficientBalance`](stealth-sender/src/test_mocks.rs#L26) | Mock token transfer balance is too low. | pre-catalog |
| 2 | [`MockTokenError::InsufficientAllowance`](stealth-sender/src/test_mocks.rs#L27) | Mock token allowance is too low. | pre-catalog |
| 11 | [`TokenError::BalanceDeauthorized`](stealth-sender/tests/mocks/token_auth_required.rs#L12) | Test token recipient is not authorized to receive balance. | pre-catalog |
