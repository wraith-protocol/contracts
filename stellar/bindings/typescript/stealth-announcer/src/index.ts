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





export interface Client {
  /**
   * Construct and simulate a announce transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Emits a Stellar v2 stealth address announcement event.
   * 
   * This is a pure event-emission function with no access control and no
   * storage. Indexers watch for these events to let recipients detect
   * incoming payments.
   * 
   * v2 event shape:
   * * topics: `("announce", scheme_id, view_tag_bucket, metadata_kind)`
   * * data: `(stealth_address, ephemeral_pub_key, metadata)`
   * 
   * The stable `view_tag_bucket` derivation is `metadata[0] as u32`, where
   * `metadata_kind = 1` (`METADATA_KIND_VIEW_TAG`) means the first metadata
   * byte is the view tag and the remaining bytes are scheme-specific. This
   * lets wallets and indexers filter Stellar RPC `getEvents` by scheme and
   * bucket before doing client-side cryptographic validation.
   * 
   * Migration note: v1 announcements used the old Stellar layout
   * `("announce", scheme_id, stealth_address)` with
   * `(caller, ephemeral_pub_key, metadata)`. Do not reinterpret historical v1
   * events as v2. The compatibility path is a new announcer deployment using
   * `scheme_id = 2`.
   * 
   * # Arguments
   * * `scheme_id` - Must be `2` for the v2 St
   */
  announce: ({scheme_id, stealth_address, ephemeral_pub_key, metadata}: {scheme_id: u32, stealth_address: string, ephemeral_pub_key: Buffer, metadata: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

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
      new ContractSpec([ "AAAAAAAABABFbWl0cyBhIFN0ZWxsYXIgdjIgc3RlYWx0aCBhZGRyZXNzIGFubm91bmNlbWVudCBldmVudC4KClRoaXMgaXMgYSBwdXJlIGV2ZW50LWVtaXNzaW9uIGZ1bmN0aW9uIHdpdGggbm8gYWNjZXNzIGNvbnRyb2wgYW5kIG5vCnN0b3JhZ2UuIEluZGV4ZXJzIHdhdGNoIGZvciB0aGVzZSBldmVudHMgdG8gbGV0IHJlY2lwaWVudHMgZGV0ZWN0CmluY29taW5nIHBheW1lbnRzLgoKdjIgZXZlbnQgc2hhcGU6CiogdG9waWNzOiBgKCJhbm5vdW5jZSIsIHNjaGVtZV9pZCwgdmlld190YWdfYnVja2V0LCBtZXRhZGF0YV9raW5kKWAKKiBkYXRhOiBgKHN0ZWFsdGhfYWRkcmVzcywgZXBoZW1lcmFsX3B1Yl9rZXksIG1ldGFkYXRhKWAKClRoZSBzdGFibGUgYHZpZXdfdGFnX2J1Y2tldGAgZGVyaXZhdGlvbiBpcyBgbWV0YWRhdGFbMF0gYXMgdTMyYCwgd2hlcmUKYG1ldGFkYXRhX2tpbmQgPSAxYCAoYE1FVEFEQVRBX0tJTkRfVklFV19UQUdgKSBtZWFucyB0aGUgZmlyc3QgbWV0YWRhdGEKYnl0ZSBpcyB0aGUgdmlldyB0YWcgYW5kIHRoZSByZW1haW5pbmcgYnl0ZXMgYXJlIHNjaGVtZS1zcGVjaWZpYy4gVGhpcwpsZXRzIHdhbGxldHMgYW5kIGluZGV4ZXJzIGZpbHRlciBTdGVsbGFyIFJQQyBgZ2V0RXZlbnRzYCBieSBzY2hlbWUgYW5kCmJ1Y2tldCBiZWZvcmUgZG9pbmcgY2xpZW50LXNpZGUgY3J5cHRvZ3JhcGhpYyB2YWxpZGF0aW9uLgoKTWlncmF0aW9uIG5vdGU6IHYxIGFubm91bmNlbWVudHMgdXNlZCB0aGUgb2xkIFN0ZWxsYXIgbGF5b3V0CmAoImFubm91bmNlIiwgc2NoZW1lX2lkLCBzdGVhbHRoX2FkZHJlc3MpYCB3aXRoCmAoY2FsbGVyLCBlcGhlbWVyYWxfcHViX2tleSwgbWV0YWRhdGEpYC4gRG8gbm90IHJlaW50ZXJwcmV0IGhpc3RvcmljYWwgdjEKZXZlbnRzIGFzIHYyLiBUaGUgY29tcGF0aWJpbGl0eSBwYXRoIGlzIGEgbmV3IGFubm91bmNlciBkZXBsb3ltZW50IHVzaW5nCmBzY2hlbWVfaWQgPSAyYC4KCiMgQXJndW1lbnRzCiogYHNjaGVtZV9pZGAgLSBNdXN0IGJlIGAyYCBmb3IgdGhlIHYyIFN0AAAACGFubm91bmNlAAAABAAAAAAAAAAJc2NoZW1lX2lkAAAAAAAABAAAAAAAAAAPc3RlYWx0aF9hZGRyZXNzAAAAABMAAAAAAAAAEWVwaGVtZXJhbF9wdWJfa2V5AAAAAAAD7gAAACAAAAAAAAAACG1ldGFkYXRhAAAADgAAAAA=" ]),
      options
    )
  }
  public readonly fromJSON = {
    announce: this.txFromJSON<null>
  }
}