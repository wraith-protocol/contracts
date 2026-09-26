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
   * Emits a stealth address announcement event.
   * 
   * This is a pure event-emission function with no access control and no
   * storage. Indexers watch for these events to let recipients detect
   * incoming payments.
   * 
   * # Arguments
   * * `scheme_id` - Identifier for the stealth address scheme (e.g. 1 for the default DKSAP scheme).
   * * `stealth_address` - The one-time stealth address that received funds.
   * * `ephemeral_pub_key` - The ephemeral public key used to derive the stealth address.
   * * `metadata` - Arbitrary metadata (e.g. view tag) to speed up scanning.
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
      new ContractSpec([ "AAAAAAAAAhlFbWl0cyBhIHN0ZWFsdGggYWRkcmVzcyBhbm5vdW5jZW1lbnQgZXZlbnQuCgpUaGlzIGlzIGEgcHVyZSBldmVudC1lbWlzc2lvbiBmdW5jdGlvbiB3aXRoIG5vIGFjY2VzcyBjb250cm9sIGFuZCBubwpzdG9yYWdlLiBJbmRleGVycyB3YXRjaCBmb3IgdGhlc2UgZXZlbnRzIHRvIGxldCByZWNpcGllbnRzIGRldGVjdAppbmNvbWluZyBwYXltZW50cy4KCiMgQXJndW1lbnRzCiogYHNjaGVtZV9pZGAgLSBJZGVudGlmaWVyIGZvciB0aGUgc3RlYWx0aCBhZGRyZXNzIHNjaGVtZSAoZS5nLiAxIGZvciB0aGUgZGVmYXVsdCBES1NBUCBzY2hlbWUpLgoqIGBzdGVhbHRoX2FkZHJlc3NgIC0gVGhlIG9uZS10aW1lIHN0ZWFsdGggYWRkcmVzcyB0aGF0IHJlY2VpdmVkIGZ1bmRzLgoqIGBlcGhlbWVyYWxfcHViX2tleWAgLSBUaGUgZXBoZW1lcmFsIHB1YmxpYyBrZXkgdXNlZCB0byBkZXJpdmUgdGhlIHN0ZWFsdGggYWRkcmVzcy4KKiBgbWV0YWRhdGFgIC0gQXJiaXRyYXJ5IG1ldGFkYXRhIChlLmcuIHZpZXcgdGFnKSB0byBzcGVlZCB1cCBzY2FubmluZy4AAAAAAAAIYW5ub3VuY2UAAAAEAAAAAAAAAAlzY2hlbWVfaWQAAAAAAAAEAAAAAAAAAA9zdGVhbHRoX2FkZHJlc3MAAAAAEwAAAAAAAAARZXBoZW1lcmFsX3B1Yl9rZXkAAAAAAAPuAAAAIAAAAAAAAAAIbWV0YWRhdGEAAAAOAAAAAA==" ]),
      options
    )
  }
  public readonly fromJSON = {
    announce: this.txFromJSON<null>
  }
}