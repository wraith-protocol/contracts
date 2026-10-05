import { Buffer } from "buffer";
import { Address } from "@stellar/stellar-sdk";
import {
  AssembledTransaction,
  Client as ContractClient,
  ClientOptions as ContractClientOptions,
  MethodOptions,
  Result,
  Spec as ContractSpec,
} from "@stellar/stellar-sdk/contract";
import type {
  u32,
  i32,
  u64,
  i64,
  u128,
  i128,
  u256,
  i256,
  Option,
  Timepoint,
  Duration,
} from "@stellar/stellar-sdk/contract";
export * from "@stellar/stellar-sdk";
export * as contract from "@stellar/stellar-sdk/contract";
export * as rpc from "@stellar/stellar-sdk/rpc";

if (typeof window !== "undefined") {
  //@ts-ignore Buffer exists
  window.Buffer = window.Buffer || Buffer;
}




/**
 * Storage keys.
 */
export type DataKey = {tag: "Name", values: readonly [Buffer]} | {tag: "Reverse", values: readonly [Buffer]} | {tag: "Replay", values: readonly [Buffer]} | {tag: "Guardians", values: readonly [Buffer]} | {tag: "Recovery", values: readonly [Buffer]} | {tag: "Admin", values: void} | {tag: "Paused", values: void} | {tag: "MultisigSigners", values: void} | {tag: "MultisigThreshold", values: void} | {tag: "PendingRotation", values: void} | {tag: "PendingAuctionAdminRotation", values: void};


/**
 * A registered name entry.
 */
export interface NameEntry {
  name: string;
  owner: string;
  /**
 * For a subdomain (`sub.parent`), the name hash of the parent label.
 * `None` for a flat top-level name. Existing flat names register with
 * `None`, so prior behaviour is preserved.
 */
parent: Option<Buffer>;
  stealth_meta_address: Buffer;
}

/**
 * Errors.
 */
export const NamesError = {
  1: {message:"NameTaken"},
  2: {message:"NameTooShort"},
  3: {message:"NameTooLong"},
  4: {message:"InvalidNameCharacter"},
  5: {message:"InvalidMetaAddress"},
  6: {message:"NameNotFound"},
  7: {message:"NotOwner"},
  8: {message:"SignatureExpired"},
  9: {message:"SignatureReplay"},
  10: {message:"InvalidSigner"},
  11: {message:"NotGuardian"},
  12: {message:"NoProposal"},
  13: {message:"ProposalAlreadyExists"},
  14: {message:"AlreadyApproved"},
  15: {message:"DelayNotElapsed"},
  16: {message:"ThresholdNotMet"},
  17: {message:"TooManyGuardians"},
  18: {message:"InvalidThreshold"},
  19: {message:"InvalidExtendLedger"},
  20: {message:"ParentNotFound"},
  /**
   * The contract is paused.
   */
  32: {message:"Paused"},
  /**
   * The protocol-level governance multisig has not been initialised.
   */
  21: {message:"MultisigNotInitialized"},
  /**
   * The protocol-level governance multisig has already been initialised.
   */
  22: {message:"MultisigAlreadyInitialized"},
  /**
   * The caller is not a current protocol-level governance signer.
   */
  23: {message:"NotSigner"},
  /**
   * A signer-rotation proposal is already pending.
   */
  24: {message:"RotationAlreadyPending"},
  /**
   * No signer-rotation proposal is pending.
   */
  25: {message:"NoPendingRotation"},
  /**
   * The caller has already approved the pending rotation.
   */
  26: {message:"AlreadyApprovedRotation"},
  /**
   * The pending rotation has not collected enough approvals yet.
   */
  27: {message:"QuorumNotMet"},
  /**
   * The rotation timelock has not elapsed yet.
   */
  28: {message:"TimelockNotElapsed"},
  29: {message:"NameTooDeep"},
  30: {message:"BulkLimitExceeded"},
  /**
   * The name is premium (<= 4 chars) and the auction window is active, so
   * it can only be obtained through the sealed-bid auction.
   */
  31: {message:"PremiumAuctionRequired"},
  /**
   * The auction subsystem has not been initialised, so there is no auction
   * admin to rotate.
   */
  1600: {message:"AuctionsNotInitialized"},
  /**
   * An auction has a revealed winner and has not settled yet, so the
   * auction admin cannot be rotated out from under it.
   */
  1601: {message:"AuctionInProgress"}
}


/**
 * Guardian configuration for a name.
 */
export interface GuardianConfig {
  guardians: Array<string>;
  threshold: u32;
}


/**
 * A pending recovery proposal.
 */
export interface RecoveryProposal {
  approvals: Array<string>;
  new_meta_address: Buffer;
  new_owner: string;
  proposed_at: u32;
}


/**
 * State of a single name auction.
 */
export interface Auction {
  /**
 * Timestamp at which the commit phase ends and the reveal phase begins.
 */
commit_end: u64;
  /**
 * Highest revealed bid amount so far.
 */
highest_amount: i128;
  /**
 * Highest revealed bidder so far. Ties go to the earliest reveal.
 */
highest_bidder: Option<string>;
  name: string;
  /**
 * Timestamp at which the reveal phase ends and settlement is possible.
 */
reveal_end: u64;
  settled: boolean;
}


/**
 * A sealed bid.
 */
export interface SealedBid {
  /**
 * sha256 commitment binding bidder, name, amount and salt.
 */
commitment: Buffer;
  /**
 * Tokens locked in the contract; refunded in full to losers.
 */
deposit: i128;
  revealed: boolean;
}

/**
 * Storage keys for the auction subsystem.
 */
export type AuctionKey = {tag: "Config", values: void} | {tag: "Auction", values: readonly [Buffer]} | {tag: "Bid", values: readonly [Buffer, string]} | {tag: "PendingSettlements", values: void};

/**
 * Auction errors. Codes start at 100 to stay disjoint from `NamesError`.
 */
export const AuctionError = {
  100: {message:"NotInitialized"},
  101: {message:"AlreadyInitialized"},
  102: {message:"InvalidConfig"},
  103: {message:"WindowClosed"},
  104: {message:"NotPremiumName"},
  105: {message:"NameAlreadyRegistered"},
  106: {message:"AuctionExists"},
  107: {message:"NoAuction"},
  108: {message:"CommitPhaseOver"},
  109: {message:"AlreadyCommitted"},
  110: {message:"DepositBelowReserve"},
  111: {message:"RevealPhaseNotActive"},
  112: {message:"NoBid"},
  113: {message:"AlreadyRevealed"},
  114: {message:"CommitmentMismatch"},
  115: {message:"BidBelowReserve"},
  116: {message:"BidExceedsDeposit"},
  117: {message:"RevealPhaseNotOver"},
  118: {message:"AlreadySettled"},
  119: {message:"NotSettled"},
  120: {message:"NotWinner"},
  121: {message:"WinnerCannotWithdraw"},
  122: {message:"InvalidMetaAddress"},
  123: {message:"RegistrationFailed"}
}


/**
 * Auction configuration, set once at initialization.
 */
export interface AuctionConfig {
  /**
 * Operator that runs settlements per the runbook.
 */
admin: string;
  /**
 * Duration of the commit phase of each auction, in seconds.
 */
commit_secs: u64;
  /**
 * Ledger timestamp at initialization; the premium window runs for
 * `PREMIUM_WINDOW_SECS` from this point.
 */
launch_time: u64;
  /**
 * Minimum bid; deposits must also be at least this amount.
 */
reserve_price: i128;
  /**
 * Duration of the reveal phase of each auction, in seconds.
 */
reveal_secs: u64;
  /**
 * Payment token (native XLM SAC on mainnet).
 */
token: string;
  /**
 * Receives winning bids.
 */
treasury: string;
}


/**
 * A pending signer-rotation proposal.
 */
export interface RotationProposal {
  approvals: Array<string>;
  executable_at: u64;
  new_signers: Array<string>;
  new_threshold: u32;
}


/**
 * A pending auction-admin rotation proposal. Mirrors `RotationProposal` —
 * same signer set, same quorum, same timelock — but carries the incoming
 * auction operator address instead of a new signer set.
 */
export interface AdminRotationProposal {
  approvals: Array<string>;
  executable_at: u64;
  new_admin: string;
}


/**
 * Wraith Protocol standard metric event schema.
 * 
 * All Wraith contracts emit metric events using this structure to enable
 * standardized off-chain observability and monitoring.
 */
export interface WraithMetricEvent {
  /**
 * Contract identifier (e.g., "stealth-registry", "stealth-sender")
 */
contract: string;
  /**
 * Optional dimensions for filtering/grouping (e.g., token_address, scheme_id)
 */
dimensions: Array<readonly [string, any]>;
  /**
 * Metric name (e.g., "register_count", "send_volume")
 */
metric_name: string;
  /**
 * Numeric value of the metric
 */
value: i128;
}

export interface Client {
  /**
   * Construct and simulate a init transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Initialise the contract by storing the pause admin.
   * 
   * Must be called before `pause` / `unpause`. Idempotent: calling
   * more than once is a no-op (the first admin sticks).
   */
  init: ({admin}: {admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a pause transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Pause the contract — admin only.
   * Prevents all registrations, updates, releases and TTL extensions
   * while paused. Lookups (`resolve`, `name_of`) remain available.
   */
  pause: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a update transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Update the meta-address for an existing name.
   * Only the current owner can update.
   */
  update: ({owner, name, new_meta_address}: {owner: string, name: string, new_meta_address: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a name_of transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Reverse lookup: find the name for a given stealth meta-address.
   */
  name_of: ({stealth_meta_address}: {stealth_meta_address: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<string>>>

  /**
   * Construct and simulate a release transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Release a name, making it available again.
   */
  release: ({owner, name}: {owner: string, name: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a resolve transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Resolve a name to its stealth meta-address.
   * 
   * For a subdomain (`payments.alice`) resolution walks to the parent
   * (`alice`): if the parent no longer exists the subdomain does not resolve.
   */
  resolve: ({name}: {name: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Buffer>>>

  /**
   * Construct and simulate a signers transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Current protocol-level governance signer set.
   */
  signers: (options?: MethodOptions) => Promise<AssembledTransaction<Array<string>>>

  /**
   * Construct and simulate a unpause transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Unpause the contract — admin only.
   */
  unpause: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a register transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Register a name mapped to a stealth meta-address.
   */
  register: ({owner, name, stealth_meta_address}: {owner: string, name: string, stealth_meta_address: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a is_paused transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns true if the contract is paused.
   */
  is_paused: (options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a threshold transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Current protocol-level governance quorum threshold.
   */
  threshold: (options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a bulk_renew transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Renew (extend TTL for) multiple names in a single atomic transaction.
   * 
   * All names must exist. If any name is not found, the entire operation
   * reverts.
   */
  bulk_renew: ({names, extend_to_ledger}: {names: Array<string>, extend_to_ledger: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a claim_name transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Claim a won auction: registers the name to the winner with their
   * stealth meta-address.
   */
  claim_name: ({winner, name, stealth_meta_address}: {winner: string, name: string, stealth_meta_address: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a commit_bid transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Commit a sealed bid. `commitment` hides the bid amount; `deposit` is
   * transferred to the contract and must cover the bid revealed later.
   */
  commit_bid: ({bidder, name, commitment, deposit}: {bidder: string, name: string, commitment: Buffer, deposit: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a reveal_bid transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Reveal a previously committed bid by disclosing the amount and salt.
   */
  reveal_bid: ({bidder, name, amount, salt}: {bidder: string, name: string, amount: i128, salt: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_auction transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Read the auction state for a name, if any.
   */
  get_auction: ({name}: {name: string}, options?: MethodOptions) => Promise<AssembledTransaction<Option<Auction>>>

  /**
   * Construct and simulate a withdraw_bid transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Withdraw a losing (or unrevealed) bid deposit in full.
   */
  withdraw_bid: ({bidder, name}: {bidder: string, name: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a bulk_register transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Register multiple names in a single atomic transaction.
   * 
   * All names must be valid and not already taken. If any name fails,
   * the entire operation reverts.
   */
  bulk_register: ({owner, names, meta_addresses}: {owner: string, names: Array<string>, meta_addresses: Array<Buffer>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a init_auctions transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * One-time initialization of the premium-name auction system.
   * 
   * `admin` operates settlements per the runbook, `treasury` receives
   * winning bids, `token` is the payment asset (native XLM SAC on mainnet),
   * `reserve_price` is the minimum bid, and `commit_secs` / `reveal_secs`
   * are the phase durations for each auction. The 90-day premium window
   * starts at the ledger timestamp of this call.
   */
  init_auctions: ({admin, treasury, token, reserve_price, commit_secs, reveal_secs}: {admin: string, treasury: string, token: string, reserve_price: i128, commit_secs: u64, reveal_secs: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a init_multisig transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * One-time setup of the protocol-level governance signer set used to
   * authorise signer rotations.
   */
  init_multisig: ({signers, threshold}: {signers: Array<string>, threshold: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a start_auction transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Start a sealed-bid auction for a premium name (<= 4 chars, top-level).
   * Permissionless: anyone may open the auction for an eligible name.
   */
  start_auction: ({name}: {name: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a auction_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Read the auction configuration, if initialized.
   */
  auction_config: (options?: MethodOptions) => Promise<AssembledTransaction<Option<AuctionConfig>>>

  /**
   * Construct and simulate a settle_auction transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Settle an auction after the reveal phase: pays the winning bid to the
   * treasury and refunds the winner's excess deposit. Permissionless so
   * funds can never be trapped, operated by the admin per the runbook.
   */
  settle_auction: ({name}: {name: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a extend_name_ttl transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Extend TTL for persistent storage entries only.
   * Extend the TTL of a registered name to a future ledger.
   * This is a permissionless function that anyone can call.
   * Idempotent: calling twice in the same ledger has no additional effect.
   */
  extend_name_ttl: ({name, extend_to_ledger}: {name: string, extend_to_ledger: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a pending_rotation transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * The pending signer-rotation proposal, if any.
   */
  pending_rotation: (options?: MethodOptions) => Promise<AssembledTransaction<Option<RotationProposal>>>

  /**
   * Construct and simulate a update_on_behalf transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Update a name on behalf of an owner using a signed authorization.
   */
  update_on_behalf: ({owner, name, new_meta_address, signature, expiry}: {owner: string, name: string, new_meta_address: Buffer, signature: Buffer, expiry: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a release_on_behalf transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Release a name on behalf of an owner using a signed authorization.
   */
  release_on_behalf: ({owner, name, signature, expiry}: {owner: string, name: string, signature: Buffer, expiry: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a compute_commitment transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Compute the sealed-bid commitment for the given parameters.
   * 
   * Intended for off-chain use (simulation only): calling this in a real
   * transaction would leak the bid amount.
   */
  compute_commitment: ({name, bidder, amount, salt}: {name: string, bidder: string, amount: i128, salt: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Buffer>>

  /**
   * Construct and simulate a register_on_behalf transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Register a name on behalf of an owner using a signed authorization.
   */
  register_on_behalf: ({owner, name, stealth_meta_address, signature, expiry}: {owner: string, name: string, stealth_meta_address: Buffer, signature: Buffer, expiry: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a cancel_rotate_signers transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Cancel the pending rotation, clearing all of its state.
   */
  cancel_rotate_signers: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a approve_rotate_signers transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Approve the pending signer-rotation proposal.
   */
  approve_rotate_signers: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a execute_rotate_signers transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Execute the pending rotation once quorum is met and the timelock has
   * elapsed. Emits `SignersRotated`.
   */
  execute_rotate_signers: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a propose_rotate_signers transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Propose a new signer set + threshold behind the rotation timelock.
   * `caller` must be a current signer; the proposal is auto-approved by
   * `caller`. Rejects thresholds that could never reach quorum.
   */
  propose_rotate_signers: ({caller, new_signers, new_threshold}: {caller: string, new_signers: Array<string>, new_threshold: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a auctions_pending_settlement transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Auctions with a revealed winner that have not settled yet. While this
   * is non-zero, `execute_rotate_auction_admin` is blocked.
   */
  auctions_pending_settlement: (options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a cancel_rotate_auction_admin transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Cancel the pending auction-admin rotation, clearing all of its state.
   */
  cancel_rotate_auction_admin: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a approve_rotate_auction_admin transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Approve the pending auction-admin rotation proposal.
   */
  approve_rotate_auction_admin: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a execute_rotate_auction_admin transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Execute the pending auction-admin rotation once quorum is met and the
   * timelock has elapsed. Emits `AuctionAdminRotated(old, new)`.
   * 
   * Fails with `AuctionInProgress` while any auction is in its reveal or
   * settle phase, leaving the proposal intact so it can be retried after
   * settlement.
   */
  execute_rotate_auction_admin: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a propose_rotate_auction_admin transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Propose a new premium-auction admin behind the 7-day rotation
   * timelock, gated by the same governance signer set as
   * `propose_rotate_signers`. `caller` must be a current signer; the
   * proposal is auto-approved by `caller`.
   */
  propose_rotate_auction_admin: ({caller, new_admin}: {caller: string, new_admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a pending_auction_admin_rotation transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * The pending auction-admin rotation proposal, if any.
   */
  pending_auction_admin_rotation: (options?: MethodOptions) => Promise<AssembledTransaction<Option<AdminRotationProposal>>>

}
export class Client extends ContractClient {
  static async deploy<T = Client>(
    /** Options for initializing a Client as well as for calling a method, with extras specific to deploying. */
    options: MethodOptions &
      Omit<ContractClientOptions, "contractId"> & {
        /** The hash of the Wasm blob, which must already be installed on-chain. */
        wasmHash: Buffer | string;
        /** Salt used to generate the contract's ID. Passed through to {@link Operation.createCustomContract}. Default: random. */
        salt?: Buffer | Uint8Array;
        /** The format used to decode `wasmHash`, if it's provided as a string. */
        format?: "hex" | "base64";
      }
  ): Promise<AssembledTransaction<T>> {
    return ContractClient.deploy(null, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "AAAAAAAAAKdJbml0aWFsaXNlIHRoZSBjb250cmFjdCBieSBzdG9yaW5nIHRoZSBwYXVzZSBhZG1pbi4KCk11c3QgYmUgY2FsbGVkIGJlZm9yZSBgcGF1c2VgIC8gYHVucGF1c2VgLiBJZGVtcG90ZW50OiBjYWxsaW5nCm1vcmUgdGhhbiBvbmNlIGlzIGEgbm8tb3AgKHRoZSBmaXJzdCBhZG1pbiBzdGlja3MpLgAAAAAEaW5pdAAAAAEAAAAAAAAABWFkbWluAAAAAAAAEwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAACk5hbWVzRXJyb3IAAA==",
        "AAAAAAAAAKJQYXVzZSB0aGUgY29udHJhY3Qg4oCUIGFkbWluIG9ubHkuClByZXZlbnRzIGFsbCByZWdpc3RyYXRpb25zLCB1cGRhdGVzLCByZWxlYXNlcyBhbmQgVFRMIGV4dGVuc2lvbnMKd2hpbGUgcGF1c2VkLiBMb29rdXBzIChgcmVzb2x2ZWAsIGBuYW1lX29mYCkgcmVtYWluIGF2YWlsYWJsZS4AAAAAAAVwYXVzZQAAAAAAAAEAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAACk5hbWVzRXJyb3IAAA==",
        "AAAAAAAAAFBVcGRhdGUgdGhlIG1ldGEtYWRkcmVzcyBmb3IgYW4gZXhpc3RpbmcgbmFtZS4KT25seSB0aGUgY3VycmVudCBvd25lciBjYW4gdXBkYXRlLgAAAAZ1cGRhdGUAAAAAAAMAAAAAAAAABW93bmVyAAAAAAAAEwAAAAAAAAAEbmFtZQAAABAAAAAAAAAAEG5ld19tZXRhX2FkZHJlc3MAAAAOAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAKTmFtZXNFcnJvcgAA",
        "AAAAAAAAAD9SZXZlcnNlIGxvb2t1cDogZmluZCB0aGUgbmFtZSBmb3IgYSBnaXZlbiBzdGVhbHRoIG1ldGEtYWRkcmVzcy4AAAAAB25hbWVfb2YAAAAAAQAAAAAAAAAUc3RlYWx0aF9tZXRhX2FkZHJlc3MAAAAOAAAAAQAAA+kAAAAQAAAH0AAAAApOYW1lc0Vycm9yAAA=",
        "AAAAAAAAACpSZWxlYXNlIGEgbmFtZSwgbWFraW5nIGl0IGF2YWlsYWJsZSBhZ2Fpbi4AAAAAAAdyZWxlYXNlAAAAAAIAAAAAAAAABW93bmVyAAAAAAAAEwAAAAAAAAAEbmFtZQAAABAAAAABAAAD6QAAA+0AAAAAAAAH0AAAAApOYW1lc0Vycm9yAAA=",
        "AAAAAAAAALhSZXNvbHZlIGEgbmFtZSB0byBpdHMgc3RlYWx0aCBtZXRhLWFkZHJlc3MuCgpGb3IgYSBzdWJkb21haW4gKGBwYXltZW50cy5hbGljZWApIHJlc29sdXRpb24gd2Fsa3MgdG8gdGhlIHBhcmVudAooYGFsaWNlYCk6IGlmIHRoZSBwYXJlbnQgbm8gbG9uZ2VyIGV4aXN0cyB0aGUgc3ViZG9tYWluIGRvZXMgbm90IHJlc29sdmUuAAAAB3Jlc29sdmUAAAAAAQAAAAAAAAAEbmFtZQAAABAAAAABAAAD6QAAAA4AAAfQAAAACk5hbWVzRXJyb3IAAA==",
        "AAAAAAAAAC1DdXJyZW50IHByb3RvY29sLWxldmVsIGdvdmVybmFuY2Ugc2lnbmVyIHNldC4AAAAAAAAHc2lnbmVycwAAAAAAAAAAAQAAA+oAAAAT",
        "AAAAAAAAACRVbnBhdXNlIHRoZSBjb250cmFjdCDigJQgYWRtaW4gb25seS4AAAAHdW5wYXVzZQAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAAApOYW1lc0Vycm9yAAA=",
        "AAAAAAAAADFSZWdpc3RlciBhIG5hbWUgbWFwcGVkIHRvIGEgc3RlYWx0aCBtZXRhLWFkZHJlc3MuAAAAAAAACHJlZ2lzdGVyAAAAAwAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAAARuYW1lAAAAEAAAAAAAAAAUc3RlYWx0aF9tZXRhX2FkZHJlc3MAAAAOAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAKTmFtZXNFcnJvcgAA",
        "AAAAAAAAACdSZXR1cm5zIHRydWUgaWYgdGhlIGNvbnRyYWN0IGlzIHBhdXNlZC4AAAAACWlzX3BhdXNlZAAAAAAAAAAAAAABAAAAAQ==",
        "AAAAAAAAADNDdXJyZW50IHByb3RvY29sLWxldmVsIGdvdmVybmFuY2UgcXVvcnVtIHRocmVzaG9sZC4AAAAACXRocmVzaG9sZAAAAAAAAAAAAAABAAAABA==",
        "AAAAAgAAAA1TdG9yYWdlIGtleXMuAAAAAAAAAAAAAAdEYXRhS2V5AAAAAAsAAAABAAAAKU1hcHMgbmFtZSBoYXNoIChCeXRlc048MzI+KSB0byBOYW1lRW50cnkuAAAAAAAABE5hbWUAAAABAAAD7gAAACAAAAABAAAAPlJldmVyc2UgbG9va3VwOiBtZXRhLWFkZHJlc3MgaGFzaCAoQnl0ZXNOPDMyPikgdG8gbmFtZSBzdHJpbmcuAAAAAAAHUmV2ZXJzZQAAAAABAAAD7gAAACAAAAABAAAALVJlcGxheSBwcm90ZWN0aW9uIGZvciBzaWduZWQgb24tYmVoYWxmIGNhbGxzLgAAAAAAAAZSZXBsYXkAAAAAAAEAAAPuAAAAIAAAAAEAAAAbR3VhcmRpYW4gY29uZmlnIGZvciBhIG5hbWUuAAAAAAlHdWFyZGlhbnMAAAAAAAABAAAD7gAAACAAAAABAAAAJVBlbmRpbmcgcmVjb3ZlcnkgcHJvcG9zYWwgZm9yIGEgbmFtZS4AAAAAAAAIUmVjb3ZlcnkAAAABAAAD7gAAACAAAAAAAAAAFFBhdXNlIGFkbWluIGFkZHJlc3MuAAAABUFkbWluAAAAAAAAAAAAAB9XaGV0aGVyIHRoZSBjb250cmFjdCBpcyBwYXVzZWQuAAAAAAZQYXVzZWQAAAAAAAAAAAAuUHJvdG9jb2wtbGV2ZWwgZ292ZXJuYW5jZSBtdWx0aXNpZyBzaWduZXIgc2V0LgAAAAAAD011bHRpc2lnU2lnbmVycwAAAAAAAAAANFByb3RvY29sLWxldmVsIGdvdmVybmFuY2UgbXVsdGlzaWcgcXVvcnVtIHRocmVzaG9sZC4AAAARTXVsdGlzaWdUaHJlc2hvbGQAAAAAAAAAAAAAOFBlbmRpbmcgcHJvdG9jb2wtbGV2ZWwgc2lnbmVyLXJvdGF0aW9uIHByb3Bvc2FsLCBpZiBhbnkuAAAAD1BlbmRpbmdSb3RhdGlvbgAAAAAAAAAAMFBlbmRpbmcgYXVjdGlvbi1hZG1pbiByb3RhdGlvbiBwcm9wb3NhbCwgaWYgYW55LgAAABtQZW5kaW5nQXVjdGlvbkFkbWluUm90YXRpb24A",
        "AAAAAAAAAJRSZW5ldyAoZXh0ZW5kIFRUTCBmb3IpIG11bHRpcGxlIG5hbWVzIGluIGEgc2luZ2xlIGF0b21pYyB0cmFuc2FjdGlvbi4KCkFsbCBuYW1lcyBtdXN0IGV4aXN0LiBJZiBhbnkgbmFtZSBpcyBub3QgZm91bmQsIHRoZSBlbnRpcmUgb3BlcmF0aW9uCnJldmVydHMuAAAACmJ1bGtfcmVuZXcAAAAAAAIAAAAAAAAABW5hbWVzAAAAAAAD6gAAABAAAAAAAAAAEGV4dGVuZF90b19sZWRnZXIAAAAEAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAKTmFtZXNFcnJvcgAA",
        "AAAAAAAAAFZDbGFpbSBhIHdvbiBhdWN0aW9uOiByZWdpc3RlcnMgdGhlIG5hbWUgdG8gdGhlIHdpbm5lciB3aXRoIHRoZWlyCnN0ZWFsdGggbWV0YS1hZGRyZXNzLgAAAAAACmNsYWltX25hbWUAAAAAAAMAAAAAAAAABndpbm5lcgAAAAAAEwAAAAAAAAAEbmFtZQAAABAAAAAAAAAAFHN0ZWFsdGhfbWV0YV9hZGRyZXNzAAAADgAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADEF1Y3Rpb25FcnJvcg==",
        "AAAAAAAAAIdDb21taXQgYSBzZWFsZWQgYmlkLiBgY29tbWl0bWVudGAgaGlkZXMgdGhlIGJpZCBhbW91bnQ7IGBkZXBvc2l0YCBpcwp0cmFuc2ZlcnJlZCB0byB0aGUgY29udHJhY3QgYW5kIG11c3QgY292ZXIgdGhlIGJpZCByZXZlYWxlZCBsYXRlci4AAAAACmNvbW1pdF9iaWQAAAAAAAQAAAAAAAAABmJpZGRlcgAAAAAAEwAAAAAAAAAEbmFtZQAAABAAAAAAAAAACmNvbW1pdG1lbnQAAAAAA+4AAAAgAAAAAAAAAAdkZXBvc2l0AAAAAAsAAAABAAAD6QAAA+0AAAAAAAAH0AAAAAxBdWN0aW9uRXJyb3I=",
        "AAAAAAAAAERSZXZlYWwgYSBwcmV2aW91c2x5IGNvbW1pdHRlZCBiaWQgYnkgZGlzY2xvc2luZyB0aGUgYW1vdW50IGFuZCBzYWx0LgAAAApyZXZlYWxfYmlkAAAAAAAEAAAAAAAAAAZiaWRkZXIAAAAAABMAAAAAAAAABG5hbWUAAAAQAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAABHNhbHQAAAPuAAAAIAAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADEF1Y3Rpb25FcnJvcg==",
        "AAAAAAAAACpSZWFkIHRoZSBhdWN0aW9uIHN0YXRlIGZvciBhIG5hbWUsIGlmIGFueS4AAAAAAAtnZXRfYXVjdGlvbgAAAAABAAAAAAAAAARuYW1lAAAAEAAAAAEAAAPoAAAH0AAAAAdBdWN0aW9uAA==",
        "AAAAAQAAABhBIHJlZ2lzdGVyZWQgbmFtZSBlbnRyeS4AAAAAAAAACU5hbWVFbnRyeQAAAAAAAAQAAAAAAAAABG5hbWUAAAAQAAAAAAAAAAVvd25lcgAAAAAAABMAAACvRm9yIGEgc3ViZG9tYWluIChgc3ViLnBhcmVudGApLCB0aGUgbmFtZSBoYXNoIG9mIHRoZSBwYXJlbnQgbGFiZWwuCmBOb25lYCBmb3IgYSBmbGF0IHRvcC1sZXZlbCBuYW1lLiBFeGlzdGluZyBmbGF0IG5hbWVzIHJlZ2lzdGVyIHdpdGgKYE5vbmVgLCBzbyBwcmlvciBiZWhhdmlvdXIgaXMgcHJlc2VydmVkLgAAAAAGcGFyZW50AAAAAAPoAAAD7gAAACAAAAAAAAAAFHN0ZWFsdGhfbWV0YV9hZGRyZXNzAAAADg==",
        "AAAAAAAAADZXaXRoZHJhdyBhIGxvc2luZyAob3IgdW5yZXZlYWxlZCkgYmlkIGRlcG9zaXQgaW4gZnVsbC4AAAAAAAx3aXRoZHJhd19iaWQAAAACAAAAAAAAAAZiaWRkZXIAAAAAABMAAAAAAAAABG5hbWUAAAAQAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAMQXVjdGlvbkVycm9y",
        "AAAABAAAAAdFcnJvcnMuAAAAAAAAAAAKTmFtZXNFcnJvcgAAAAAAIgAAAAAAAAAJTmFtZVRha2VuAAAAAAAAAQAAAAAAAAAMTmFtZVRvb1Nob3J0AAAAAgAAAAAAAAALTmFtZVRvb0xvbmcAAAAAAwAAAAAAAAAUSW52YWxpZE5hbWVDaGFyYWN0ZXIAAAAEAAAAAAAAABJJbnZhbGlkTWV0YUFkZHJlc3MAAAAAAAUAAAAAAAAADE5hbWVOb3RGb3VuZAAAAAYAAAAAAAAACE5vdE93bmVyAAAABwAAAAAAAAAQU2lnbmF0dXJlRXhwaXJlZAAAAAgAAAAAAAAAD1NpZ25hdHVyZVJlcGxheQAAAAAJAAAAAAAAAA1JbnZhbGlkU2lnbmVyAAAAAAAACgAAAAAAAAALTm90R3VhcmRpYW4AAAAACwAAAAAAAAAKTm9Qcm9wb3NhbAAAAAAADAAAAAAAAAAVUHJvcG9zYWxBbHJlYWR5RXhpc3RzAAAAAAAADQAAAAAAAAAPQWxyZWFkeUFwcHJvdmVkAAAAAA4AAAAAAAAAD0RlbGF5Tm90RWxhcHNlZAAAAAAPAAAAAAAAAA9UaHJlc2hvbGROb3RNZXQAAAAAEAAAAAAAAAAQVG9vTWFueUd1YXJkaWFucwAAABEAAAAAAAAAEEludmFsaWRUaHJlc2hvbGQAAAASAAAAAAAAABNJbnZhbGlkRXh0ZW5kTGVkZ2VyAAAAABMAAAAAAAAADlBhcmVudE5vdEZvdW5kAAAAAAAUAAAAF1RoZSBjb250cmFjdCBpcyBwYXVzZWQuAAAAAAZQYXVzZWQAAAAAACAAAABAVGhlIHByb3RvY29sLWxldmVsIGdvdmVybmFuY2UgbXVsdGlzaWcgaGFzIG5vdCBiZWVuIGluaXRpYWxpc2VkLgAAABZNdWx0aXNpZ05vdEluaXRpYWxpemVkAAAAAAAVAAAARFRoZSBwcm90b2NvbC1sZXZlbCBnb3Zlcm5hbmNlIG11bHRpc2lnIGhhcyBhbHJlYWR5IGJlZW4gaW5pdGlhbGlzZWQuAAAAGk11bHRpc2lnQWxyZWFkeUluaXRpYWxpemVkAAAAAAAWAAAAPVRoZSBjYWxsZXIgaXMgbm90IGEgY3VycmVudCBwcm90b2NvbC1sZXZlbCBnb3Zlcm5hbmNlIHNpZ25lci4AAAAAAAAJTm90U2lnbmVyAAAAAAAAFwAAAC5BIHNpZ25lci1yb3RhdGlvbiBwcm9wb3NhbCBpcyBhbHJlYWR5IHBlbmRpbmcuAAAAAAAWUm90YXRpb25BbHJlYWR5UGVuZGluZwAAAAAAGAAAACdObyBzaWduZXItcm90YXRpb24gcHJvcG9zYWwgaXMgcGVuZGluZy4AAAAAEU5vUGVuZGluZ1JvdGF0aW9uAAAAAAAAGQAAADVUaGUgY2FsbGVyIGhhcyBhbHJlYWR5IGFwcHJvdmVkIHRoZSBwZW5kaW5nIHJvdGF0aW9uLgAAAAAAABdBbHJlYWR5QXBwcm92ZWRSb3RhdGlvbgAAAAAaAAAAPFRoZSBwZW5kaW5nIHJvdGF0aW9uIGhhcyBub3QgY29sbGVjdGVkIGVub3VnaCBhcHByb3ZhbHMgeWV0LgAAAAxRdW9ydW1Ob3RNZXQAAAAbAAAAKlRoZSByb3RhdGlvbiB0aW1lbG9jayBoYXMgbm90IGVsYXBzZWQgeWV0LgAAAAAAElRpbWVsb2NrTm90RWxhcHNlZAAAAAAAHAAAAAAAAAALTmFtZVRvb0RlZXAAAAAAHQAAAAAAAAARQnVsa0xpbWl0RXhjZWVkZWQAAAAAAAAeAAAAfVRoZSBuYW1lIGlzIHByZW1pdW0gKDw9IDQgY2hhcnMpIGFuZCB0aGUgYXVjdGlvbiB3aW5kb3cgaXMgYWN0aXZlLCBzbwppdCBjYW4gb25seSBiZSBvYnRhaW5lZCB0aHJvdWdoIHRoZSBzZWFsZWQtYmlkIGF1Y3Rpb24uAAAAAAAAFlByZW1pdW1BdWN0aW9uUmVxdWlyZWQAAAAAAB8AAABXVGhlIGF1Y3Rpb24gc3Vic3lzdGVtIGhhcyBub3QgYmVlbiBpbml0aWFsaXNlZCwgc28gdGhlcmUgaXMgbm8gYXVjdGlvbgphZG1pbiB0byByb3RhdGUuAAAAABZBdWN0aW9uc05vdEluaXRpYWxpemVkAAAAAAZAAAAAc0FuIGF1Y3Rpb24gaGFzIGEgcmV2ZWFsZWQgd2lubmVyIGFuZCBoYXMgbm90IHNldHRsZWQgeWV0LCBzbyB0aGUKYXVjdGlvbiBhZG1pbiBjYW5ub3QgYmUgcm90YXRlZCBvdXQgZnJvbSB1bmRlciBpdC4AAAAAEUF1Y3Rpb25JblByb2dyZXNzAAAAAAAGQQ==",
        "AAAAAAAAAJhSZWdpc3RlciBtdWx0aXBsZSBuYW1lcyBpbiBhIHNpbmdsZSBhdG9taWMgdHJhbnNhY3Rpb24uCgpBbGwgbmFtZXMgbXVzdCBiZSB2YWxpZCBhbmQgbm90IGFscmVhZHkgdGFrZW4uIElmIGFueSBuYW1lIGZhaWxzLAp0aGUgZW50aXJlIG9wZXJhdGlvbiByZXZlcnRzLgAAAA1idWxrX3JlZ2lzdGVyAAAAAAAAAwAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAAAVuYW1lcwAAAAAAA+oAAAAQAAAAAAAAAA5tZXRhX2FkZHJlc3NlcwAAAAAD6gAAAA4AAAABAAAD6QAAA+0AAAAAAAAH0AAAAApOYW1lc0Vycm9yAAA=",
        "AAAAAAAAAX1PbmUtdGltZSBpbml0aWFsaXphdGlvbiBvZiB0aGUgcHJlbWl1bS1uYW1lIGF1Y3Rpb24gc3lzdGVtLgoKYGFkbWluYCBvcGVyYXRlcyBzZXR0bGVtZW50cyBwZXIgdGhlIHJ1bmJvb2ssIGB0cmVhc3VyeWAgcmVjZWl2ZXMKd2lubmluZyBiaWRzLCBgdG9rZW5gIGlzIHRoZSBwYXltZW50IGFzc2V0IChuYXRpdmUgWExNIFNBQyBvbiBtYWlubmV0KSwKYHJlc2VydmVfcHJpY2VgIGlzIHRoZSBtaW5pbXVtIGJpZCwgYW5kIGBjb21taXRfc2Vjc2AgLyBgcmV2ZWFsX3NlY3NgCmFyZSB0aGUgcGhhc2UgZHVyYXRpb25zIGZvciBlYWNoIGF1Y3Rpb24uIFRoZSA5MC1kYXkgcHJlbWl1bSB3aW5kb3cKc3RhcnRzIGF0IHRoZSBsZWRnZXIgdGltZXN0YW1wIG9mIHRoaXMgY2FsbC4AAAAAAAANaW5pdF9hdWN0aW9ucwAAAAAAAAYAAAAAAAAABWFkbWluAAAAAAAAEwAAAAAAAAAIdHJlYXN1cnkAAAATAAAAAAAAAAV0b2tlbgAAAAAAABMAAAAAAAAADXJlc2VydmVfcHJpY2UAAAAAAAALAAAAAAAAAAtjb21taXRfc2VjcwAAAAAGAAAAAAAAAAtyZXZlYWxfc2VjcwAAAAAGAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAMQXVjdGlvbkVycm9y",
        "AAAAAAAAAF5PbmUtdGltZSBzZXR1cCBvZiB0aGUgcHJvdG9jb2wtbGV2ZWwgZ292ZXJuYW5jZSBzaWduZXIgc2V0IHVzZWQgdG8KYXV0aG9yaXNlIHNpZ25lciByb3RhdGlvbnMuAAAAAAANaW5pdF9tdWx0aXNpZwAAAAAAAAIAAAAAAAAAB3NpZ25lcnMAAAAD6gAAABMAAAAAAAAACXRocmVzaG9sZAAAAAAAAAQAAAABAAAD6QAAA+0AAAAAAAAH0AAAAApOYW1lc0Vycm9yAAA=",
        "AAAAAAAAAIhTdGFydCBhIHNlYWxlZC1iaWQgYXVjdGlvbiBmb3IgYSBwcmVtaXVtIG5hbWUgKDw9IDQgY2hhcnMsIHRvcC1sZXZlbCkuClBlcm1pc3Npb25sZXNzOiBhbnlvbmUgbWF5IG9wZW4gdGhlIGF1Y3Rpb24gZm9yIGFuIGVsaWdpYmxlIG5hbWUuAAAADXN0YXJ0X2F1Y3Rpb24AAAAAAAABAAAAAAAAAARuYW1lAAAAEAAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADEF1Y3Rpb25FcnJvcg==",
        "AAAAAAAAAC9SZWFkIHRoZSBhdWN0aW9uIGNvbmZpZ3VyYXRpb24sIGlmIGluaXRpYWxpemVkLgAAAAAOYXVjdGlvbl9jb25maWcAAAAAAAAAAAABAAAD6AAAB9AAAAANQXVjdGlvbkNvbmZpZwAAAA==",
        "AAAAAAAAAMxTZXR0bGUgYW4gYXVjdGlvbiBhZnRlciB0aGUgcmV2ZWFsIHBoYXNlOiBwYXlzIHRoZSB3aW5uaW5nIGJpZCB0byB0aGUKdHJlYXN1cnkgYW5kIHJlZnVuZHMgdGhlIHdpbm5lcidzIGV4Y2VzcyBkZXBvc2l0LiBQZXJtaXNzaW9ubGVzcyBzbwpmdW5kcyBjYW4gbmV2ZXIgYmUgdHJhcHBlZCwgb3BlcmF0ZWQgYnkgdGhlIGFkbWluIHBlciB0aGUgcnVuYm9vay4AAAAOc2V0dGxlX2F1Y3Rpb24AAAAAAAEAAAAAAAAABG5hbWUAAAAQAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAMQXVjdGlvbkVycm9y",
        "AAAAAAAAAOZFeHRlbmQgVFRMIGZvciBwZXJzaXN0ZW50IHN0b3JhZ2UgZW50cmllcyBvbmx5LgpFeHRlbmQgdGhlIFRUTCBvZiBhIHJlZ2lzdGVyZWQgbmFtZSB0byBhIGZ1dHVyZSBsZWRnZXIuClRoaXMgaXMgYSBwZXJtaXNzaW9ubGVzcyBmdW5jdGlvbiB0aGF0IGFueW9uZSBjYW4gY2FsbC4KSWRlbXBvdGVudDogY2FsbGluZyB0d2ljZSBpbiB0aGUgc2FtZSBsZWRnZXIgaGFzIG5vIGFkZGl0aW9uYWwgZWZmZWN0LgAAAAAAD2V4dGVuZF9uYW1lX3R0bAAAAAACAAAAAAAAAARuYW1lAAAAEAAAAAAAAAAQZXh0ZW5kX3RvX2xlZGdlcgAAAAQAAAABAAAD6QAAA+0AAAAAAAAH0AAAAApOYW1lc0Vycm9yAAA=",
        "AAAAAAAAAC1UaGUgcGVuZGluZyBzaWduZXItcm90YXRpb24gcHJvcG9zYWwsIGlmIGFueS4AAAAAAAAQcGVuZGluZ19yb3RhdGlvbgAAAAAAAAABAAAD6AAAB9AAAAAQUm90YXRpb25Qcm9wb3NhbA==",
        "AAAAAAAAAEFVcGRhdGUgYSBuYW1lIG9uIGJlaGFsZiBvZiBhbiBvd25lciB1c2luZyBhIHNpZ25lZCBhdXRob3JpemF0aW9uLgAAAAAAABB1cGRhdGVfb25fYmVoYWxmAAAABQAAAAAAAAAFb3duZXIAAAAAAAATAAAAAAAAAARuYW1lAAAAEAAAAAAAAAAQbmV3X21ldGFfYWRkcmVzcwAAAA4AAAAAAAAACXNpZ25hdHVyZQAAAAAAA+4AAABAAAAAAAAAAAZleHBpcnkAAAAAAAYAAAABAAAD6QAAA+0AAAAAAAAH0AAAAApOYW1lc0Vycm9yAAA=",
        "AAAAAQAAACJHdWFyZGlhbiBjb25maWd1cmF0aW9uIGZvciBhIG5hbWUuAAAAAAAAAAAADkd1YXJkaWFuQ29uZmlnAAAAAAACAAAAAAAAAAlndWFyZGlhbnMAAAAAAAPqAAAAEwAAAAAAAAAJdGhyZXNob2xkAAAAAAAABA==",
        "AAAAAAAAAEJSZWxlYXNlIGEgbmFtZSBvbiBiZWhhbGYgb2YgYW4gb3duZXIgdXNpbmcgYSBzaWduZWQgYXV0aG9yaXphdGlvbi4AAAAAABFyZWxlYXNlX29uX2JlaGFsZgAAAAAAAAQAAAAAAAAABW93bmVyAAAAAAAAEwAAAAAAAAAEbmFtZQAAABAAAAAAAAAACXNpZ25hdHVyZQAAAAAAA+4AAABAAAAAAAAAAAZleHBpcnkAAAAAAAYAAAABAAAD6QAAA+0AAAAAAAAH0AAAAApOYW1lc0Vycm9yAAA=",
        "AAAAAAAAAKhDb21wdXRlIHRoZSBzZWFsZWQtYmlkIGNvbW1pdG1lbnQgZm9yIHRoZSBnaXZlbiBwYXJhbWV0ZXJzLgoKSW50ZW5kZWQgZm9yIG9mZi1jaGFpbiB1c2UgKHNpbXVsYXRpb24gb25seSk6IGNhbGxpbmcgdGhpcyBpbiBhIHJlYWwKdHJhbnNhY3Rpb24gd291bGQgbGVhayB0aGUgYmlkIGFtb3VudC4AAAASY29tcHV0ZV9jb21taXRtZW50AAAAAAAEAAAAAAAAAARuYW1lAAAAEAAAAAAAAAAGYmlkZGVyAAAAAAATAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAABHNhbHQAAAPuAAAAIAAAAAEAAAPuAAAAIA==",
        "AAAAAAAAAENSZWdpc3RlciBhIG5hbWUgb24gYmVoYWxmIG9mIGFuIG93bmVyIHVzaW5nIGEgc2lnbmVkIGF1dGhvcml6YXRpb24uAAAAABJyZWdpc3Rlcl9vbl9iZWhhbGYAAAAAAAUAAAAAAAAABW93bmVyAAAAAAAAEwAAAAAAAAAEbmFtZQAAABAAAAAAAAAAFHN0ZWFsdGhfbWV0YV9hZGRyZXNzAAAADgAAAAAAAAAJc2lnbmF0dXJlAAAAAAAD7gAAAEAAAAAAAAAABmV4cGlyeQAAAAAABgAAAAEAAAPpAAAD7QAAAAAAAAfQAAAACk5hbWVzRXJyb3IAAA==",
        "AAAAAQAAABxBIHBlbmRpbmcgcmVjb3ZlcnkgcHJvcG9zYWwuAAAAAAAAABBSZWNvdmVyeVByb3Bvc2FsAAAABAAAAAAAAAAJYXBwcm92YWxzAAAAAAAD6gAAABMAAAAAAAAAEG5ld19tZXRhX2FkZHJlc3MAAAAOAAAAAAAAAAluZXdfb3duZXIAAAAAAAATAAAAAAAAAAtwcm9wb3NlZF9hdAAAAAAE",
        "AAAAAAAAADdDYW5jZWwgdGhlIHBlbmRpbmcgcm90YXRpb24sIGNsZWFyaW5nIGFsbCBvZiBpdHMgc3RhdGUuAAAAABVjYW5jZWxfcm90YXRlX3NpZ25lcnMAAAAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAAApOYW1lc0Vycm9yAAA=",
        "AAAAAAAAAC1BcHByb3ZlIHRoZSBwZW5kaW5nIHNpZ25lci1yb3RhdGlvbiBwcm9wb3NhbC4AAAAAAAAWYXBwcm92ZV9yb3RhdGVfc2lnbmVycwAAAAAAAQAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAKTmFtZXNFcnJvcgAA",
        "AAAAAAAAAGVFeGVjdXRlIHRoZSBwZW5kaW5nIHJvdGF0aW9uIG9uY2UgcXVvcnVtIGlzIG1ldCBhbmQgdGhlIHRpbWVsb2NrIGhhcwplbGFwc2VkLiBFbWl0cyBgU2lnbmVyc1JvdGF0ZWRgLgAAAAAAABZleGVjdXRlX3JvdGF0ZV9zaWduZXJzAAAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAAApOYW1lc0Vycm9yAAA=",
        "AAAAAAAAAMJQcm9wb3NlIGEgbmV3IHNpZ25lciBzZXQgKyB0aHJlc2hvbGQgYmVoaW5kIHRoZSByb3RhdGlvbiB0aW1lbG9jay4KYGNhbGxlcmAgbXVzdCBiZSBhIGN1cnJlbnQgc2lnbmVyOyB0aGUgcHJvcG9zYWwgaXMgYXV0by1hcHByb3ZlZCBieQpgY2FsbGVyYC4gUmVqZWN0cyB0aHJlc2hvbGRzIHRoYXQgY291bGQgbmV2ZXIgcmVhY2ggcXVvcnVtLgAAAAAAFnByb3Bvc2Vfcm90YXRlX3NpZ25lcnMAAAAAAAMAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAALbmV3X3NpZ25lcnMAAAAD6gAAABMAAAAAAAAADW5ld190aHJlc2hvbGQAAAAAAAAEAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAKTmFtZXNFcnJvcgAA",
        "AAAAAAAAAH1BdWN0aW9ucyB3aXRoIGEgcmV2ZWFsZWQgd2lubmVyIHRoYXQgaGF2ZSBub3Qgc2V0dGxlZCB5ZXQuIFdoaWxlIHRoaXMKaXMgbm9uLXplcm8sIGBleGVjdXRlX3JvdGF0ZV9hdWN0aW9uX2FkbWluYCBpcyBibG9ja2VkLgAAAAAAABthdWN0aW9uc19wZW5kaW5nX3NldHRsZW1lbnQAAAAAAAAAAAEAAAAE",
        "AAAAAAAAAEVDYW5jZWwgdGhlIHBlbmRpbmcgYXVjdGlvbi1hZG1pbiByb3RhdGlvbiwgY2xlYXJpbmcgYWxsIG9mIGl0cyBzdGF0ZS4AAAAAAAAbY2FuY2VsX3JvdGF0ZV9hdWN0aW9uX2FkbWluAAAAAAEAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAACk5hbWVzRXJyb3IAAA==",
        "AAAAAAAAADRBcHByb3ZlIHRoZSBwZW5kaW5nIGF1Y3Rpb24tYWRtaW4gcm90YXRpb24gcHJvcG9zYWwuAAAAHGFwcHJvdmVfcm90YXRlX2F1Y3Rpb25fYWRtaW4AAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAAApOYW1lc0Vycm9yAAA=",
        "AAAAAAAAARlFeGVjdXRlIHRoZSBwZW5kaW5nIGF1Y3Rpb24tYWRtaW4gcm90YXRpb24gb25jZSBxdW9ydW0gaXMgbWV0IGFuZCB0aGUKdGltZWxvY2sgaGFzIGVsYXBzZWQuIEVtaXRzIGBBdWN0aW9uQWRtaW5Sb3RhdGVkKG9sZCwgbmV3KWAuCgpGYWlscyB3aXRoIGBBdWN0aW9uSW5Qcm9ncmVzc2Agd2hpbGUgYW55IGF1Y3Rpb24gaXMgaW4gaXRzIHJldmVhbCBvcgpzZXR0bGUgcGhhc2UsIGxlYXZpbmcgdGhlIHByb3Bvc2FsIGludGFjdCBzbyBpdCBjYW4gYmUgcmV0cmllZCBhZnRlcgpzZXR0bGVtZW50LgAAAAAAABxleGVjdXRlX3JvdGF0ZV9hdWN0aW9uX2FkbWluAAAAAQAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAKTmFtZXNFcnJvcgAA",
        "AAAAAAAAANpQcm9wb3NlIGEgbmV3IHByZW1pdW0tYXVjdGlvbiBhZG1pbiBiZWhpbmQgdGhlIDctZGF5IHJvdGF0aW9uCnRpbWVsb2NrLCBnYXRlZCBieSB0aGUgc2FtZSBnb3Zlcm5hbmNlIHNpZ25lciBzZXQgYXMKYHByb3Bvc2Vfcm90YXRlX3NpZ25lcnNgLiBgY2FsbGVyYCBtdXN0IGJlIGEgY3VycmVudCBzaWduZXI7IHRoZQpwcm9wb3NhbCBpcyBhdXRvLWFwcHJvdmVkIGJ5IGBjYWxsZXJgLgAAAAAAHHByb3Bvc2Vfcm90YXRlX2F1Y3Rpb25fYWRtaW4AAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAACW5ld19hZG1pbgAAAAAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAAApOYW1lc0Vycm9yAAA=",
        "AAAAAAAAADRUaGUgcGVuZGluZyBhdWN0aW9uLWFkbWluIHJvdGF0aW9uIHByb3Bvc2FsLCBpZiBhbnkuAAAAHnBlbmRpbmdfYXVjdGlvbl9hZG1pbl9yb3RhdGlvbgAAAAAAAAAAAAEAAAPoAAAH0AAAABVBZG1pblJvdGF0aW9uUHJvcG9zYWwAAAA=",
        "AAAAAQAAAB9TdGF0ZSBvZiBhIHNpbmdsZSBuYW1lIGF1Y3Rpb24uAAAAAAAAAAAHQXVjdGlvbgAAAAAGAAAARVRpbWVzdGFtcCBhdCB3aGljaCB0aGUgY29tbWl0IHBoYXNlIGVuZHMgYW5kIHRoZSByZXZlYWwgcGhhc2UgYmVnaW5zLgAAAAAAAApjb21taXRfZW5kAAAAAAAGAAAAI0hpZ2hlc3QgcmV2ZWFsZWQgYmlkIGFtb3VudCBzbyBmYXIuAAAAAA5oaWdoZXN0X2Ftb3VudAAAAAAACwAAAD9IaWdoZXN0IHJldmVhbGVkIGJpZGRlciBzbyBmYXIuIFRpZXMgZ28gdG8gdGhlIGVhcmxpZXN0IHJldmVhbC4AAAAADmhpZ2hlc3RfYmlkZGVyAAAAAAPoAAAAEwAAAAAAAAAEbmFtZQAAABAAAABEVGltZXN0YW1wIGF0IHdoaWNoIHRoZSByZXZlYWwgcGhhc2UgZW5kcyBhbmQgc2V0dGxlbWVudCBpcyBwb3NzaWJsZS4AAAAKcmV2ZWFsX2VuZAAAAAAABgAAAAAAAAAHc2V0dGxlZAAAAAAB",
        "AAAAAQAAAA1BIHNlYWxlZCBiaWQuAAAAAAAAAAAAAAlTZWFsZWRCaWQAAAAAAAADAAAAOHNoYTI1NiBjb21taXRtZW50IGJpbmRpbmcgYmlkZGVyLCBuYW1lLCBhbW91bnQgYW5kIHNhbHQuAAAACmNvbW1pdG1lbnQAAAAAA+4AAAAgAAAAOlRva2VucyBsb2NrZWQgaW4gdGhlIGNvbnRyYWN0OyByZWZ1bmRlZCBpbiBmdWxsIHRvIGxvc2Vycy4AAAAAAAdkZXBvc2l0AAAAAAsAAAAAAAAACHJldmVhbGVkAAAAAQ==",
        "AAAAAgAAACdTdG9yYWdlIGtleXMgZm9yIHRoZSBhdWN0aW9uIHN1YnN5c3RlbS4AAAAAAAAAAApBdWN0aW9uS2V5AAAAAAAEAAAAAAAAAClBdWN0aW9uIGNvbmZpZ3VyYXRpb24gKGluc3RhbmNlIHN0b3JhZ2UpLgAAAAAAAAZDb25maWcAAAAAAAEAAAAcQXVjdGlvbiBzdGF0ZSBwZXIgbmFtZSBoYXNoLgAAAAdBdWN0aW9uAAAAAAEAAAPuAAAAIAAAAAEAAAAjU2VhbGVkIGJpZCBwZXIgKG5hbWUgaGFzaCwgYmlkZGVyKS4AAAAAA0JpZAAAAAACAAAD7gAAACAAAAATAAAAAAAAAHVDb3VudCBvZiBhdWN0aW9ucyB3aXRoIGEgcmV2ZWFsZWQgd2lubmVyIHRoYXQgaGF2ZSBub3Qgc2V0dGxlZCB5ZXQKKGluc3RhbmNlIHN0b3JhZ2UpLiBHdWFyZHMgYXVjdGlvbi1hZG1pbiByb3RhdGlvbi4AAAAAAAASUGVuZGluZ1NldHRsZW1lbnRzAAA=",
        "AAAABAAAAEZBdWN0aW9uIGVycm9ycy4gQ29kZXMgc3RhcnQgYXQgMTAwIHRvIHN0YXkgZGlzam9pbnQgZnJvbSBgTmFtZXNFcnJvcmAuAAAAAAAAAAAADEF1Y3Rpb25FcnJvcgAAABgAAAAAAAAADk5vdEluaXRpYWxpemVkAAAAAABkAAAAAAAAABJBbHJlYWR5SW5pdGlhbGl6ZWQAAAAAAGUAAAAAAAAADUludmFsaWRDb25maWcAAAAAAABmAAAAAAAAAAxXaW5kb3dDbG9zZWQAAABnAAAAAAAAAA5Ob3RQcmVtaXVtTmFtZQAAAAAAaAAAAAAAAAAVTmFtZUFscmVhZHlSZWdpc3RlcmVkAAAAAAAAaQAAAAAAAAANQXVjdGlvbkV4aXN0cwAAAAAAAGoAAAAAAAAACU5vQXVjdGlvbgAAAAAAAGsAAAAAAAAAD0NvbW1pdFBoYXNlT3ZlcgAAAABsAAAAAAAAABBBbHJlYWR5Q29tbWl0dGVkAAAAbQAAAAAAAAATRGVwb3NpdEJlbG93UmVzZXJ2ZQAAAABuAAAAAAAAABRSZXZlYWxQaGFzZU5vdEFjdGl2ZQAAAG8AAAAAAAAABU5vQmlkAAAAAAAAcAAAAAAAAAAPQWxyZWFkeVJldmVhbGVkAAAAAHEAAAAAAAAAEkNvbW1pdG1lbnRNaXNtYXRjaAAAAAAAcgAAAAAAAAAPQmlkQmVsb3dSZXNlcnZlAAAAAHMAAAAAAAAAEUJpZEV4Y2VlZHNEZXBvc2l0AAAAAAAAdAAAAAAAAAASUmV2ZWFsUGhhc2VOb3RPdmVyAAAAAAB1AAAAAAAAAA5BbHJlYWR5U2V0dGxlZAAAAAAAdgAAAAAAAAAKTm90U2V0dGxlZAAAAAAAdwAAAAAAAAAJTm90V2lubmVyAAAAAAAAeAAAAAAAAAAUV2lubmVyQ2Fubm90V2l0aGRyYXcAAAB5AAAAAAAAABJJbnZhbGlkTWV0YUFkZHJlc3MAAAAAAHoAAAAAAAAAElJlZ2lzdHJhdGlvbkZhaWxlZAAAAAAAew==",
        "AAAAAQAAADJBdWN0aW9uIGNvbmZpZ3VyYXRpb24sIHNldCBvbmNlIGF0IGluaXRpYWxpemF0aW9uLgAAAAAAAAAAAA1BdWN0aW9uQ29uZmlnAAAAAAAABwAAAC9PcGVyYXRvciB0aGF0IHJ1bnMgc2V0dGxlbWVudHMgcGVyIHRoZSBydW5ib29rLgAAAAAFYWRtaW4AAAAAAAATAAAAOUR1cmF0aW9uIG9mIHRoZSBjb21taXQgcGhhc2Ugb2YgZWFjaCBhdWN0aW9uLCBpbiBzZWNvbmRzLgAAAAAAAAtjb21taXRfc2VjcwAAAAAGAAAAZkxlZGdlciB0aW1lc3RhbXAgYXQgaW5pdGlhbGl6YXRpb247IHRoZSBwcmVtaXVtIHdpbmRvdyBydW5zIGZvcgpgUFJFTUlVTV9XSU5ET1dfU0VDU2AgZnJvbSB0aGlzIHBvaW50LgAAAAAAC2xhdW5jaF90aW1lAAAAAAYAAAA4TWluaW11bSBiaWQ7IGRlcG9zaXRzIG11c3QgYWxzbyBiZSBhdCBsZWFzdCB0aGlzIGFtb3VudC4AAAANcmVzZXJ2ZV9wcmljZQAAAAAAAAsAAAA5RHVyYXRpb24gb2YgdGhlIHJldmVhbCBwaGFzZSBvZiBlYWNoIGF1Y3Rpb24sIGluIHNlY29uZHMuAAAAAAAAC3JldmVhbF9zZWNzAAAAAAYAAAAqUGF5bWVudCB0b2tlbiAobmF0aXZlIFhMTSBTQUMgb24gbWFpbm5ldCkuAAAAAAAFdG9rZW4AAAAAAAATAAAAFlJlY2VpdmVzIHdpbm5pbmcgYmlkcy4AAAAAAAh0cmVhc3VyeQAAABM=",
        "AAAAAQAAACNBIHBlbmRpbmcgc2lnbmVyLXJvdGF0aW9uIHByb3Bvc2FsLgAAAAAAAAAAEFJvdGF0aW9uUHJvcG9zYWwAAAAEAAAAAAAAAAlhcHByb3ZhbHMAAAAAAAPqAAAAEwAAAAAAAAANZXhlY3V0YWJsZV9hdAAAAAAAAAYAAAAAAAAAC25ld19zaWduZXJzAAAAA+oAAAATAAAAAAAAAA1uZXdfdGhyZXNob2xkAAAAAAAABA==",
        "AAAAAQAAAMhBIHBlbmRpbmcgYXVjdGlvbi1hZG1pbiByb3RhdGlvbiBwcm9wb3NhbC4gTWlycm9ycyBgUm90YXRpb25Qcm9wb3NhbGAg4oCUCnNhbWUgc2lnbmVyIHNldCwgc2FtZSBxdW9ydW0sIHNhbWUgdGltZWxvY2sg4oCUIGJ1dCBjYXJyaWVzIHRoZSBpbmNvbWluZwphdWN0aW9uIG9wZXJhdG9yIGFkZHJlc3MgaW5zdGVhZCBvZiBhIG5ldyBzaWduZXIgc2V0LgAAAAAAAAAVQWRtaW5Sb3RhdGlvblByb3Bvc2FsAAAAAAAAAwAAAAAAAAAJYXBwcm92YWxzAAAAAAAD6gAAABMAAAAAAAAADWV4ZWN1dGFibGVfYXQAAAAAAAAGAAAAAAAAAAluZXdfYWRtaW4AAAAAAAAT",
        "AAAAAQAAAKpXcmFpdGggUHJvdG9jb2wgc3RhbmRhcmQgbWV0cmljIGV2ZW50IHNjaGVtYS4KCkFsbCBXcmFpdGggY29udHJhY3RzIGVtaXQgbWV0cmljIGV2ZW50cyB1c2luZyB0aGlzIHN0cnVjdHVyZSB0byBlbmFibGUKc3RhbmRhcmRpemVkIG9mZi1jaGFpbiBvYnNlcnZhYmlsaXR5IGFuZCBtb25pdG9yaW5nLgAAAAAAAAAAABFXcmFpdGhNZXRyaWNFdmVudAAAAAAAAAQAAABAQ29udHJhY3QgaWRlbnRpZmllciAoZS5nLiwgInN0ZWFsdGgtcmVnaXN0cnkiLCAic3RlYWx0aC1zZW5kZXIiKQAAAAhjb250cmFjdAAAABEAAABLT3B0aW9uYWwgZGltZW5zaW9ucyBmb3IgZmlsdGVyaW5nL2dyb3VwaW5nIChlLmcuLCB0b2tlbl9hZGRyZXNzLCBzY2hlbWVfaWQpAAAAAApkaW1lbnNpb25zAAAAAAPqAAAD7QAAAAIAAAARAAAAAAAAADNNZXRyaWMgbmFtZSAoZS5nLiwgInJlZ2lzdGVyX2NvdW50IiwgInNlbmRfdm9sdW1lIikAAAAAC21ldHJpY19uYW1lAAAAABEAAAAbTnVtZXJpYyB2YWx1ZSBvZiB0aGUgbWV0cmljAAAAAAV2YWx1ZQAAAAAAAAs=" ]),
      options
    )
  }
  public readonly fromJSON = {
    init: this.txFromJSON<Result<void>>,
        pause: this.txFromJSON<Result<void>>,
        update: this.txFromJSON<Result<void>>,
        name_of: this.txFromJSON<Result<string>>,
        release: this.txFromJSON<Result<void>>,
        resolve: this.txFromJSON<Result<Buffer>>,
        signers: this.txFromJSON<Array<string>>,
        unpause: this.txFromJSON<Result<void>>,
        register: this.txFromJSON<Result<void>>,
        is_paused: this.txFromJSON<boolean>,
        threshold: this.txFromJSON<u32>,
        bulk_renew: this.txFromJSON<Result<void>>,
        claim_name: this.txFromJSON<Result<void>>,
        commit_bid: this.txFromJSON<Result<void>>,
        reveal_bid: this.txFromJSON<Result<void>>,
        get_auction: this.txFromJSON<Option<Auction>>,
        withdraw_bid: this.txFromJSON<Result<void>>,
        bulk_register: this.txFromJSON<Result<void>>,
        init_auctions: this.txFromJSON<Result<void>>,
        init_multisig: this.txFromJSON<Result<void>>,
        start_auction: this.txFromJSON<Result<void>>,
        auction_config: this.txFromJSON<Option<AuctionConfig>>,
        settle_auction: this.txFromJSON<Result<void>>,
        extend_name_ttl: this.txFromJSON<Result<void>>,
        pending_rotation: this.txFromJSON<Option<RotationProposal>>,
        update_on_behalf: this.txFromJSON<Result<void>>,
        release_on_behalf: this.txFromJSON<Result<void>>,
        compute_commitment: this.txFromJSON<Buffer>,
        register_on_behalf: this.txFromJSON<Result<void>>,
        cancel_rotate_signers: this.txFromJSON<Result<void>>,
        approve_rotate_signers: this.txFromJSON<Result<void>>,
        execute_rotate_signers: this.txFromJSON<Result<void>>,
        propose_rotate_signers: this.txFromJSON<Result<void>>,
        auctions_pending_settlement: this.txFromJSON<u32>,
        cancel_rotate_auction_admin: this.txFromJSON<Result<void>>,
        approve_rotate_auction_admin: this.txFromJSON<Result<void>>,
        execute_rotate_auction_admin: this.txFromJSON<Result<void>>,
        propose_rotate_auction_admin: this.txFromJSON<Result<void>>,
        pending_auction_admin_rotation: this.txFromJSON<Option<AdminRotationProposal>>
  }
}