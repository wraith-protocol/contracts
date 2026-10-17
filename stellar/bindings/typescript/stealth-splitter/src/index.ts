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
export type DataKey = {tag: "Announcer", values: void} | {tag: "Split", values: readonly [Buffer]} | {tag: "SplitFunded", values: readonly [Buffer]};


/**
 * A single beneficiary in a split.
 */
export interface Beneficiary {
  /**
 * Stealth meta-address of the beneficiary (64 bytes: spending_pubkey || viewing_pubkey).
 */
meta_address: Buffer;
  /**
 * Weight for proportional distribution.
 */
weight: u128;
}


/**
 * Details returned by get_split query.
 */
export interface SplitDetails {
  /**
 * Immutable list of beneficiaries.
 */
beneficiaries: Array<Beneficiary>;
  /**
 * Total amount funded to date.
 */
total_funded: i128;
}

/**
 * Errors that the splitter contract can produce.
 */
export const SplitterError = {
  /**
   * The contract has already been initialised.
   */
  1: {message:"AlreadyInitialized"},
  /**
   * The contract has not been initialised yet.
   */
  2: {message:"NotInitialized"},
  /**
   * The split does not exist.
   */
  3: {message:"SplitNotFound"},
  /**
   * Too many beneficiaries (max 25).
   */
  4: {message:"TooManyBeneficiaries"},
  /**
   * Weight sum would overflow (internal sanity check).
   */
  5: {message:"WeightOverflow"},
  /**
   * Invalid beneficiary meta-address length (must be 64 bytes).
   */
  6: {message:"InvalidMetaAddressLength"},
  /**
   * Amount to fund is zero or negative.
   */
  7: {message:"InvalidAmount"},
  /**
   * Empty beneficiary list.
   */
  8: {message:"EmptyBeneficiaries"}
}


/**
 * Complete definition of a split.
 */
export interface SplitDefinition {
  /**
 * Token contract address.
 */
asset: string;
  /**
 * List of beneficiaries (max 25 enforced at creation).
 */
beneficiaries: Array<Beneficiary>;
  /**
 * Creator address.
 */
creator: string;
  /**
 * Salt used to create this split (for uniqueness).
 */
salt: Buffer;
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
   * Initialise the contract by storing the announcer address.
   * 
   * Must be called exactly once before any `create_split` or `fund_split`.
   */
  init: ({announcer}: {announcer: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_split transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Query split details: beneficiaries and total funded amount.
   * 
   * # Arguments
   * * `split_id` - The split to query.
   * 
   * # Returns
   * SplitDetails with immutable beneficiary list and total_funded.
   */
  get_split: ({split_id}: {split_id: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<SplitDetails>>>

  /**
   * Construct and simulate a fund_split transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Fund a split: deposit amount and atomically distribute to all beneficiaries.
   * 
   * Each beneficiary receives a proportional share based on their weight.
   * Rounding error (dust) is absorbed by the first beneficiary.
   * All transfers and announcements are atomic: any failure rolls back everything.
   * 
   * # Arguments
   * * `funder` - The address funding the split (must authorise).
   * * `split_id` - The split to fund.
   * * `amount` - Total amount to distribute.
   * * `scheme_id` - Stealth address scheme identifier.
   * * `stealth_addresses` - One-time stealth address for each beneficiary.
   * * `ephemeral_pub_keys` - Ephemeral key for each beneficiary.
   * * `metadatas` - Metadata (e.g. view tag) for each beneficiary.
   */
  fund_split: ({funder, split_id, amount, scheme_id, stealth_addresses, ephemeral_pub_keys, metadatas}: {funder: string, split_id: Buffer, amount: i128, scheme_id: u32, stealth_addresses: Array<string>, ephemeral_pub_keys: Array<Buffer>, metadatas: Array<Buffer>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a create_split transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Create a split definition (immutable after creation).
   * 
   * Computes a deterministic split_id from the beneficiaries, asset, and salt.
   * The split is not funded until `fund_split` is called.
   * 
   * # Arguments
   * * `creator` - The address creating the split (must authorise).
   * * `beneficiaries` - List of (meta_address, weight) pairs. Max 25.
   * * `asset` - The token contract address to be split.
   * * `salt` - Random bytes to ensure uniqueness.
   * 
   * # Returns
   * The deterministic split_id (32-byte hash).
   */
  create_split: ({creator, beneficiaries, asset, salt}: {creator: string, beneficiaries: Array<Beneficiary>, asset: string, salt: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Buffer>>>

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
      new ContractSpec([ "AAAAAAAAAIFJbml0aWFsaXNlIHRoZSBjb250cmFjdCBieSBzdG9yaW5nIHRoZSBhbm5vdW5jZXIgYWRkcmVzcy4KCk11c3QgYmUgY2FsbGVkIGV4YWN0bHkgb25jZSBiZWZvcmUgYW55IGBjcmVhdGVfc3BsaXRgIG9yIGBmdW5kX3NwbGl0YC4AAAAAAAAEaW5pdAAAAAEAAAAAAAAACWFubm91bmNlcgAAAAAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA1TcGxpdHRlckVycm9yAAAA",
        "AAAAAAAAALVRdWVyeSBzcGxpdCBkZXRhaWxzOiBiZW5lZmljaWFyaWVzIGFuZCB0b3RhbCBmdW5kZWQgYW1vdW50LgoKIyBBcmd1bWVudHMKKiBgc3BsaXRfaWRgIC0gVGhlIHNwbGl0IHRvIHF1ZXJ5LgoKIyBSZXR1cm5zClNwbGl0RGV0YWlscyB3aXRoIGltbXV0YWJsZSBiZW5lZmljaWFyeSBsaXN0IGFuZCB0b3RhbF9mdW5kZWQuAAAAAAAACWdldF9zcGxpdAAAAAAAAAEAAAAAAAAACHNwbGl0X2lkAAAD7gAAACAAAAABAAAD6QAAB9AAAAAMU3BsaXREZXRhaWxzAAAH0AAAAA1TcGxpdHRlckVycm9yAAAA",
        "AAAAAgAAAA1TdG9yYWdlIGtleXMuAAAAAAAAAAAAAAdEYXRhS2V5AAAAAAMAAAAAAAAAMkFkZHJlc3Mgb2YgdGhlIGRlcGxveWVkIFN0ZWFsdGhBbm5vdW5jZXIgY29udHJhY3QuAAAAAAAJQW5ub3VuY2VyAAAAAAAAAQAAAD5TdG9yZXMgc3BsaXQgZGVmaW5pdGlvbjogU3BsaXREZWZpbml0aW9uIGZvciBhIGdpdmVuIHNwbGl0X2lkLgAAAAAABVNwbGl0AAAAAAAAAQAAA+4AAAAgAAAAAQAAADBUcmFja3MgdG90YWwgZnVuZGVkIGFtb3VudCBmb3IgYSBnaXZlbiBzcGxpdF9pZC4AAAALU3BsaXRGdW5kZWQAAAAAAQAAA+4AAAAg",
        "AAAAAAAAAqlGdW5kIGEgc3BsaXQ6IGRlcG9zaXQgYW1vdW50IGFuZCBhdG9taWNhbGx5IGRpc3RyaWJ1dGUgdG8gYWxsIGJlbmVmaWNpYXJpZXMuCgpFYWNoIGJlbmVmaWNpYXJ5IHJlY2VpdmVzIGEgcHJvcG9ydGlvbmFsIHNoYXJlIGJhc2VkIG9uIHRoZWlyIHdlaWdodC4KUm91bmRpbmcgZXJyb3IgKGR1c3QpIGlzIGFic29yYmVkIGJ5IHRoZSBmaXJzdCBiZW5lZmljaWFyeS4KQWxsIHRyYW5zZmVycyBhbmQgYW5ub3VuY2VtZW50cyBhcmUgYXRvbWljOiBhbnkgZmFpbHVyZSByb2xscyBiYWNrIGV2ZXJ5dGhpbmcuCgojIEFyZ3VtZW50cwoqIGBmdW5kZXJgIC0gVGhlIGFkZHJlc3MgZnVuZGluZyB0aGUgc3BsaXQgKG11c3QgYXV0aG9yaXNlKS4KKiBgc3BsaXRfaWRgIC0gVGhlIHNwbGl0IHRvIGZ1bmQuCiogYGFtb3VudGAgLSBUb3RhbCBhbW91bnQgdG8gZGlzdHJpYnV0ZS4KKiBgc2NoZW1lX2lkYCAtIFN0ZWFsdGggYWRkcmVzcyBzY2hlbWUgaWRlbnRpZmllci4KKiBgc3RlYWx0aF9hZGRyZXNzZXNgIC0gT25lLXRpbWUgc3RlYWx0aCBhZGRyZXNzIGZvciBlYWNoIGJlbmVmaWNpYXJ5LgoqIGBlcGhlbWVyYWxfcHViX2tleXNgIC0gRXBoZW1lcmFsIGtleSBmb3IgZWFjaCBiZW5lZmljaWFyeS4KKiBgbWV0YWRhdGFzYCAtIE1ldGFkYXRhIChlLmcuIHZpZXcgdGFnKSBmb3IgZWFjaCBiZW5lZmljaWFyeS4AAAAAAAAKZnVuZF9zcGxpdAAAAAAABwAAAAAAAAAGZnVuZGVyAAAAAAATAAAAAAAAAAhzcGxpdF9pZAAAA+4AAAAgAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAACXNjaGVtZV9pZAAAAAAAAAQAAAAAAAAAEXN0ZWFsdGhfYWRkcmVzc2VzAAAAAAAD6gAAABMAAAAAAAAAEmVwaGVtZXJhbF9wdWJfa2V5cwAAAAAD6gAAA+4AAAAgAAAAAAAAAAltZXRhZGF0YXMAAAAAAAPqAAAADgAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADVNwbGl0dGVyRXJyb3IAAAA=",
        "AAAAAAAAAd1DcmVhdGUgYSBzcGxpdCBkZWZpbml0aW9uIChpbW11dGFibGUgYWZ0ZXIgY3JlYXRpb24pLgoKQ29tcHV0ZXMgYSBkZXRlcm1pbmlzdGljIHNwbGl0X2lkIGZyb20gdGhlIGJlbmVmaWNpYXJpZXMsIGFzc2V0LCBhbmQgc2FsdC4KVGhlIHNwbGl0IGlzIG5vdCBmdW5kZWQgdW50aWwgYGZ1bmRfc3BsaXRgIGlzIGNhbGxlZC4KCiMgQXJndW1lbnRzCiogYGNyZWF0b3JgIC0gVGhlIGFkZHJlc3MgY3JlYXRpbmcgdGhlIHNwbGl0IChtdXN0IGF1dGhvcmlzZSkuCiogYGJlbmVmaWNpYXJpZXNgIC0gTGlzdCBvZiAobWV0YV9hZGRyZXNzLCB3ZWlnaHQpIHBhaXJzLiBNYXggMjUuCiogYGFzc2V0YCAtIFRoZSB0b2tlbiBjb250cmFjdCBhZGRyZXNzIHRvIGJlIHNwbGl0LgoqIGBzYWx0YCAtIFJhbmRvbSBieXRlcyB0byBlbnN1cmUgdW5pcXVlbmVzcy4KCiMgUmV0dXJucwpUaGUgZGV0ZXJtaW5pc3RpYyBzcGxpdF9pZCAoMzItYnl0ZSBoYXNoKS4AAAAAAAAMY3JlYXRlX3NwbGl0AAAABAAAAAAAAAAHY3JlYXRvcgAAAAATAAAAAAAAAA1iZW5lZmljaWFyaWVzAAAAAAAD6gAAB9AAAAALQmVuZWZpY2lhcnkAAAAAAAAAAAVhc3NldAAAAAAAABMAAAAAAAAABHNhbHQAAAAOAAAAAQAAA+kAAAPuAAAAIAAAB9AAAAANU3BsaXR0ZXJFcnJvcgAAAA==",
        "AAAAAQAAACBBIHNpbmdsZSBiZW5lZmljaWFyeSBpbiBhIHNwbGl0LgAAAAAAAAALQmVuZWZpY2lhcnkAAAAAAgAAAFZTdGVhbHRoIG1ldGEtYWRkcmVzcyBvZiB0aGUgYmVuZWZpY2lhcnkgKDY0IGJ5dGVzOiBzcGVuZGluZ19wdWJrZXkgfHwgdmlld2luZ19wdWJrZXkpLgAAAAAADG1ldGFfYWRkcmVzcwAAAA4AAAAlV2VpZ2h0IGZvciBwcm9wb3J0aW9uYWwgZGlzdHJpYnV0aW9uLgAAAAAAAAZ3ZWlnaHQAAAAAAAo=",
        "AAAAAQAAACREZXRhaWxzIHJldHVybmVkIGJ5IGdldF9zcGxpdCBxdWVyeS4AAAAAAAAADFNwbGl0RGV0YWlscwAAAAIAAAAgSW1tdXRhYmxlIGxpc3Qgb2YgYmVuZWZpY2lhcmllcy4AAAANYmVuZWZpY2lhcmllcwAAAAAAA+oAAAfQAAAAC0JlbmVmaWNpYXJ5AAAAABxUb3RhbCBhbW91bnQgZnVuZGVkIHRvIGRhdGUuAAAADHRvdGFsX2Z1bmRlZAAAAAs=",
        "AAAABAAAAC5FcnJvcnMgdGhhdCB0aGUgc3BsaXR0ZXIgY29udHJhY3QgY2FuIHByb2R1Y2UuAAAAAAAAAAAADVNwbGl0dGVyRXJyb3IAAAAAAAAIAAAAKlRoZSBjb250cmFjdCBoYXMgYWxyZWFkeSBiZWVuIGluaXRpYWxpc2VkLgAAAAAAEkFscmVhZHlJbml0aWFsaXplZAAAAAAAAQAAACpUaGUgY29udHJhY3QgaGFzIG5vdCBiZWVuIGluaXRpYWxpc2VkIHlldC4AAAAAAA5Ob3RJbml0aWFsaXplZAAAAAAAAgAAABlUaGUgc3BsaXQgZG9lcyBub3QgZXhpc3QuAAAAAAAADVNwbGl0Tm90Rm91bmQAAAAAAAADAAAAIFRvbyBtYW55IGJlbmVmaWNpYXJpZXMgKG1heCAyNSkuAAAAFFRvb01hbnlCZW5lZmljaWFyaWVzAAAABAAAADJXZWlnaHQgc3VtIHdvdWxkIG92ZXJmbG93IChpbnRlcm5hbCBzYW5pdHkgY2hlY2spLgAAAAAADldlaWdodE92ZXJmbG93AAAAAAAFAAAAO0ludmFsaWQgYmVuZWZpY2lhcnkgbWV0YS1hZGRyZXNzIGxlbmd0aCAobXVzdCBiZSA2NCBieXRlcykuAAAAABhJbnZhbGlkTWV0YUFkZHJlc3NMZW5ndGgAAAAGAAAAI0Ftb3VudCB0byBmdW5kIGlzIHplcm8gb3IgbmVnYXRpdmUuAAAAAA1JbnZhbGlkQW1vdW50AAAAAAAABwAAABdFbXB0eSBiZW5lZmljaWFyeSBsaXN0LgAAAAASRW1wdHlCZW5lZmljaWFyaWVzAAAAAAAI",
        "AAAAAQAAAB9Db21wbGV0ZSBkZWZpbml0aW9uIG9mIGEgc3BsaXQuAAAAAAAAAAAPU3BsaXREZWZpbml0aW9uAAAAAAQAAAAXVG9rZW4gY29udHJhY3QgYWRkcmVzcy4AAAAABWFzc2V0AAAAAAAAEwAAADRMaXN0IG9mIGJlbmVmaWNpYXJpZXMgKG1heCAyNSBlbmZvcmNlZCBhdCBjcmVhdGlvbikuAAAADWJlbmVmaWNpYXJpZXMAAAAAAAPqAAAH0AAAAAtCZW5lZmljaWFyeQAAAAAQQ3JlYXRvciBhZGRyZXNzLgAAAAdjcmVhdG9yAAAAABMAAAAwU2FsdCB1c2VkIHRvIGNyZWF0ZSB0aGlzIHNwbGl0IChmb3IgdW5pcXVlbmVzcykuAAAABHNhbHQAAAAO",
        "AAAAAQAAAKpXcmFpdGggUHJvdG9jb2wgc3RhbmRhcmQgbWV0cmljIGV2ZW50IHNjaGVtYS4KCkFsbCBXcmFpdGggY29udHJhY3RzIGVtaXQgbWV0cmljIGV2ZW50cyB1c2luZyB0aGlzIHN0cnVjdHVyZSB0byBlbmFibGUKc3RhbmRhcmRpemVkIG9mZi1jaGFpbiBvYnNlcnZhYmlsaXR5IGFuZCBtb25pdG9yaW5nLgAAAAAAAAAAABFXcmFpdGhNZXRyaWNFdmVudAAAAAAAAAQAAABAQ29udHJhY3QgaWRlbnRpZmllciAoZS5nLiwgInN0ZWFsdGgtcmVnaXN0cnkiLCAic3RlYWx0aC1zZW5kZXIiKQAAAAhjb250cmFjdAAAABEAAABLT3B0aW9uYWwgZGltZW5zaW9ucyBmb3IgZmlsdGVyaW5nL2dyb3VwaW5nIChlLmcuLCB0b2tlbl9hZGRyZXNzLCBzY2hlbWVfaWQpAAAAAApkaW1lbnNpb25zAAAAAAPqAAAD7QAAAAIAAAARAAAAAAAAADNNZXRyaWMgbmFtZSAoZS5nLiwgInJlZ2lzdGVyX2NvdW50IiwgInNlbmRfdm9sdW1lIikAAAAAC21ldHJpY19uYW1lAAAAABEAAAAbTnVtZXJpYyB2YWx1ZSBvZiB0aGUgbWV0cmljAAAAAAV2YWx1ZQAAAAAAAAs=" ]),
      options
    )
  }
  public readonly fromJSON = {
    init: this.txFromJSON<Result<void>>,
        get_split: this.txFromJSON<Result<SplitDetails>>,
        fund_split: this.txFromJSON<Result<void>>,
        create_split: this.txFromJSON<Result<Buffer>>
  }
}