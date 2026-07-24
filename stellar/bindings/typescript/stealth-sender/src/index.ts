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
export type DataKey = {tag: "Announcer", values: void} | {tag: "AssetPolicy", values: void} | {tag: "FeeRecipient", values: void} | {tag: "FeeBasisPoints", values: void};

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
   * The sponsored announcement batch exceeds the maximum entry count.
   */
  6: {message:"SponsoredBatchTooLarge"}
}


/**
 * A token transfer and announcement authenticated by its sender.
 */
export interface SponsoredEntry {
  amount: i128;
  ephemeral_pub_key: Buffer;
  metadata: Buffer;
  scheme_id: u32;
  sender: string;
  stealth_address: string;
  token: string;
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
  init: ({announcer, asset_policy, fee_recipient, fee_basis_points}: {announcer: string, asset_policy: Option<string>, fee_recipient: Option<string>, fee_basis_points: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

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
   * Construct and simulate a batch_send transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Batch version of `send` — transfers tokens to multiple stealth addresses
   * and emits an announcement for each.
   * 
   * All input vectors must have the same length.
   */
  batch_send: ({sender, token, scheme_id, stealth_addresses, ephemeral_pub_keys, metadatas, amounts}: {sender: string, token: string, scheme_id: u32, stealth_addresses: Array<string>, ephemeral_pub_keys: Array<Buffer>, metadatas: Array<Buffer>, amounts: Array<i128>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a sponsored_announce transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Transfer tokens and emit announcements for entries paid for by a sponsor.
   * 
   * The sponsor and every entry sender must authorise the invocation. The
   * sponsor pays the transaction fee through Stellar fee-bump mechanics;
   * entry senders remain responsible for their own token transfers.
   */
  sponsored_announce: ({sponsor, entries}: {sponsor: string, entries: Array<SponsoredEntry>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

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
      new ContractSpec([ "AAAAAgAAAA1TdG9yYWdlIGtleXMuAAAAAAAAAAAAAAdEYXRhS2V5AAAAAAQAAAAAAAAANlRoZSBhZGRyZXNzIG9mIHRoZSBkZXBsb3llZCBTdGVhbHRoQW5ub3VuY2VyIGNvbnRyYWN0LgAAAAAACUFubm91bmNlcgAAAAAAAAAAAAAuT3B0aW9uYWwgYWRkcmVzcyBvZiB0aGUgYXNzZXQgcG9saWN5IGNvbnRyYWN0LgAAAAAAC0Fzc2V0UG9saWN5AAAAAAAAAAAvT3B0aW9uYWwgYWRkcmVzcyBvZiB0aGUgcHJvdG9jb2wgZmVlIHJlY2lwaWVudC4AAAAADEZlZVJlY2lwaWVudAAAAAAAAAA4UHJvdG9jb2wgZmVlIGluIGJhc2lzIHBvaW50cyAobWF4IDUwIGJwcywgMCA9IGRpc2FibGVkKS4AAAAORmVlQmFzaXNQb2ludHMAAA==",
        "AAAABAAAACxFcnJvcnMgdGhhdCB0aGUgc2VuZGVyIGNvbnRyYWN0IGNhbiBwcm9kdWNlLgAAAAAAAAALU2VuZGVyRXJyb3IAAAAABgAAACpUaGUgY29udHJhY3QgaGFzIGFscmVhZHkgYmVlbiBpbml0aWFsaXNlZC4AAAAAABJBbHJlYWR5SW5pdGlhbGl6ZWQAAAAAAAEAAAAqVGhlIGNvbnRyYWN0IGhhcyBub3QgYmVlbiBpbml0aWFsaXNlZCB5ZXQuAAAAAAAOTm90SW5pdGlhbGl6ZWQAAAAAAAIAAAAwVGhlIGJhdGNoIGlucHV0IHZlY3RvcnMgaGF2ZSBtaXNtYXRjaGVkIGxlbmd0aHMuAAAADkxlbmd0aE1pc21hdGNoAAAAAAADAAAALVRoZSB0b2tlbiBpcyBub3QgYWxsb3dlZCBieSB0aGUgYXNzZXQgcG9saWN5LgAAAAAAAA9Ub2tlbk5vdEFsbG93ZWQAAAAABAAAAFNUaGUgZmVlIGNvbmZpZ3VyYXRpb24gaXMgaW52YWxpZCAoZS5nLiBmZWUgPiA1MCBicHMsIG9yIGZlZSA+IDAgd2l0aCBubyByZWNpcGllbnQpLgAAAAAQSW52YWxpZEZlZUNvbmZpZwAAAAUAAABBVGhlIHNwb25zb3JlZCBhbm5vdW5jZW1lbnQgYmF0Y2ggZXhjZWVkcyB0aGUgbWF4aW11bSBlbnRyeSBjb3VudC4AAAAAAAAWU3BvbnNvcmVkQmF0Y2hUb29MYXJnZQAAAAAABg==",
        "AAAAAQAAAD5BIHRva2VuIHRyYW5zZmVyIGFuZCBhbm5vdW5jZW1lbnQgYXV0aGVudGljYXRlZCBieSBpdHMgc2VuZGVyLgAAAAAAAAAAAA5TcG9uc29yZWRFbnRyeQAAAAAABwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAABFlcGhlbWVyYWxfcHViX2tleQAAAAAAA+4AAAAgAAAAAAAAAAhtZXRhZGF0YQAAAA4AAAAAAAAACXNjaGVtZV9pZAAAAAAAAAQAAAAAAAAABnNlbmRlcgAAAAAAEwAAAAAAAAAPc3RlYWx0aF9hZGRyZXNzAAAAABMAAAAAAAAABXRva2VuAAAAAAAAEw==",
        "AAAAAAAAALlJbml0aWFsaXNlIHRoZSBjb250cmFjdCBieSBzdG9yaW5nIHRoZSBhbm5vdW5jZXIgYWRkcmVzcywgb3B0aW9uYWwgYXNzZXQgcG9saWN5LAphbmQgb3B0aW9uYWwgcHJvdG9jb2wgZmVlIGNvbmZpZ3VyYXRpb24uCgpNdXN0IGJlIGNhbGxlZCBleGFjdGx5IG9uY2UgYmVmb3JlIGFueSBgc2VuZGAgb3IgYGJhdGNoX3NlbmRgLgAAAAAAAARpbml0AAAABAAAAAAAAAAJYW5ub3VuY2VyAAAAAAAAEwAAAAAAAAAMYXNzZXRfcG9saWN5AAAD6AAAABMAAAAAAAAADWZlZV9yZWNpcGllbnQAAAAAAAPoAAAAEwAAAAAAAAAQZmVlX2Jhc2lzX3BvaW50cwAAAAQAAAABAAAD6QAAA+0AAAAAAAAH0AAAAAtTZW5kZXJFcnJvcgA=",
        "AAAAAAAAAglUcmFuc2ZlciB0b2tlbnMgdG8gYSBzdGVhbHRoIGFkZHJlc3MgYW5kIGVtaXQgYW4gYW5ub3VuY2VtZW50LgoKIyBBcmd1bWVudHMKKiBgc2VuZGVyYCAgICAgICAgICAgIC0gVGhlIGFkZHJlc3Mgc2VuZGluZyBmdW5kcyAobXVzdCBhdXRob3Jpc2UpLgoqIGB0b2tlbmAgICAgICAgICAgICAgLSBTQUMgdG9rZW4gY29udHJhY3QgYWRkcmVzcyAod29ya3MgZm9yIG5hdGl2ZSBYTE0gdG9vKS4KKiBgYW1vdW50YCAgICAgICAgICAgIC0gQW1vdW50IG9mIHRva2VucyB0byB0cmFuc2Zlci4KKiBgc2NoZW1lX2lkYCAgICAgICAgIC0gU3RlYWx0aCBhZGRyZXNzIHNjaGVtZSBpZGVudGlmaWVyLgoqIGBzdGVhbHRoX2FkZHJlc3NgICAgLSBUaGUgZGVyaXZlZCBvbmUtdGltZSBzdGVhbHRoIGFkZHJlc3MuCiogYGVwaGVtZXJhbF9wdWJfa2V5YCAtIEVwaGVtZXJhbCBwdWJsaWMga2V5IGZvciB0aGUgcmVjaXBpZW50IHRvIHNjYW4uCiogYG1ldGFkYXRhYCAgICAgICAgICAtIEV4dHJhIGRhdGEgKGUuZy4gdmlldyB0YWcpLgAAAAAAAARzZW5kAAAABwAAAAAAAAAGc2VuZGVyAAAAAAATAAAAAAAAAAV0b2tlbgAAAAAAABMAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAAJc2NoZW1lX2lkAAAAAAAABAAAAAAAAAAPc3RlYWx0aF9hZGRyZXNzAAAAABMAAAAAAAAAEWVwaGVtZXJhbF9wdWJfa2V5AAAAAAAD7gAAACAAAAAAAAAACG1ldGFkYXRhAAAADgAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAC1NlbmRlckVycm9yAA==",
        "AAAAAAAAAJxCYXRjaCB2ZXJzaW9uIG9mIGBzZW5kYCDigJQgdHJhbnNmZXJzIHRva2VucyB0byBtdWx0aXBsZSBzdGVhbHRoIGFkZHJlc3NlcwphbmQgZW1pdHMgYW4gYW5ub3VuY2VtZW50IGZvciBlYWNoLgoKQWxsIGlucHV0IHZlY3RvcnMgbXVzdCBoYXZlIHRoZSBzYW1lIGxlbmd0aC4AAAAKYmF0Y2hfc2VuZAAAAAAABwAAAAAAAAAGc2VuZGVyAAAAAAATAAAAAAAAAAV0b2tlbgAAAAAAABMAAAAAAAAACXNjaGVtZV9pZAAAAAAAAAQAAAAAAAAAEXN0ZWFsdGhfYWRkcmVzc2VzAAAAAAAD6gAAABMAAAAAAAAAEmVwaGVtZXJhbF9wdWJfa2V5cwAAAAAD6gAAA+4AAAAgAAAAAAAAAAltZXRhZGF0YXMAAAAAAAPqAAAADgAAAAAAAAAHYW1vdW50cwAAAAPqAAAACwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAC1NlbmRlckVycm9yAA==",
        "AAAAAAAAARVUcmFuc2ZlciB0b2tlbnMgYW5kIGVtaXQgYW5ub3VuY2VtZW50cyBmb3IgZW50cmllcyBwYWlkIGZvciBieSBhIHNwb25zb3IuCgpUaGUgc3BvbnNvciBhbmQgZXZlcnkgZW50cnkgc2VuZGVyIG11c3QgYXV0aG9yaXNlIHRoZSBpbnZvY2F0aW9uLiBUaGUKc3BvbnNvciBwYXlzIHRoZSB0cmFuc2FjdGlvbiBmZWUgdGhyb3VnaCBTdGVsbGFyIGZlZS1idW1wIG1lY2hhbmljczsKZW50cnkgc2VuZGVycyByZW1haW4gcmVzcG9uc2libGUgZm9yIHRoZWlyIG93biB0b2tlbiB0cmFuc2ZlcnMuAAAAAAAAEnNwb25zb3JlZF9hbm5vdW5jZQAAAAAAAgAAAAAAAAAHc3BvbnNvcgAAAAATAAAAAAAAAAdlbnRyaWVzAAAAA+oAAAfQAAAADlNwb25zb3JlZEVudHJ5AAAAAAABAAAD6QAAA+0AAAAAAAAH0AAAAAtTZW5kZXJFcnJvcgA=",
        "AAAAAQAAAKpXcmFpdGggUHJvdG9jb2wgc3RhbmRhcmQgbWV0cmljIGV2ZW50IHNjaGVtYS4KCkFsbCBXcmFpdGggY29udHJhY3RzIGVtaXQgbWV0cmljIGV2ZW50cyB1c2luZyB0aGlzIHN0cnVjdHVyZSB0byBlbmFibGUKc3RhbmRhcmRpemVkIG9mZi1jaGFpbiBvYnNlcnZhYmlsaXR5IGFuZCBtb25pdG9yaW5nLgAAAAAAAAAAABFXcmFpdGhNZXRyaWNFdmVudAAAAAAAAAQAAABAQ29udHJhY3QgaWRlbnRpZmllciAoZS5nLiwgInN0ZWFsdGgtcmVnaXN0cnkiLCAic3RlYWx0aC1zZW5kZXIiKQAAAAhjb250cmFjdAAAABEAAABLT3B0aW9uYWwgZGltZW5zaW9ucyBmb3IgZmlsdGVyaW5nL2dyb3VwaW5nIChlLmcuLCB0b2tlbl9hZGRyZXNzLCBzY2hlbWVfaWQpAAAAAApkaW1lbnNpb25zAAAAAAPqAAAD7QAAAAIAAAARAAAAAAAAADNNZXRyaWMgbmFtZSAoZS5nLiwgInJlZ2lzdGVyX2NvdW50IiwgInNlbmRfdm9sdW1lIikAAAAAC21ldHJpY19uYW1lAAAAABEAAAAbTnVtZXJpYyB2YWx1ZSBvZiB0aGUgbWV0cmljAAAAAAV2YWx1ZQAAAAAAAAs=" ]),
      options
    )
  }
  public readonly fromJSON = {
    init: this.txFromJSON<Result<void>>,
        send: this.txFromJSON<Result<void>>,
        batch_send: this.txFromJSON<Result<void>>,
        sponsored_announce: this.txFromJSON<Result<void>>
  }
}