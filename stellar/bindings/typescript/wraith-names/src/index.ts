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
export type DataKey = {tag: "Name", values: readonly [Buffer]} | {tag: "Reverse", values: readonly [Buffer]};


/**
 * A registered name entry.
 */
export interface NameEntry {
  /**
 * The human-readable name.
 */
name: string;
  /**
 * The registrant address (for auth).
 */
owner: string;
  /**
 * The 64-byte stealth meta-address (spending_pubkey || viewing_pubkey).
 */
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
  7: {message:"NotOwner"}
}

export interface Client {
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
   */
  resolve: ({name}: {name: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Buffer>>>

  /**
   * Construct and simulate a register transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Register a name mapped to a stealth meta-address.
   * The caller (owner) must authorize. Ownership is tied to the caller's address.
   * 
   * # Arguments
   * * `owner` - The address registering the name (must authorize).
   * * `name` - The human-readable name (lowercase alphanumeric, 3-32 chars).
   * * `stealth_meta_address` - 64-byte stealth meta-address.
   */
  register: ({owner, name, stealth_meta_address}: {owner: string, name: string, stealth_meta_address: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

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
      new ContractSpec([ "AAAAAAAAAFBVcGRhdGUgdGhlIG1ldGEtYWRkcmVzcyBmb3IgYW4gZXhpc3RpbmcgbmFtZS4KT25seSB0aGUgY3VycmVudCBvd25lciBjYW4gdXBkYXRlLgAAAAZ1cGRhdGUAAAAAAAMAAAAAAAAABW93bmVyAAAAAAAAEwAAAAAAAAAEbmFtZQAAABAAAAAAAAAAEG5ld19tZXRhX2FkZHJlc3MAAAAOAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAKTmFtZXNFcnJvcgAA",
        "AAAAAAAAAD9SZXZlcnNlIGxvb2t1cDogZmluZCB0aGUgbmFtZSBmb3IgYSBnaXZlbiBzdGVhbHRoIG1ldGEtYWRkcmVzcy4AAAAAB25hbWVfb2YAAAAAAQAAAAAAAAAUc3RlYWx0aF9tZXRhX2FkZHJlc3MAAAAOAAAAAQAAA+kAAAAQAAAH0AAAAApOYW1lc0Vycm9yAAA=",
        "AAAAAAAAACpSZWxlYXNlIGEgbmFtZSwgbWFraW5nIGl0IGF2YWlsYWJsZSBhZ2Fpbi4AAAAAAAdyZWxlYXNlAAAAAAIAAAAAAAAABW93bmVyAAAAAAAAEwAAAAAAAAAEbmFtZQAAABAAAAABAAAD6QAAA+0AAAAAAAAH0AAAAApOYW1lc0Vycm9yAAA=",
        "AAAAAAAAACtSZXNvbHZlIGEgbmFtZSB0byBpdHMgc3RlYWx0aCBtZXRhLWFkZHJlc3MuAAAAAAdyZXNvbHZlAAAAAAEAAAAAAAAABG5hbWUAAAAQAAAAAQAAA+kAAAAOAAAH0AAAAApOYW1lc0Vycm9yAAA=",
        "AAAAAAAAAU1SZWdpc3RlciBhIG5hbWUgbWFwcGVkIHRvIGEgc3RlYWx0aCBtZXRhLWFkZHJlc3MuClRoZSBjYWxsZXIgKG93bmVyKSBtdXN0IGF1dGhvcml6ZS4gT3duZXJzaGlwIGlzIHRpZWQgdG8gdGhlIGNhbGxlcidzIGFkZHJlc3MuCgojIEFyZ3VtZW50cwoqIGBvd25lcmAgLSBUaGUgYWRkcmVzcyByZWdpc3RlcmluZyB0aGUgbmFtZSAobXVzdCBhdXRob3JpemUpLgoqIGBuYW1lYCAtIFRoZSBodW1hbi1yZWFkYWJsZSBuYW1lIChsb3dlcmNhc2UgYWxwaGFudW1lcmljLCAzLTMyIGNoYXJzKS4KKiBgc3RlYWx0aF9tZXRhX2FkZHJlc3NgIC0gNjQtYnl0ZSBzdGVhbHRoIG1ldGEtYWRkcmVzcy4AAAAAAAAIcmVnaXN0ZXIAAAADAAAAAAAAAAVvd25lcgAAAAAAABMAAAAAAAAABG5hbWUAAAAQAAAAAAAAABRzdGVhbHRoX21ldGFfYWRkcmVzcwAAAA4AAAABAAAD6QAAA+0AAAAAAAAH0AAAAApOYW1lc0Vycm9yAAA=",
        "AAAAAgAAAA1TdG9yYWdlIGtleXMuAAAAAAAAAAAAAAdEYXRhS2V5AAAAAAIAAAABAAAAKU1hcHMgbmFtZSBoYXNoIChCeXRlc048MzI+KSB0byBOYW1lRW50cnkuAAAAAAAABE5hbWUAAAABAAAD7gAAACAAAAABAAAASVJldmVyc2UgbG9va3VwOiBtZXRhLWFkZHJlc3MgaGFzaCAoQnl0ZXNOPDMyPikgdG8gbmFtZSBoYXNoIChCeXRlc048MzI+KS4AAAAAAAAHUmV2ZXJzZQAAAAABAAAD7gAAACA=",
        "AAAAAQAAABhBIHJlZ2lzdGVyZWQgbmFtZSBlbnRyeS4AAAAAAAAACU5hbWVFbnRyeQAAAAAAAAMAAAAYVGhlIGh1bWFuLXJlYWRhYmxlIG5hbWUuAAAABG5hbWUAAAAQAAAAIlRoZSByZWdpc3RyYW50IGFkZHJlc3MgKGZvciBhdXRoKS4AAAAAAAVvd25lcgAAAAAAABMAAABFVGhlIDY0LWJ5dGUgc3RlYWx0aCBtZXRhLWFkZHJlc3MgKHNwZW5kaW5nX3B1YmtleSB8fCB2aWV3aW5nX3B1YmtleSkuAAAAAAAAFHN0ZWFsdGhfbWV0YV9hZGRyZXNzAAAADg==",
        "AAAABAAAAAdFcnJvcnMuAAAAAAAAAAAKTmFtZXNFcnJvcgAAAAAABwAAAAAAAAAJTmFtZVRha2VuAAAAAAAAAQAAAAAAAAAMTmFtZVRvb1Nob3J0AAAAAgAAAAAAAAALTmFtZVRvb0xvbmcAAAAAAwAAAAAAAAAUSW52YWxpZE5hbWVDaGFyYWN0ZXIAAAAEAAAAAAAAABJJbnZhbGlkTWV0YUFkZHJlc3MAAAAAAAUAAAAAAAAADE5hbWVOb3RGb3VuZAAAAAYAAAAAAAAACE5vdE93bmVyAAAABw==" ]),
      options
    )
  }
  public readonly fromJSON = {
    update: this.txFromJSON<Result<void>>,
        name_of: this.txFromJSON<Result<string>>,
        release: this.txFromJSON<Result<void>>,
        resolve: this.txFromJSON<Result<Buffer>>,
        register: this.txFromJSON<Result<void>>
  }
}