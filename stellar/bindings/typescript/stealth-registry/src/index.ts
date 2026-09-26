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
export type DataKey = {tag: "MetaAddress", values: readonly [string, u32]};

/**
 * Errors that the registry can produce.
 */
export const RegistryError = {
  /**
   * The supplied stealth meta-address is not exactly 64 bytes.
   */
  1: {message:"InvalidMetaAddressLength"},
  /**
   * No stealth meta-address has been registered for the given address and scheme.
   */
  2: {message:"NotRegistered"}
}

export interface Client {
  /**
   * Construct and simulate a register_keys transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Register or update a stealth meta-address.
   * 
   * # Arguments
   * * `registrant` - The address whose meta-address is being set (must authorise).
   * * `scheme_id`  - The stealth address scheme identifier.
   * * `stealth_meta_address` - 64-byte value: `spending_pubkey || viewing_pubkey`.
   */
  register_keys: ({registrant, scheme_id, stealth_meta_address}: {registrant: string, scheme_id: u32, stealth_meta_address: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a stealth_meta_address_of transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Look up a previously registered stealth meta-address.
   * 
   * # Arguments
   * * `registrant` - The address to look up.
   * * `scheme_id`  - The stealth address scheme identifier.
   */
  stealth_meta_address_of: ({registrant, scheme_id}: {registrant: string, scheme_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Buffer>>>

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
      new ContractSpec([ "AAAAAgAAAA1TdG9yYWdlIGtleXMuAAAAAAAAAAAAAAdEYXRhS2V5AAAAAAEAAAABAAAAaU1hcHMgKHJlZ2lzdHJhbnQsIHNjaGVtZV9pZCkgdG8gdGhlaXIgc3RlYWx0aCBtZXRhLWFkZHJlc3MgKDY0IGJ5dGVzOgpzcGVuZGluZ19wdWJrZXkgfHwgdmlld2luZ19wdWJrZXkpLgAAAAAAAAtNZXRhQWRkcmVzcwAAAAACAAAAEwAAAAQ=",
        "AAAAAAAAAQ1SZWdpc3RlciBvciB1cGRhdGUgYSBzdGVhbHRoIG1ldGEtYWRkcmVzcy4KCiMgQXJndW1lbnRzCiogYHJlZ2lzdHJhbnRgIC0gVGhlIGFkZHJlc3Mgd2hvc2UgbWV0YS1hZGRyZXNzIGlzIGJlaW5nIHNldCAobXVzdCBhdXRob3Jpc2UpLgoqIGBzY2hlbWVfaWRgICAtIFRoZSBzdGVhbHRoIGFkZHJlc3Mgc2NoZW1lIGlkZW50aWZpZXIuCiogYHN0ZWFsdGhfbWV0YV9hZGRyZXNzYCAtIDY0LWJ5dGUgdmFsdWU6IGBzcGVuZGluZ19wdWJrZXkgfHwgdmlld2luZ19wdWJrZXlgLgAAAAAAAA1yZWdpc3Rlcl9rZXlzAAAAAAAAAwAAAAAAAAAKcmVnaXN0cmFudAAAAAAAEwAAAAAAAAAJc2NoZW1lX2lkAAAAAAAABAAAAAAAAAAUc3RlYWx0aF9tZXRhX2FkZHJlc3MAAAAOAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANUmVnaXN0cnlFcnJvcgAAAA==",
        "AAAABAAAACVFcnJvcnMgdGhhdCB0aGUgcmVnaXN0cnkgY2FuIHByb2R1Y2UuAAAAAAAAAAAAAA1SZWdpc3RyeUVycm9yAAAAAAAAAgAAADpUaGUgc3VwcGxpZWQgc3RlYWx0aCBtZXRhLWFkZHJlc3MgaXMgbm90IGV4YWN0bHkgNjQgYnl0ZXMuAAAAAAAYSW52YWxpZE1ldGFBZGRyZXNzTGVuZ3RoAAAAAQAAAE1ObyBzdGVhbHRoIG1ldGEtYWRkcmVzcyBoYXMgYmVlbiByZWdpc3RlcmVkIGZvciB0aGUgZ2l2ZW4gYWRkcmVzcyBhbmQgc2NoZW1lLgAAAAAAAA1Ob3RSZWdpc3RlcmVkAAAAAAAAAg==",
        "AAAAAAAAAKNMb29rIHVwIGEgcHJldmlvdXNseSByZWdpc3RlcmVkIHN0ZWFsdGggbWV0YS1hZGRyZXNzLgoKIyBBcmd1bWVudHMKKiBgcmVnaXN0cmFudGAgLSBUaGUgYWRkcmVzcyB0byBsb29rIHVwLgoqIGBzY2hlbWVfaWRgICAtIFRoZSBzdGVhbHRoIGFkZHJlc3Mgc2NoZW1lIGlkZW50aWZpZXIuAAAAABdzdGVhbHRoX21ldGFfYWRkcmVzc19vZgAAAAACAAAAAAAAAApyZWdpc3RyYW50AAAAAAATAAAAAAAAAAlzY2hlbWVfaWQAAAAAAAAEAAAAAQAAA+kAAAAOAAAH0AAAAA1SZWdpc3RyeUVycm9yAAAA" ]),
      options
    )
  }
  public readonly fromJSON = {
    register_keys: this.txFromJSON<Result<void>>,
        stealth_meta_address_of: this.txFromJSON<Result<Buffer>>
  }
}