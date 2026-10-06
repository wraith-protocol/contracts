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
export type DataKey = {tag: "Announcer", values: void} | {tag: "AssetPolicy", values: void} | {tag: "FeeRecipient", values: void} | {tag: "FeeBasisPoints", values: void} | {tag: "Admin", values: void} | {tag: "Paused", values: void} | {tag: "MultisigSigners", values: void} | {tag: "MultisigThreshold", values: void} | {tag: "PendingRotation", values: void};

/**
 * Errors that the sender contract can produce.
 */
export const SenderError = {
  /**
   * The contract has already been initialised.
   */
  1: {message:"AlreadyInitialized"},
  /**
   * The contract has not been initialised yet.
   */
  2: {message:"NotInitialized"},
  /**
   * The batch input vectors have mismatched lengths.
   */
  3: {message:"LengthMismatch"},
  /**
   * The token is not allowed by the asset policy.
   */
  4: {message:"TokenNotAllowed"},
  /**
   * The fee configuration is invalid (e.g. fee > 50 bps, or fee > 0 with no recipient).
   */
  5: {message:"InvalidFeeConfig"},
  /**
   * The contract is paused.
   */
  16: {message:"Paused"},
  /**
   * The batch withdrawal exceeds the supported size cap.
   */
  6: {message:"BatchTooLarge"},
  /**
   * The governance multisig has not been initialised.
   */
  7: {message:"MultisigNotInitialized"},
  /**
   * The governance multisig has already been initialised.
   */
  8: {message:"MultisigAlreadyInitialized"},
  /**
   * The caller is not a current governance signer.
   */
  9: {message:"NotSigner"},
  /**
   * The requested threshold is invalid (zero, or greater than signer count).
   */
  10: {message:"InvalidThreshold"},
  /**
   * A signer-rotation proposal is already pending.
   */
  11: {message:"RotationAlreadyPending"},
  /**
   * No signer-rotation proposal is pending.
   */
  12: {message:"NoPendingRotation"},
  /**
   * The caller has already approved the pending rotation.
   */
  13: {message:"AlreadyApprovedRotation"},
  /**
   * The pending rotation has not collected enough approvals yet.
   */
  14: {message:"QuorumNotMet"},
  /**
   * The rotation timelock has not elapsed yet.
   */
  15: {message:"TimelockNotElapsed"}
}


/**
 * A single withdrawal entry for batched asset exits.
 */
export interface WithdrawalEntry {
  /**
 * The amount to transfer in the token's base unit.
 */
amount: i128;
  /**
 * The destination address for the withdrawal.
 */
to: string;
  /**
 * The token contract to withdraw from or to transfer through.
 */
token: string;
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
   * Initialise the contract by storing the announcer address, optional asset policy,
   * and optional protocol fee configuration.
   * 
   * Must be called exactly once before any `send` or `batch_send`.
   */
  init: ({announcer, asset_policy, fee_recipient, fee_basis_points, admin}: {announcer: string, asset_policy: Option<string>, fee_recipient: Option<string>, fee_basis_points: u32, admin: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a send transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Transfer tokens to a stealth address and emit an announcement.
   * 
   * # Arguments
   * * `sender`            - The address sending funds (must authorise).
   * * `token`             - SAC token contract address (works for native XLM too).
   * * `amount`            - Amount of tokens to transfer.
   * * `scheme_id`         - Stealth address scheme identifier.
   * * `stealth_address`   - The derived one-time stealth address.
   * * `ephemeral_pub_key` - Ephemeral public key for the recipient to scan.
   * * `metadata`          - Extra data (e.g. view tag).
   */
  send: ({sender, token, amount, scheme_id, stealth_address, ephemeral_pub_key, metadata}: {sender: string, token: string, amount: i128, scheme_id: u32, stealth_address: string, ephemeral_pub_key: Buffer, metadata: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a pause transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Pause the contract — admin only.
   * Prevents all sends and batch_sends while paused.
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
   * Batch version of `send` — transfers tokens to multiple stealth addresses
   * and emits an announcement for each.
   * 
   * All input vectors must have the same length.
   */
  batch_send: ({sender, token, scheme_id, stealth_addresses, ephemeral_pub_keys, metadatas, amounts}: {sender: string, token: string, scheme_id: u32, stealth_addresses: Array<string>, ephemeral_pub_keys: Array<Buffer>, metadatas: Array<Buffer>, amounts: Array<i128>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a init_multisig transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * One-time setup of the governance signer set used to authorise signer
   * rotations. Independent of `init` — does not gate `send`/`batch_send`.
   */
  init_multisig: ({signers, threshold}: {signers: Array<string>, threshold: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a withdraw_many transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Withdraw assets to multiple destinations in a single atomic transaction.
   * 
   * The batch is capped at 30 entries. If any single entry cannot be
   * processed, the entire batch aborts and no state changes are retained.
   */
  withdraw_many: ({withdrawer, entries}: {withdrawer: string, entries: Array<WithdrawalEntry>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

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
      new ContractSpec([ "AAAAAAAAALlJbml0aWFsaXNlIHRoZSBjb250cmFjdCBieSBzdG9yaW5nIHRoZSBhbm5vdW5jZXIgYWRkcmVzcywgb3B0aW9uYWwgYXNzZXQgcG9saWN5LAphbmQgb3B0aW9uYWwgcHJvdG9jb2wgZmVlIGNvbmZpZ3VyYXRpb24uCgpNdXN0IGJlIGNhbGxlZCBleGFjdGx5IG9uY2UgYmVmb3JlIGFueSBgc2VuZGAgb3IgYGJhdGNoX3NlbmRgLgAAAAAAAARpbml0AAAABQAAAAAAAAAJYW5ub3VuY2VyAAAAAAAAEwAAAAAAAAAMYXNzZXRfcG9saWN5AAAD6AAAABMAAAAAAAAADWZlZV9yZWNpcGllbnQAAAAAAAPoAAAAEwAAAAAAAAAQZmVlX2Jhc2lzX3BvaW50cwAAAAQAAAAAAAAABWFkbWluAAAAAAAAEwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAC1NlbmRlckVycm9yAA==",
        "AAAAAAAAAglUcmFuc2ZlciB0b2tlbnMgdG8gYSBzdGVhbHRoIGFkZHJlc3MgYW5kIGVtaXQgYW4gYW5ub3VuY2VtZW50LgoKIyBBcmd1bWVudHMKKiBgc2VuZGVyYCAgICAgICAgICAgIC0gVGhlIGFkZHJlc3Mgc2VuZGluZyBmdW5kcyAobXVzdCBhdXRob3Jpc2UpLgoqIGB0b2tlbmAgICAgICAgICAgICAgLSBTQUMgdG9rZW4gY29udHJhY3QgYWRkcmVzcyAod29ya3MgZm9yIG5hdGl2ZSBYTE0gdG9vKS4KKiBgYW1vdW50YCAgICAgICAgICAgIC0gQW1vdW50IG9mIHRva2VucyB0byB0cmFuc2Zlci4KKiBgc2NoZW1lX2lkYCAgICAgICAgIC0gU3RlYWx0aCBhZGRyZXNzIHNjaGVtZSBpZGVudGlmaWVyLgoqIGBzdGVhbHRoX2FkZHJlc3NgICAgLSBUaGUgZGVyaXZlZCBvbmUtdGltZSBzdGVhbHRoIGFkZHJlc3MuCiogYGVwaGVtZXJhbF9wdWJfa2V5YCAtIEVwaGVtZXJhbCBwdWJsaWMga2V5IGZvciB0aGUgcmVjaXBpZW50IHRvIHNjYW4uCiogYG1ldGFkYXRhYCAgICAgICAgICAtIEV4dHJhIGRhdGEgKGUuZy4gdmlldyB0YWcpLgAAAAAAAARzZW5kAAAABwAAAAAAAAAGc2VuZGVyAAAAAAATAAAAAAAAAAV0b2tlbgAAAAAAABMAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAAJc2NoZW1lX2lkAAAAAAAABAAAAAAAAAAPc3RlYWx0aF9hZGRyZXNzAAAAABMAAAAAAAAAEWVwaGVtZXJhbF9wdWJfa2V5AAAAAAAD7gAAACAAAAAAAAAACG1ldGFkYXRhAAAADgAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAC1NlbmRlckVycm9yAA==",
        "AAAAAAAAAFNQYXVzZSB0aGUgY29udHJhY3Qg4oCUIGFkbWluIG9ubHkuClByZXZlbnRzIGFsbCBzZW5kcyBhbmQgYmF0Y2hfc2VuZHMgd2hpbGUgcGF1c2VkLgAAAAAFcGF1c2UAAAAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAAAtTZW5kZXJFcnJvcgA=",
        "AAAAAAAAAB5DdXJyZW50IGdvdmVybmFuY2Ugc2lnbmVyIHNldC4AAAAAAAdzaWduZXJzAAAAAAAAAAABAAAD6gAAABM=",
        "AAAAAAAAACRVbnBhdXNlIHRoZSBjb250cmFjdCDigJQgYWRtaW4gb25seS4AAAAHdW5wYXVzZQAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAAAtTZW5kZXJFcnJvcgA=",
        "AAAAAAAAACdSZXR1cm5zIHRydWUgaWYgdGhlIGNvbnRyYWN0IGlzIHBhdXNlZC4AAAAACWlzX3BhdXNlZAAAAAAAAAAAAAABAAAAAQ==",
        "AAAAAAAAACRDdXJyZW50IGdvdmVybmFuY2UgcXVvcnVtIHRocmVzaG9sZC4AAAAJdGhyZXNob2xkAAAAAAAAAAAAAAEAAAAE",
        "AAAAAgAAAA1TdG9yYWdlIGtleXMuAAAAAAAAAAAAAAdEYXRhS2V5AAAAAAkAAAAAAAAANlRoZSBhZGRyZXNzIG9mIHRoZSBkZXBsb3llZCBTdGVhbHRoQW5ub3VuY2VyIGNvbnRyYWN0LgAAAAAACUFubm91bmNlcgAAAAAAAAAAAAAuT3B0aW9uYWwgYWRkcmVzcyBvZiB0aGUgYXNzZXQgcG9saWN5IGNvbnRyYWN0LgAAAAAAC0Fzc2V0UG9saWN5AAAAAAAAAAAvT3B0aW9uYWwgYWRkcmVzcyBvZiB0aGUgcHJvdG9jb2wgZmVlIHJlY2lwaWVudC4AAAAADEZlZVJlY2lwaWVudAAAAAAAAAA4UHJvdG9jb2wgZmVlIGluIGJhc2lzIHBvaW50cyAobWF4IDUwIGJwcywgMCA9IGRpc2FibGVkKS4AAAAORmVlQmFzaXNQb2ludHMAAAAAAAAAAAAUUGF1c2UgYWRtaW4gYWRkcmVzcy4AAAAFQWRtaW4AAAAAAAAAAAAAH1doZXRoZXIgdGhlIGNvbnRyYWN0IGlzIHBhdXNlZC4AAAAABlBhdXNlZAAAAAAAAAAAAB9Hb3Zlcm5hbmNlIG11bHRpc2lnIHNpZ25lciBzZXQuAAAAAA9NdWx0aXNpZ1NpZ25lcnMAAAAAAAAAACVHb3Zlcm5hbmNlIG11bHRpc2lnIHF1b3J1bSB0aHJlc2hvbGQuAAAAAAAAEU11bHRpc2lnVGhyZXNob2xkAAAAAAAAAAAAAClQZW5kaW5nIHNpZ25lci1yb3RhdGlvbiBwcm9wb3NhbCwgaWYgYW55LgAAAAAAAA9QZW5kaW5nUm90YXRpb24A",
        "AAAAAAAAAJxCYXRjaCB2ZXJzaW9uIG9mIGBzZW5kYCDigJQgdHJhbnNmZXJzIHRva2VucyB0byBtdWx0aXBsZSBzdGVhbHRoIGFkZHJlc3NlcwphbmQgZW1pdHMgYW4gYW5ub3VuY2VtZW50IGZvciBlYWNoLgoKQWxsIGlucHV0IHZlY3RvcnMgbXVzdCBoYXZlIHRoZSBzYW1lIGxlbmd0aC4AAAAKYmF0Y2hfc2VuZAAAAAAABwAAAAAAAAAGc2VuZGVyAAAAAAATAAAAAAAAAAV0b2tlbgAAAAAAABMAAAAAAAAACXNjaGVtZV9pZAAAAAAAAAQAAAAAAAAAEXN0ZWFsdGhfYWRkcmVzc2VzAAAAAAAD6gAAABMAAAAAAAAAEmVwaGVtZXJhbF9wdWJfa2V5cwAAAAAD6gAAA+4AAAAgAAAAAAAAAAltZXRhZGF0YXMAAAAAAAPqAAAADgAAAAAAAAAHYW1vdW50cwAAAAPqAAAACwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAC1NlbmRlckVycm9yAA==",
        "AAAAAAAAAIxPbmUtdGltZSBzZXR1cCBvZiB0aGUgZ292ZXJuYW5jZSBzaWduZXIgc2V0IHVzZWQgdG8gYXV0aG9yaXNlIHNpZ25lcgpyb3RhdGlvbnMuIEluZGVwZW5kZW50IG9mIGBpbml0YCDigJQgZG9lcyBub3QgZ2F0ZSBgc2VuZGAvYGJhdGNoX3NlbmRgLgAAAA1pbml0X211bHRpc2lnAAAAAAAAAgAAAAAAAAAHc2lnbmVycwAAAAPqAAAAEwAAAAAAAAAJdGhyZXNob2xkAAAAAAAABAAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAC1NlbmRlckVycm9yAA==",
        "AAAAAAAAANBXaXRoZHJhdyBhc3NldHMgdG8gbXVsdGlwbGUgZGVzdGluYXRpb25zIGluIGEgc2luZ2xlIGF0b21pYyB0cmFuc2FjdGlvbi4KClRoZSBiYXRjaCBpcyBjYXBwZWQgYXQgMzAgZW50cmllcy4gSWYgYW55IHNpbmdsZSBlbnRyeSBjYW5ub3QgYmUKcHJvY2Vzc2VkLCB0aGUgZW50aXJlIGJhdGNoIGFib3J0cyBhbmQgbm8gc3RhdGUgY2hhbmdlcyBhcmUgcmV0YWluZWQuAAAADXdpdGhkcmF3X21hbnkAAAAAAAACAAAAAAAAAAp3aXRoZHJhd2VyAAAAAAATAAAAAAAAAAdlbnRyaWVzAAAAA+oAAAfQAAAAD1dpdGhkcmF3YWxFbnRyeQAAAAABAAAD6QAAA+0AAAAAAAAH0AAAAAtTZW5kZXJFcnJvcgA=",
        "AAAABAAAACxFcnJvcnMgdGhhdCB0aGUgc2VuZGVyIGNvbnRyYWN0IGNhbiBwcm9kdWNlLgAAAAAAAAALU2VuZGVyRXJyb3IAAAAAEAAAACpUaGUgY29udHJhY3QgaGFzIGFscmVhZHkgYmVlbiBpbml0aWFsaXNlZC4AAAAAABJBbHJlYWR5SW5pdGlhbGl6ZWQAAAAAAAEAAAAqVGhlIGNvbnRyYWN0IGhhcyBub3QgYmVlbiBpbml0aWFsaXNlZCB5ZXQuAAAAAAAOTm90SW5pdGlhbGl6ZWQAAAAAAAIAAAAwVGhlIGJhdGNoIGlucHV0IHZlY3RvcnMgaGF2ZSBtaXNtYXRjaGVkIGxlbmd0aHMuAAAADkxlbmd0aE1pc21hdGNoAAAAAAADAAAALVRoZSB0b2tlbiBpcyBub3QgYWxsb3dlZCBieSB0aGUgYXNzZXQgcG9saWN5LgAAAAAAAA9Ub2tlbk5vdEFsbG93ZWQAAAAABAAAAFNUaGUgZmVlIGNvbmZpZ3VyYXRpb24gaXMgaW52YWxpZCAoZS5nLiBmZWUgPiA1MCBicHMsIG9yIGZlZSA+IDAgd2l0aCBubyByZWNpcGllbnQpLgAAAAAQSW52YWxpZEZlZUNvbmZpZwAAAAUAAAAXVGhlIGNvbnRyYWN0IGlzIHBhdXNlZC4AAAAABlBhdXNlZAAAAAAAEAAAADRUaGUgYmF0Y2ggd2l0aGRyYXdhbCBleGNlZWRzIHRoZSBzdXBwb3J0ZWQgc2l6ZSBjYXAuAAAADUJhdGNoVG9vTGFyZ2UAAAAAAAAGAAAAMVRoZSBnb3Zlcm5hbmNlIG11bHRpc2lnIGhhcyBub3QgYmVlbiBpbml0aWFsaXNlZC4AAAAAAAAWTXVsdGlzaWdOb3RJbml0aWFsaXplZAAAAAAABwAAADVUaGUgZ292ZXJuYW5jZSBtdWx0aXNpZyBoYXMgYWxyZWFkeSBiZWVuIGluaXRpYWxpc2VkLgAAAAAAABpNdWx0aXNpZ0FscmVhZHlJbml0aWFsaXplZAAAAAAACAAAAC5UaGUgY2FsbGVyIGlzIG5vdCBhIGN1cnJlbnQgZ292ZXJuYW5jZSBzaWduZXIuAAAAAAAJTm90U2lnbmVyAAAAAAAACQAAAEhUaGUgcmVxdWVzdGVkIHRocmVzaG9sZCBpcyBpbnZhbGlkICh6ZXJvLCBvciBncmVhdGVyIHRoYW4gc2lnbmVyIGNvdW50KS4AAAAQSW52YWxpZFRocmVzaG9sZAAAAAoAAAAuQSBzaWduZXItcm90YXRpb24gcHJvcG9zYWwgaXMgYWxyZWFkeSBwZW5kaW5nLgAAAAAAFlJvdGF0aW9uQWxyZWFkeVBlbmRpbmcAAAAAAAsAAAAnTm8gc2lnbmVyLXJvdGF0aW9uIHByb3Bvc2FsIGlzIHBlbmRpbmcuAAAAABFOb1BlbmRpbmdSb3RhdGlvbgAAAAAAAAwAAAA1VGhlIGNhbGxlciBoYXMgYWxyZWFkeSBhcHByb3ZlZCB0aGUgcGVuZGluZyByb3RhdGlvbi4AAAAAAAAXQWxyZWFkeUFwcHJvdmVkUm90YXRpb24AAAAADQAAADxUaGUgcGVuZGluZyByb3RhdGlvbiBoYXMgbm90IGNvbGxlY3RlZCBlbm91Z2ggYXBwcm92YWxzIHlldC4AAAAMUXVvcnVtTm90TWV0AAAADgAAACpUaGUgcm90YXRpb24gdGltZWxvY2sgaGFzIG5vdCBlbGFwc2VkIHlldC4AAAAAABJUaW1lbG9ja05vdEVsYXBzZWQAAAAAAA8=",
        "AAAAAAAAAC1UaGUgcGVuZGluZyBzaWduZXItcm90YXRpb24gcHJvcG9zYWwsIGlmIGFueS4AAAAAAAAQcGVuZGluZ19yb3RhdGlvbgAAAAAAAAABAAAD6AAAB9AAAAAQUm90YXRpb25Qcm9wb3NhbA==",
        "AAAAAQAAADJBIHNpbmdsZSB3aXRoZHJhd2FsIGVudHJ5IGZvciBiYXRjaGVkIGFzc2V0IGV4aXRzLgAAAAAAAAAAAA9XaXRoZHJhd2FsRW50cnkAAAAAAwAAADBUaGUgYW1vdW50IHRvIHRyYW5zZmVyIGluIHRoZSB0b2tlbidzIGJhc2UgdW5pdC4AAAAGYW1vdW50AAAAAAALAAAAK1RoZSBkZXN0aW5hdGlvbiBhZGRyZXNzIGZvciB0aGUgd2l0aGRyYXdhbC4AAAAAAnRvAAAAAAATAAAAO1RoZSB0b2tlbiBjb250cmFjdCB0byB3aXRoZHJhdyBmcm9tIG9yIHRvIHRyYW5zZmVyIHRocm91Z2guAAAAAAV0b2tlbgAAAAAAABM=",
        "AAAAAAAAADdDYW5jZWwgdGhlIHBlbmRpbmcgcm90YXRpb24sIGNsZWFyaW5nIGFsbCBvZiBpdHMgc3RhdGUuAAAAABVjYW5jZWxfcm90YXRlX3NpZ25lcnMAAAAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAAAtTZW5kZXJFcnJvcgA=",
        "AAAAAAAAAC1BcHByb3ZlIHRoZSBwZW5kaW5nIHNpZ25lci1yb3RhdGlvbiBwcm9wb3NhbC4AAAAAAAAWYXBwcm92ZV9yb3RhdGVfc2lnbmVycwAAAAAAAQAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAALU2VuZGVyRXJyb3IA",
        "AAAAAAAAAGVFeGVjdXRlIHRoZSBwZW5kaW5nIHJvdGF0aW9uIG9uY2UgcXVvcnVtIGlzIG1ldCBhbmQgdGhlIHRpbWVsb2NrIGhhcwplbGFwc2VkLiBFbWl0cyBgU2lnbmVyc1JvdGF0ZWRgLgAAAAAAABZleGVjdXRlX3JvdGF0ZV9zaWduZXJzAAAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAAAtTZW5kZXJFcnJvcgA=",
        "AAAAAAAAAMJQcm9wb3NlIGEgbmV3IHNpZ25lciBzZXQgKyB0aHJlc2hvbGQgYmVoaW5kIHRoZSByb3RhdGlvbiB0aW1lbG9jay4KYGNhbGxlcmAgbXVzdCBiZSBhIGN1cnJlbnQgc2lnbmVyOyB0aGUgcHJvcG9zYWwgaXMgYXV0by1hcHByb3ZlZCBieQpgY2FsbGVyYC4gUmVqZWN0cyB0aHJlc2hvbGRzIHRoYXQgY291bGQgbmV2ZXIgcmVhY2ggcXVvcnVtLgAAAAAAFnByb3Bvc2Vfcm90YXRlX3NpZ25lcnMAAAAAAAMAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAALbmV3X3NpZ25lcnMAAAAD6gAAABMAAAAAAAAADW5ld190aHJlc2hvbGQAAAAAAAAEAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAALU2VuZGVyRXJyb3IA",
        "AAAAAQAAACNBIHBlbmRpbmcgc2lnbmVyLXJvdGF0aW9uIHByb3Bvc2FsLgAAAAAAAAAAEFJvdGF0aW9uUHJvcG9zYWwAAAAEAAAAAAAAAAlhcHByb3ZhbHMAAAAAAAPqAAAAEwAAAAAAAAANZXhlY3V0YWJsZV9hdAAAAAAAAAYAAAAAAAAAC25ld19zaWduZXJzAAAAA+oAAAATAAAAAAAAAA1uZXdfdGhyZXNob2xkAAAAAAAABA==",
        "AAAAAQAAAKpXcmFpdGggUHJvdG9jb2wgc3RhbmRhcmQgbWV0cmljIGV2ZW50IHNjaGVtYS4KCkFsbCBXcmFpdGggY29udHJhY3RzIGVtaXQgbWV0cmljIGV2ZW50cyB1c2luZyB0aGlzIHN0cnVjdHVyZSB0byBlbmFibGUKc3RhbmRhcmRpemVkIG9mZi1jaGFpbiBvYnNlcnZhYmlsaXR5IGFuZCBtb25pdG9yaW5nLgAAAAAAAAAAABFXcmFpdGhNZXRyaWNFdmVudAAAAAAAAAQAAABAQ29udHJhY3QgaWRlbnRpZmllciAoZS5nLiwgInN0ZWFsdGgtcmVnaXN0cnkiLCAic3RlYWx0aC1zZW5kZXIiKQAAAAhjb250cmFjdAAAABEAAABLT3B0aW9uYWwgZGltZW5zaW9ucyBmb3IgZmlsdGVyaW5nL2dyb3VwaW5nIChlLmcuLCB0b2tlbl9hZGRyZXNzLCBzY2hlbWVfaWQpAAAAAApkaW1lbnNpb25zAAAAAAPqAAAD7QAAAAIAAAARAAAAAAAAADNNZXRyaWMgbmFtZSAoZS5nLiwgInJlZ2lzdGVyX2NvdW50IiwgInNlbmRfdm9sdW1lIikAAAAAC21ldHJpY19uYW1lAAAAABEAAAAbTnVtZXJpYyB2YWx1ZSBvZiB0aGUgbWV0cmljAAAAAAV2YWx1ZQAAAAAAAAs=" ]),
      options
    )
  }
  public readonly fromJSON = {
    init: this.txFromJSON<Result<void>>,
        send: this.txFromJSON<Result<void>>,
        pause: this.txFromJSON<Result<void>>,
        signers: this.txFromJSON<Array<string>>,
        unpause: this.txFromJSON<Result<void>>,
        is_paused: this.txFromJSON<boolean>,
        threshold: this.txFromJSON<u32>,
        batch_send: this.txFromJSON<Result<void>>,
        init_multisig: this.txFromJSON<Result<void>>,
        withdraw_many: this.txFromJSON<Result<void>>,
        pending_rotation: this.txFromJSON<Option<RotationProposal>>,
        cancel_rotate_signers: this.txFromJSON<Result<void>>,
        approve_rotate_signers: this.txFromJSON<Result<void>>,
        execute_rotate_signers: this.txFromJSON<Result<void>>,
        propose_rotate_signers: this.txFromJSON<Result<void>>
  }
}