Stellar Contract Error Catalog
This catalog maps Soroban #[contracterror] codes to their source variants for SDKs, indexers, and integration tests.

Introduced in is pre-catalog for variants that already existed when this catalog was added on 2026-08-25. New variants should name the PR, issue, or release that introduced the code.

Code allocation policy
Error codes are only unique within a contract enum at the Soroban ABI layer, but Wraith keeps disjoint reserved ranges so logs and SDK helpers can map raw codes without re-deriving context from Rust source.

Contract or enum	Reserved range	Notes
stealth-announcer	none	Panic-only/stateless event emitter today. Reserve 1000-1099 if it gains #[contracterror].
stealth-registry / RegistryError	1100-1199	Existing deployed codes are 1-2; do not renumber them. Add future variants in the reserved range unless a breaking ABI migration is planned.
stealth-sender / SenderError	1200-1299	Existing deployed codes are 1-16; do not renumber them.
stealth-batch-sender / BatchSenderError	1300-1399	Existing deployed codes are 1300-1316; do not renumber them.
stealth-vault / VaultError	1400-1499	Existing codes are 1-7; do not renumber them.
stealth-splitter / SplitterError	1500-1599	Existing codes are 1-8; do not renumber them.
wraith-names / NamesError	1600-1699	Existing codes are 1-32; do not renumber them. New variants use the reserved range, starting at 1600.
wraith-names / AuctionError	1700-1799	Existing codes are 100-123 and intentionally disjoint from NamesError; do not renumber them.
wraith-asset-policy	1800-1899	Panic-only today; use this range if it gains #[contracterror].
governance / GovernanceError	1900-1999	Proof-of-concept governance contract; existing codes are 1-14.
For existing public enums, numeric codes are part of the contract ABI. Append new variants; never recycle or renumber old codes.

stealth-announcer
No #[contracterror] enum is defined. Current validation failures are panics/assertions in stealth-announcer/src/lib.rs.

Code	Name	Meaning	Introduced in
N/A	Panic-only	announce rejects non-v2 scheme IDs or missing view-tag metadata by panic/assertion.	pre-catalog
stealth-registry
Code	Name	Meaning	Introduced in
1	RegistryError::InvalidMetaAddressLength	Supplied stealth meta-address is not exactly 64 bytes.	pre-catalog
2	RegistryError::NotRegistered	No meta-address is registered for the requested address and scheme.	pre-catalog
stealth-sender
Code	Name	Meaning	Introduced in
1	SenderError::AlreadyInitialized	Contract initialization was attempted more than once.	pre-catalog
2	SenderError::NotInitialized	Contract state required for the call has not been initialized.	pre-catalog
3	SenderError::LengthMismatch	Batch input vectors do not have the same length.	pre-catalog
4	SenderError::TokenNotAllowed	Configured asset policy rejected the token.	pre-catalog
5	SenderError::InvalidFeeConfig	Fee basis points or fee-recipient configuration is invalid.	pre-catalog
6	SenderError::BatchTooLarge	Withdrawal batch exceeds the supported size cap.	pre-catalog
7	SenderError::MultisigNotInitialized	Governance multisig has not been initialized.	pre-catalog
8	SenderError::MultisigAlreadyInitialized	Governance multisig initialization was attempted more than once.	pre-catalog
9	SenderError::NotSigner	Caller is not a current governance signer.	pre-catalog
10	SenderError::InvalidThreshold	Requested multisig threshold is zero or exceeds signer count.	pre-catalog
11	SenderError::RotationAlreadyPending	A signer-rotation proposal already exists.	pre-catalog
12	SenderError::NoPendingRotation	No signer-rotation proposal exists for this action.	pre-catalog
13	SenderError::AlreadyApprovedRotation	Caller already approved the pending rotation.	pre-catalog
14	SenderError::QuorumNotMet	Pending rotation lacks enough approvals.	pre-catalog
15	SenderError::TimelockNotElapsed	Rotation timelock has not elapsed.	pre-catalog
16	SenderError::Paused	Contract is paused.	pre-catalog
stealth-batch-sender
Code	Name	Meaning	Introduced in
1300	BatchSenderError::AlreadyInitialized	Contract initialization was attempted more than once.	issue #155
1301	BatchSenderError::NotInitialized	batch_send was called before init.	issue #155
1302	BatchSenderError::EmptyBatch	Batch contains no transfers.	issue #155
1303	BatchSenderError::BatchTooLarge	Batch exceeds MAX_BATCH_SIZE (100).	issue #155
1304	BatchSenderError::NonPositiveAmount	A transfer amount is zero or negative.	issue #155
1305	BatchSenderError::EmptyEphemeralKey	A transfer's ephemeral public key is empty or not 32 bytes.	issue #155
1306	BatchSenderError::Paused	Contract is paused.	issue #155
1307	BatchSenderError::AssetNotAllowed	Configured asset policy rejected the token.	issue #155
1308	BatchSenderError::MultisigNotInitialized	Governance multisig has not been initialized.	issue #155
1309	BatchSenderError::MultisigAlreadyInitialized	Governance multisig initialization was attempted more than once.	issue #155
1310	BatchSenderError::NotSigner	Caller is not a current governance signer.	issue #155
1311	BatchSenderError::InvalidThreshold	Requested multisig threshold is zero or exceeds signer count.	issue #155
1312	BatchSenderError::RotationAlreadyPending	A signer-rotation proposal already exists.	issue #155
1313	BatchSenderError::NoPendingRotation	No signer-rotation proposal exists for this action.	issue #155
1314	BatchSenderError::AlreadyApprovedRotation	Caller already approved the pending rotation.	issue #155
1315	BatchSenderError::QuorumNotMet	Pending rotation lacks enough approvals.	issue #155
1316	BatchSenderError::TimelockNotElapsed	Rotation timelock has not elapsed.	issue #155
stealth-vault
Code	Name	Meaning	Introduced in
1	VaultError::AlreadyInitialized	Contract initialization was attempted more than once.	pre-catalog
2	VaultError::NotInitialized	Announcer address has not been initialized.	pre-catalog
3	VaultError::InvalidWindow	Refund window is not strictly after unlock plus grace period.	pre-catalog
4	VaultError::DepositNotFound	Deposit ID is unknown or already consumed.	pre-catalog
5	VaultError::NotYetUnlocked	Claim attempted before the unlock ledger.	pre-catalog
6	VaultError::NotYetRefundable	Refund attempted before the refund ledger.	pre-catalog
7	VaultError::WrongRecipient	Claim signer does not match the deposit recipient.	pre-catalog
8	VaultError::Paused	Vault operations are paused.	PR #169
9	VaultError::NotYetPermissionless	Operation attempted before the permissionless threshold has elapsed.	PR #169
10	VaultError::InvalidGracePeriod	Specified grace period parameter is invalid.	PR #169
stealth-splitter
Code	Name	Meaning	Introduced in
1	SplitterError::AlreadyInitialized	Contract initialization was attempted more than once.	pre-catalog
2	SplitterError::NotInitialized	Announcer address has not been initialized.	pre-catalog
3	SplitterError::SplitNotFound	Split ID is unknown, or a fund-split vector count did not match the split definition.	pre-catalog
4	SplitterError::TooManyBeneficiaries	Split creation exceeded the 25-beneficiary cap.	pre-catalog
5	SplitterError::WeightOverflow	Total beneficiary weight overflowed.	pre-catalog
6	SplitterError::InvalidMetaAddressLength	Beneficiary stealth meta-address is not exactly 64 bytes.	pre-catalog
7	SplitterError::InvalidAmount	Fund amount is zero or negative.	pre-catalog
8	SplitterError::EmptyBeneficiaries	Split definition has no beneficiaries.	pre-catalog
wraith-names
Code	Name	Meaning	Introduced in
1	NamesError::NameTaken	Name is already registered.	pre-catalog
2	NamesError::NameTooShort	Name is shorter than the minimum length.	pre-catalog
3	NamesError::NameTooLong	Name exceeds the maximum length.	pre-catalog
4	NamesError::InvalidNameCharacter	Name contains a character outside the allowed set.	pre-catalog
5	NamesError::InvalidMetaAddress	Stealth meta-address is invalid, usually not 64 bytes.	pre-catalog
6	NamesError::NameNotFound	Name or reverse lookup entry was not found.	pre-catalog
7	NamesError::NotOwner	Caller is not authorized as owner or parent owner.	pre-catalog
8	NamesError::SignatureExpired	On-behalf signature expiry ledger has passed.	pre-catalog
9	NamesError::SignatureReplay	On-behalf signature was already used.	pre-catalog
10	NamesError::InvalidSigner	Owner address could not be converted to a supported signer key.	pre-catalog
11	NamesError::NotGuardian	Caller is not one of the configured guardians.	pre-catalog
12	NamesError::NoProposal	No recovery proposal exists.	pre-catalog
13	NamesError::ProposalAlreadyExists	A recovery proposal already exists.	pre-catalog
14	NamesError::AlreadyApproved	Guardian already approved the proposal.	pre-catalog
15	NamesError::DelayNotElapsed	Recovery delay has not elapsed.	pre-catalog
16	NamesError::ThresholdNotMet	Recovery guardian threshold has not been met.	pre-catalog
17	NamesError::TooManyGuardians	Guardian set exceeds the supported size.	pre-catalog
18	NamesError::InvalidThreshold	Guardian or multisig threshold is invalid.	pre-catalog
19	NamesError::InvalidExtendLedger	Requested TTL extension ledger is not in the future.	pre-catalog
20	NamesError::ParentNotFound	Subdomain parent name does not exist.	pre-catalog
21	NamesError::MultisigNotInitialized	Protocol governance multisig has not been initialized.	pre-catalog
22	NamesError::MultisigAlreadyInitialized	Protocol governance multisig initialization was attempted more than once.	pre-catalog
23	NamesError::NotSigner	Caller is not a current protocol governance signer.	pre-catalog
24	NamesError::RotationAlreadyPending	A signer-rotation proposal already exists.	pre-catalog
25	NamesError::NoPendingRotation	No signer-rotation proposal exists for this action.	pre-catalog
26	NamesError::AlreadyApprovedRotation	Caller already approved the pending rotation.	pre-catalog
27	NamesError::QuorumNotMet	Pending rotation lacks enough approvals.	pre-catalog
28	NamesError::TimelockNotElapsed	Rotation timelock has not elapsed.	pre-catalog
29	NamesError::NameTooDeep	Name exceeds the supported subdomain depth.	pre-catalog
30	NamesError::BulkLimitExceeded	Bulk operation exceeds the supported item cap.	pre-catalog
31	NamesError::PremiumAuctionRequired	Premium top-level name must be obtained through auction during the launch window.	pre-catalog
32	NamesError::Paused	Contract is paused.	pre-catalog
1600	NamesError::AuctionsNotInitialized	Auction subsystem is not initialized, so there is no auction admin to rotate.	#165
1601	NamesError::AuctionInProgress	An auction has a revealed winner and has not settled, so the auction admin cannot be rotated.	#165
wraith-names auctions
Code	Name	Meaning	Introduced in
100	AuctionError::NotInitialized	Auction subsystem has not been initialized.	pre-catalog
101	AuctionError::AlreadyInitialized	Auction subsystem initialization was attempted more than once.	pre-catalog
102	AuctionError::InvalidConfig	Reserve price or phase durations are invalid.	pre-catalog
103	AuctionError::WindowClosed	Premium-name launch auction window has closed.	pre-catalog
104	AuctionError::NotPremiumName	Name is not eligible for premium-name auction handling.	pre-catalog
105	AuctionError::NameAlreadyRegistered	Name was already registered before auction start or claim.	pre-catalog
106	AuctionError::AuctionExists	Auction already exists for the name.	pre-catalog
107	AuctionError::NoAuction	No auction exists for the name.	pre-catalog
108	AuctionError::CommitPhaseOver	Commit attempted after the commit phase ended.	pre-catalog
109	AuctionError::AlreadyCommitted	Bidder already committed for this auction.	pre-catalog
110	AuctionError::DepositBelowReserve	Bid deposit is below the configured reserve price.	pre-catalog
111	AuctionError::RevealPhaseNotActive	Reveal attempted outside the reveal phase.	pre-catalog
112	AuctionError::NoBid	No bid exists for the bidder and auction.	pre-catalog
113	AuctionError::AlreadyRevealed	Bid was already revealed.	pre-catalog
114	AuctionError::CommitmentMismatch	Revealed amount/salt does not match the stored commitment.	pre-catalog
115	AuctionError::BidBelowReserve	Revealed bid amount is below reserve.	pre-catalog
116	AuctionError::BidExceedsDeposit	Revealed bid amount exceeds locked deposit.	pre-catalog
117	AuctionError::RevealPhaseNotOver	Settle or withdraw attempted before reveal phase ended.	pre-catalog
118	AuctionError::AlreadySettled	Auction has already been settled.	pre-catalog
119	AuctionError::NotSettled	Winner tried to claim before settlement.	pre-catalog
120	AuctionError::NotWinner	Caller is not the winning bidder.	pre-catalog
121	AuctionError::WinnerCannotWithdraw	Highest bidder cannot self-withdraw before settlement.	pre-catalog
122	AuctionError::InvalidMetaAddress	Winner claim supplied an invalid stealth meta-address.	pre-catalog
123	AuctionError::RegistrationFailed	Auction claim could not complete name registration for an unmapped reason.	pre-catalog
wraith-asset-policy
No #[contracterror] enum is defined. This contract is currently panic-only.

Code	Name	Meaning	Introduced in
N/A	Panic-only	Double initialization and missing admin state currently abort by panic/host error.	pre-catalog
governance
Code	Name	Meaning	Introduced in
1	GovernanceError::AlreadyInitialized	Governance initialization was attempted more than once.	pre-catalog
2	GovernanceError::NotInitialized	Governance configuration has not been initialized.	pre-catalog
3	GovernanceError::NotAdmin	Caller is not the admin.	pre-catalog
4	GovernanceError::ProposalNotFound	Proposal ID does not exist.	pre-catalog
5	GovernanceError::AlreadyVoted	Voter already voted on the proposal.	pre-catalog
6	GovernanceError::VotingNotActive	Vote attempted outside the active voting window.	pre-catalog
7	GovernanceError::VotingStillActive	Execute/cancel path requires voting to have ended.	pre-catalog
8	GovernanceError::QuorumNotMet	Proposal did not receive enough total voting power.	pre-catalog
9	GovernanceError::ProposalDefeated	Proposal received insufficient support to pass.	pre-catalog
10	GovernanceError::TimelockNotElapsed	Proposal timelock has not elapsed after voting closed.	pre-catalog
11	GovernanceError::AlreadyExecuted	Proposal has already been executed.	pre-catalog
12	GovernanceError::AlreadyCancelled	Proposal has already been cancelled.	pre-catalog
13	GovernanceError::ExecutionFailed	Target execution failed.	pre-catalog
14	GovernanceError::NoVotingPower	Voter has no token balance to vote with.	pre-catalog
test-only mock contracts
These enums are compiled only for tests or live under test fixtures. They are included so the CI coverage check catches every #[contracterror] variant in the repository.

Code	Name	Meaning	Introduced in
1	MockTokenError::InsufficientBalance	Mock token transfer balance is too low.	pre-catalog
2	MockTokenError::InsufficientAllowance	Mock token allowance is too low.	pre-catalog
11	TokenError::BalanceDeauthorized	Test token recipient is not authorized to receive balance.	pre-catalog
<!-- ci-retrigger: validate error catalog after ERRORS.md's addition to this branch --><!-- ci-retrigger: attempt 2, dispatch gap on prior push -->