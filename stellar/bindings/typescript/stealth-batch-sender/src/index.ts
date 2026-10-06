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
export type DataKey = {tag: "Admin", values: void} | {tag: "Announcer", values: void} | {tag: "AssetPolicy", values: void} | {tag: "Paused", values: void} | {tag: "MultisigSigners", values: void} | {tag: "MultisigThreshold", values: void} | {tag: "PendingRotation", values: void};


/**
 * A single stealth transfer within a batch.
 * Mirrors the EVM WraithSender batchSendETH/batchSendERC20 structure.
 */
export interface Transfer {
  /**
 * Token amount (in the asset's base unit)
 */
amount: i128;
  /**
 * Ephemeral public key for the recipient to scan with.
 * Must be exactly 32 bytes so it can be forwarded to the announcer.
 */
ephemeral_pub_key: Buffer;
  /**
 * Announcement metadata whose first byte is the view tag
 * (`view_tag_bucket = metadata[0] as u32` under metadata_kind = 1).
 */
metadata: Buffer;
  /**
 * Pre-computed stealth address (recipient)
 */
stealth_address: string;
}

/**
 * Errors that the batch-sender contract can produce.
 * 
 * Codes are allocated from the `1300-1399` range reserved for
 * `stealth-batch-sender` in `ERRORS.md`'s code-allocation policy.
 */
export const BatchSenderError = {
  /**
   * The contract has already been initialised.
   */
  1300: {message:"AlreadyInitialized"},
  /**
   * The contract has not been initialised yet.
   */
  1301: {message:"NotInitialized"},
  /**
   * The batch contains no transfers.
   */
  1302: {message:"EmptyBatch"},
  /**
   * The batch exceeds `MAX_BATCH_SIZE`.
   */
  1303: {message:"BatchTooLarge"},
  /**
   * A transfer amount was zero or negative.
   */
  1304: {message:"NonPositiveAmount"},
  /**
   * A transfer's ephemeral public key was empty or not 32 bytes.
   */
  1305: {message:"EmptyEphemeralKey"},
  /**
   * The contract is paused.
   */
  1306: {message:"Paused"},
  /**
   * The asset is not allowed by the configured asset policy.
   */
  1307: {message:"AssetNotAllowed"},
  /**
   * The governance multisig has not been initialised.
   */
  1308: {message:"MultisigNotInitialized"},
  /**
   * The governance multisig has already been initialised.
   */
  1309: {message:"MultisigAlreadyInitialized"},
  /**
   * The caller is not a current governance signer.
   */
  1310: {message:"NotSigner"},
  /**
   * The requested threshold is invalid (zero, or greater than signer count).
   */
  1311: {message:"InvalidThreshold"},
  /**
   * A signer-rotation proposal is already pending.
   */
  1312: {message:"RotationAlreadyPending"},
  /**
   * No signer-rotation proposal is pending.
   */
  1313: {message:"NoPendingRotation"},
  /**
   * The caller has already approved the pending rotation.
   */
  1314: {message:"AlreadyApprovedRotation"},
  /**
   * The pending rotation has not collected enough approvals yet.
   */
  1315: {message:"QuorumNotMet"},
  /**
   * The rotation timelock has not elapsed yet.
   */
  1316: {message:"TimelockNotElapsed"}
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
   * Initialise the contract by storing the pause admin, the announcer
   * address, and an optional asset policy. Idempotent: a second call
   * returns `AlreadyInitialized` rather than overwriting the config.
   * 
   * Must be called before `batch_send`.
   */
  init: ({admin, announcer, asset_policy}: {admin: string, announcer: string, asset_policy: Option<string>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a pause transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Pause the contract — admin only. Prevents `batch_send` while paused.
   */
  pause: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a signers transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Current governance signer set.
   */
  signers: (options?: MethodOptions) => Promise<AssembledTransaction<Array<string>>>

  /**
   * Construct and simulate a unpause transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Unpause the contract — admin only.
   */
  unpause: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a is_paused transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns true if the contract is paused.
   */
  is_paused: (options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a threshold transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Current governance quorum threshold.
   */
  threshold: (options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a batch_send transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Atomically send `asset` tokens from `from` to N pre-computed stealth
   * addresses in a single transaction.
   * 
   * # All-or-nothing semantics
   * Soroban reverts every state change made during a failed invocation —
   * whether the failure is a panic or a returned contract error — so a
   * rejected batch never leaves partial transfers behind.
   * 
   * # Resource budget
   * Capped at MAX_BATCH_SIZE (100) transfers. This keeps instruction usage
   * well under Soroban's per-transaction limit while still being ~100x more
   * efficient than N individual stealth-sender::send calls (one auth, one
   * ledger round-trip vs N).
   */
  batch_send: ({from, transfers, asset}: {from: string, transfers: Array<Transfer>, asset: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a init_multisig transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * One-time setup of the governance signer set used to authorise signer
   * rotations. Independent of `init` — does not gate `batch_send`.
   */
  init_multisig: ({signers, threshold}: {signers: Array<string>, threshold: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a max_batch_size transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Query the maximum allowed batch size.
   */
  max_batch_size: (options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a pending_rotation transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * The pending signer-rotation proposal, if any.
   */
  pending_rotation: (options?: MethodOptions) => Promise<AssembledTransaction<Option<RotationProposal>>>

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
      new ContractSpec([ "AAAAAAAAAOhJbml0aWFsaXNlIHRoZSBjb250cmFjdCBieSBzdG9yaW5nIHRoZSBwYXVzZSBhZG1pbiwgdGhlIGFubm91bmNlcgphZGRyZXNzLCBhbmQgYW4gb3B0aW9uYWwgYXNzZXQgcG9saWN5LiBJZGVtcG90ZW50OiBhIHNlY29uZCBjYWxsCnJldHVybnMgYEFscmVhZHlJbml0aWFsaXplZGAgcmF0aGVyIHRoYW4gb3ZlcndyaXRpbmcgdGhlIGNvbmZpZy4KCk11c3QgYmUgY2FsbGVkIGJlZm9yZSBgYmF0Y2hfc2VuZGAuAAAABGluaXQAAAADAAAAAAAAAAVhZG1pbgAAAAAAABMAAAAAAAAACWFubm91bmNlcgAAAAAAABMAAAAAAAAADGFzc2V0X3BvbGljeQAAA+gAAAATAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAQQmF0Y2hTZW5kZXJFcnJvcg==",
        "AAAAAAAAAEZQYXVzZSB0aGUgY29udHJhY3Qg4oCUIGFkbWluIG9ubHkuIFByZXZlbnRzIGBiYXRjaF9zZW5kYCB3aGlsZSBwYXVzZWQuAAAAAAAFcGF1c2UAAAAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAABBCYXRjaFNlbmRlckVycm9y",
        "AAAAAAAAAB5DdXJyZW50IGdvdmVybmFuY2Ugc2lnbmVyIHNldC4AAAAAAAdzaWduZXJzAAAAAAAAAAABAAAD6gAAABM=",
        "AAAAAAAAACRVbnBhdXNlIHRoZSBjb250cmFjdCDigJQgYWRtaW4gb25seS4AAAAHdW5wYXVzZQAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAABBCYXRjaFNlbmRlckVycm9y",
        "AAAAAAAAACdSZXR1cm5zIHRydWUgaWYgdGhlIGNvbnRyYWN0IGlzIHBhdXNlZC4AAAAACWlzX3BhdXNlZAAAAAAAAAAAAAABAAAAAQ==",
        "AAAAAAAAACRDdXJyZW50IGdvdmVybmFuY2UgcXVvcnVtIHRocmVzaG9sZC4AAAAJdGhyZXNob2xkAAAAAAAAAAAAAAEAAAAE",
        "AAAAAgAAAA1TdG9yYWdlIGtleXMuAAAAAAAAAAAAAAdEYXRhS2V5AAAAAAcAAAAAAAAAFFBhdXNlIGFkbWluIGFkZHJlc3MuAAAABUFkbWluAAAAAAAAAAAAANdBZGRyZXNzIG9mIHRoZSBkZXBsb3llZCBTdGVhbHRoQW5ub3VuY2VyIGNvbnRyYWN0LCByZWNvcmRlZCBhdCBpbml0CnRpbWUuIEludm9rZWQgYnkgYGJhdGNoX3NlbmRgIGZvciBlYWNoIHRyYW5zZmVyIHNvIGFubm91bmNlbWVudHMgdXNlCnRoZSB2MiA0LXRvcGljIGxheW91dCAoYGFubm91bmNlYCwgc2NoZW1lX2lkLCB2aWV3X3RhZ19idWNrZXQsIG1ldGFkYXRhX2tpbmQpLgAAAAAJQW5ub3VuY2VyAAAAAAAAAAAAAC5PcHRpb25hbCBhZGRyZXNzIG9mIHRoZSBhc3NldCBwb2xpY3kgY29udHJhY3QuAAAAAAALQXNzZXRQb2xpY3kAAAAAAAAAAB9XaGV0aGVyIHRoZSBjb250cmFjdCBpcyBwYXVzZWQuAAAAAAZQYXVzZWQAAAAAAAAAAAAfR292ZXJuYW5jZSBtdWx0aXNpZyBzaWduZXIgc2V0LgAAAAAPTXVsdGlzaWdTaWduZXJzAAAAAAAAAAAlR292ZXJuYW5jZSBtdWx0aXNpZyBxdW9ydW0gdGhyZXNob2xkLgAAAAAAABFNdWx0aXNpZ1RocmVzaG9sZAAAAAAAAAAAAAApUGVuZGluZyBzaWduZXItcm90YXRpb24gcHJvcG9zYWwsIGlmIGFueS4AAAAAAAAPUGVuZGluZ1JvdGF0aW9uAA==",
        "AAAAAAAAAkZBdG9taWNhbGx5IHNlbmQgYGFzc2V0YCB0b2tlbnMgZnJvbSBgZnJvbWAgdG8gTiBwcmUtY29tcHV0ZWQgc3RlYWx0aAphZGRyZXNzZXMgaW4gYSBzaW5nbGUgdHJhbnNhY3Rpb24uCgojIEFsbC1vci1ub3RoaW5nIHNlbWFudGljcwpTb3JvYmFuIHJldmVydHMgZXZlcnkgc3RhdGUgY2hhbmdlIG1hZGUgZHVyaW5nIGEgZmFpbGVkIGludm9jYXRpb24g4oCUCndoZXRoZXIgdGhlIGZhaWx1cmUgaXMgYSBwYW5pYyBvciBhIHJldHVybmVkIGNvbnRyYWN0IGVycm9yIOKAlCBzbyBhCnJlamVjdGVkIGJhdGNoIG5ldmVyIGxlYXZlcyBwYXJ0aWFsIHRyYW5zZmVycyBiZWhpbmQuCgojIFJlc291cmNlIGJ1ZGdldApDYXBwZWQgYXQgTUFYX0JBVENIX1NJWkUgKDEwMCkgdHJhbnNmZXJzLiBUaGlzIGtlZXBzIGluc3RydWN0aW9uIHVzYWdlCndlbGwgdW5kZXIgU29yb2JhbidzIHBlci10cmFuc2FjdGlvbiBsaW1pdCB3aGlsZSBzdGlsbCBiZWluZyB+MTAweCBtb3JlCmVmZmljaWVudCB0aGFuIE4gaW5kaXZpZHVhbCBzdGVhbHRoLXNlbmRlcjo6c2VuZCBjYWxscyAob25lIGF1dGgsIG9uZQpsZWRnZXIgcm91bmQtdHJpcCB2cyBOKS4AAAAAAApiYXRjaF9zZW5kAAAAAAADAAAAAAAAAARmcm9tAAAAEwAAAAAAAAAJdHJhbnNmZXJzAAAAAAAD6gAAB9AAAAAIVHJhbnNmZXIAAAAAAAAABWFzc2V0AAAAAAAAEwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAEEJhdGNoU2VuZGVyRXJyb3I=",
        "AAAAAQAAAG1BIHNpbmdsZSBzdGVhbHRoIHRyYW5zZmVyIHdpdGhpbiBhIGJhdGNoLgpNaXJyb3JzIHRoZSBFVk0gV3JhaXRoU2VuZGVyIGJhdGNoU2VuZEVUSC9iYXRjaFNlbmRFUkMyMCBzdHJ1Y3R1cmUuAAAAAAAAAAAAAAhUcmFuc2ZlcgAAAAQAAAAnVG9rZW4gYW1vdW50IChpbiB0aGUgYXNzZXQncyBiYXNlIHVuaXQpAAAAAAZhbW91bnQAAAAAAAsAAAB2RXBoZW1lcmFsIHB1YmxpYyBrZXkgZm9yIHRoZSByZWNpcGllbnQgdG8gc2NhbiB3aXRoLgpNdXN0IGJlIGV4YWN0bHkgMzIgYnl0ZXMgc28gaXQgY2FuIGJlIGZvcndhcmRlZCB0byB0aGUgYW5ub3VuY2VyLgAAAAAAEWVwaGVtZXJhbF9wdWJfa2V5AAAAAAAADgAAAHhBbm5vdW5jZW1lbnQgbWV0YWRhdGEgd2hvc2UgZmlyc3QgYnl0ZSBpcyB0aGUgdmlldyB0YWcKKGB2aWV3X3RhZ19idWNrZXQgPSBtZXRhZGF0YVswXSBhcyB1MzJgIHVuZGVyIG1ldGFkYXRhX2tpbmQgPSAxKS4AAAAIbWV0YWRhdGEAAAAOAAAAKFByZS1jb21wdXRlZCBzdGVhbHRoIGFkZHJlc3MgKHJlY2lwaWVudCkAAAAPc3RlYWx0aF9hZGRyZXNzAAAAABM=",
        "AAAAAAAAAIVPbmUtdGltZSBzZXR1cCBvZiB0aGUgZ292ZXJuYW5jZSBzaWduZXIgc2V0IHVzZWQgdG8gYXV0aG9yaXNlIHNpZ25lcgpyb3RhdGlvbnMuIEluZGVwZW5kZW50IG9mIGBpbml0YCDigJQgZG9lcyBub3QgZ2F0ZSBgYmF0Y2hfc2VuZGAuAAAAAAAADWluaXRfbXVsdGlzaWcAAAAAAAACAAAAAAAAAAdzaWduZXJzAAAAA+oAAAATAAAAAAAAAAl0aHJlc2hvbGQAAAAAAAAEAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAQQmF0Y2hTZW5kZXJFcnJvcg==",
        "AAAAAAAAACVRdWVyeSB0aGUgbWF4aW11bSBhbGxvd2VkIGJhdGNoIHNpemUuAAAAAAAADm1heF9iYXRjaF9zaXplAAAAAAAAAAAAAQAAAAQ=",
        "AAAAAAAAAC1UaGUgcGVuZGluZyBzaWduZXItcm90YXRpb24gcHJvcG9zYWwsIGlmIGFueS4AAAAAAAAQcGVuZGluZ19yb3RhdGlvbgAAAAAAAAABAAAD6AAAB9AAAAAQUm90YXRpb25Qcm9wb3NhbA==",
        "AAAABAAAAK9FcnJvcnMgdGhhdCB0aGUgYmF0Y2gtc2VuZGVyIGNvbnRyYWN0IGNhbiBwcm9kdWNlLgoKQ29kZXMgYXJlIGFsbG9jYXRlZCBmcm9tIHRoZSBgMTMwMC0xMzk5YCByYW5nZSByZXNlcnZlZCBmb3IKYHN0ZWFsdGgtYmF0Y2gtc2VuZGVyYCBpbiBgRVJST1JTLm1kYCdzIGNvZGUtYWxsb2NhdGlvbiBwb2xpY3kuAAAAAAAAAAAQQmF0Y2hTZW5kZXJFcnJvcgAAABEAAAAqVGhlIGNvbnRyYWN0IGhhcyBhbHJlYWR5IGJlZW4gaW5pdGlhbGlzZWQuAAAAAAASQWxyZWFkeUluaXRpYWxpemVkAAAAAAUUAAAAKlRoZSBjb250cmFjdCBoYXMgbm90IGJlZW4gaW5pdGlhbGlzZWQgeWV0LgAAAAAADk5vdEluaXRpYWxpemVkAAAAAAUVAAAAIFRoZSBiYXRjaCBjb250YWlucyBubyB0cmFuc2ZlcnMuAAAACkVtcHR5QmF0Y2gAAAAABRYAAAAjVGhlIGJhdGNoIGV4Y2VlZHMgYE1BWF9CQVRDSF9TSVpFYC4AAAAADUJhdGNoVG9vTGFyZ2UAAAAAAAUXAAAAJ0EgdHJhbnNmZXIgYW1vdW50IHdhcyB6ZXJvIG9yIG5lZ2F0aXZlLgAAAAARTm9uUG9zaXRpdmVBbW91bnQAAAAAAAUYAAAAPEEgdHJhbnNmZXIncyBlcGhlbWVyYWwgcHVibGljIGtleSB3YXMgZW1wdHkgb3Igbm90IDMyIGJ5dGVzLgAAABFFbXB0eUVwaGVtZXJhbEtleQAAAAAABRkAAAAXVGhlIGNvbnRyYWN0IGlzIHBhdXNlZC4AAAAABlBhdXNlZAAAAAAFGgAAADhUaGUgYXNzZXQgaXMgbm90IGFsbG93ZWQgYnkgdGhlIGNvbmZpZ3VyZWQgYXNzZXQgcG9saWN5LgAAAA9Bc3NldE5vdEFsbG93ZWQAAAAFGwAAADFUaGUgZ292ZXJuYW5jZSBtdWx0aXNpZyBoYXMgbm90IGJlZW4gaW5pdGlhbGlzZWQuAAAAAAAAFk11bHRpc2lnTm90SW5pdGlhbGl6ZWQAAAAABRwAAAA1VGhlIGdvdmVybmFuY2UgbXVsdGlzaWcgaGFzIGFscmVhZHkgYmVlbiBpbml0aWFsaXNlZC4AAAAAAAAaTXVsdGlzaWdBbHJlYWR5SW5pdGlhbGl6ZWQAAAAABR0AAAAuVGhlIGNhbGxlciBpcyBub3QgYSBjdXJyZW50IGdvdmVybmFuY2Ugc2lnbmVyLgAAAAAACU5vdFNpZ25lcgAAAAAABR4AAABIVGhlIHJlcXVlc3RlZCB0aHJlc2hvbGQgaXMgaW52YWxpZCAoemVybywgb3IgZ3JlYXRlciB0aGFuIHNpZ25lciBjb3VudCkuAAAAEEludmFsaWRUaHJlc2hvbGQAAAUfAAAALkEgc2lnbmVyLXJvdGF0aW9uIHByb3Bvc2FsIGlzIGFscmVhZHkgcGVuZGluZy4AAAAAABZSb3RhdGlvbkFscmVhZHlQZW5kaW5nAAAAAAUgAAAAJ05vIHNpZ25lci1yb3RhdGlvbiBwcm9wb3NhbCBpcyBwZW5kaW5nLgAAAAARTm9QZW5kaW5nUm90YXRpb24AAAAAAAUhAAAANVRoZSBjYWxsZXIgaGFzIGFscmVhZHkgYXBwcm92ZWQgdGhlIHBlbmRpbmcgcm90YXRpb24uAAAAAAAAF0FscmVhZHlBcHByb3ZlZFJvdGF0aW9uAAAABSIAAAA8VGhlIHBlbmRpbmcgcm90YXRpb24gaGFzIG5vdCBjb2xsZWN0ZWQgZW5vdWdoIGFwcHJvdmFscyB5ZXQuAAAADFF1b3J1bU5vdE1ldAAABSMAAAAqVGhlIHJvdGF0aW9uIHRpbWVsb2NrIGhhcyBub3QgZWxhcHNlZCB5ZXQuAAAAAAASVGltZWxvY2tOb3RFbGFwc2VkAAAAAAUk",
        "AAAAAAAAADdDYW5jZWwgdGhlIHBlbmRpbmcgcm90YXRpb24sIGNsZWFyaW5nIGFsbCBvZiBpdHMgc3RhdGUuAAAAABVjYW5jZWxfcm90YXRlX3NpZ25lcnMAAAAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAABBCYXRjaFNlbmRlckVycm9y",
        "AAAAAAAAAC1BcHByb3ZlIHRoZSBwZW5kaW5nIHNpZ25lci1yb3RhdGlvbiBwcm9wb3NhbC4AAAAAAAAWYXBwcm92ZV9yb3RhdGVfc2lnbmVycwAAAAAAAQAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAQQmF0Y2hTZW5kZXJFcnJvcg==",
        "AAAAAAAAAGVFeGVjdXRlIHRoZSBwZW5kaW5nIHJvdGF0aW9uIG9uY2UgcXVvcnVtIGlzIG1ldCBhbmQgdGhlIHRpbWVsb2NrIGhhcwplbGFwc2VkLiBFbWl0cyBgU2lnbmVyc1JvdGF0ZWRgLgAAAAAAABZleGVjdXRlX3JvdGF0ZV9zaWduZXJzAAAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAABBCYXRjaFNlbmRlckVycm9y",
        "AAAAAAAAAMJQcm9wb3NlIGEgbmV3IHNpZ25lciBzZXQgKyB0aHJlc2hvbGQgYmVoaW5kIHRoZSByb3RhdGlvbiB0aW1lbG9jay4KYGNhbGxlcmAgbXVzdCBiZSBhIGN1cnJlbnQgc2lnbmVyOyB0aGUgcHJvcG9zYWwgaXMgYXV0by1hcHByb3ZlZCBieQpgY2FsbGVyYC4gUmVqZWN0cyB0aHJlc2hvbGRzIHRoYXQgY291bGQgbmV2ZXIgcmVhY2ggcXVvcnVtLgAAAAAAFnByb3Bvc2Vfcm90YXRlX3NpZ25lcnMAAAAAAAMAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAALbmV3X3NpZ25lcnMAAAAD6gAAABMAAAAAAAAADW5ld190aHJlc2hvbGQAAAAAAAAEAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAQQmF0Y2hTZW5kZXJFcnJvcg==",
        "AAAAAQAAACNBIHBlbmRpbmcgc2lnbmVyLXJvdGF0aW9uIHByb3Bvc2FsLgAAAAAAAAAAEFJvdGF0aW9uUHJvcG9zYWwAAAAEAAAAAAAAAAlhcHByb3ZhbHMAAAAAAAPqAAAAEwAAAAAAAAANZXhlY3V0YWJsZV9hdAAAAAAAAAYAAAAAAAAAC25ld19zaWduZXJzAAAAA+oAAAATAAAAAAAAAA1uZXdfdGhyZXNob2xkAAAAAAAABA==",
        "AAAAAQAAAKpXcmFpdGggUHJvdG9jb2wgc3RhbmRhcmQgbWV0cmljIGV2ZW50IHNjaGVtYS4KCkFsbCBXcmFpdGggY29udHJhY3RzIGVtaXQgbWV0cmljIGV2ZW50cyB1c2luZyB0aGlzIHN0cnVjdHVyZSB0byBlbmFibGUKc3RhbmRhcmRpemVkIG9mZi1jaGFpbiBvYnNlcnZhYmlsaXR5IGFuZCBtb25pdG9yaW5nLgAAAAAAAAAAABFXcmFpdGhNZXRyaWNFdmVudAAAAAAAAAQAAABAQ29udHJhY3QgaWRlbnRpZmllciAoZS5nLiwgInN0ZWFsdGgtcmVnaXN0cnkiLCAic3RlYWx0aC1zZW5kZXIiKQAAAAhjb250cmFjdAAAABEAAABLT3B0aW9uYWwgZGltZW5zaW9ucyBmb3IgZmlsdGVyaW5nL2dyb3VwaW5nIChlLmcuLCB0b2tlbl9hZGRyZXNzLCBzY2hlbWVfaWQpAAAAAApkaW1lbnNpb25zAAAAAAPqAAAD7QAAAAIAAAARAAAAAAAAADNNZXRyaWMgbmFtZSAoZS5nLiwgInJlZ2lzdGVyX2NvdW50IiwgInNlbmRfdm9sdW1lIikAAAAAC21ldHJpY19uYW1lAAAAABEAAAAbTnVtZXJpYyB2YWx1ZSBvZiB0aGUgbWV0cmljAAAAAAV2YWx1ZQAAAAAAAAs=" ]),
      options
    )
  }
  public readonly fromJSON = {
    init: this.txFromJSON<Result<void>>,
        pause: this.txFromJSON<Result<void>>,
        signers: this.txFromJSON<Array<string>>,
        unpause: this.txFromJSON<Result<void>>,
        is_paused: this.txFromJSON<boolean>,
        threshold: this.txFromJSON<u32>,
        batch_send: this.txFromJSON<Result<void>>,
        init_multisig: this.txFromJSON<Result<void>>,
        max_batch_size: this.txFromJSON<u32>,
        pending_rotation: this.txFromJSON<Option<RotationProposal>>,
        cancel_rotate_signers: this.txFromJSON<Result<void>>,
        approve_rotate_signers: this.txFromJSON<Result<void>>,
        execute_rotate_signers: this.txFromJSON<Result<void>>,
        propose_rotate_signers: this.txFromJSON<Result<void>>
  }
}