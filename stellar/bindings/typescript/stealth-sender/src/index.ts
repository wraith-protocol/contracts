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
export type DataKey = {tag: "Announcer", values: void};

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
  3: {message:"LengthMismatch"}
}

export interface Client {
  /**
   * Construct and simulate a init transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Initialise the contract by storing the announcer address.
   * 
   * Must be called exactly once before any `send` or `batch_send`.
   */
  init: ({announcer}: {announcer: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

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
      new ContractSpec([ "AAAAAAAAAHlJbml0aWFsaXNlIHRoZSBjb250cmFjdCBieSBzdG9yaW5nIHRoZSBhbm5vdW5jZXIgYWRkcmVzcy4KCk11c3QgYmUgY2FsbGVkIGV4YWN0bHkgb25jZSBiZWZvcmUgYW55IGBzZW5kYCBvciBgYmF0Y2hfc2VuZGAuAAAAAAAABGluaXQAAAABAAAAAAAAAAlhbm5vdW5jZXIAAAAAAAATAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAALU2VuZGVyRXJyb3IA",
        "AAAAAAAAAglUcmFuc2ZlciB0b2tlbnMgdG8gYSBzdGVhbHRoIGFkZHJlc3MgYW5kIGVtaXQgYW4gYW5ub3VuY2VtZW50LgoKIyBBcmd1bWVudHMKKiBgc2VuZGVyYCAgICAgICAgICAgIC0gVGhlIGFkZHJlc3Mgc2VuZGluZyBmdW5kcyAobXVzdCBhdXRob3Jpc2UpLgoqIGB0b2tlbmAgICAgICAgICAgICAgLSBTQUMgdG9rZW4gY29udHJhY3QgYWRkcmVzcyAod29ya3MgZm9yIG5hdGl2ZSBYTE0gdG9vKS4KKiBgYW1vdW50YCAgICAgICAgICAgIC0gQW1vdW50IG9mIHRva2VucyB0byB0cmFuc2Zlci4KKiBgc2NoZW1lX2lkYCAgICAgICAgIC0gU3RlYWx0aCBhZGRyZXNzIHNjaGVtZSBpZGVudGlmaWVyLgoqIGBzdGVhbHRoX2FkZHJlc3NgICAgLSBUaGUgZGVyaXZlZCBvbmUtdGltZSBzdGVhbHRoIGFkZHJlc3MuCiogYGVwaGVtZXJhbF9wdWJfa2V5YCAtIEVwaGVtZXJhbCBwdWJsaWMga2V5IGZvciB0aGUgcmVjaXBpZW50IHRvIHNjYW4uCiogYG1ldGFkYXRhYCAgICAgICAgICAtIEV4dHJhIGRhdGEgKGUuZy4gdmlldyB0YWcpLgAAAAAAAARzZW5kAAAABwAAAAAAAAAGc2VuZGVyAAAAAAATAAAAAAAAAAV0b2tlbgAAAAAAABMAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAAJc2NoZW1lX2lkAAAAAAAABAAAAAAAAAAPc3RlYWx0aF9hZGRyZXNzAAAAABMAAAAAAAAAEWVwaGVtZXJhbF9wdWJfa2V5AAAAAAAD7gAAACAAAAAAAAAACG1ldGFkYXRhAAAADgAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAC1NlbmRlckVycm9yAA==",
        "AAAAAgAAAA1TdG9yYWdlIGtleXMuAAAAAAAAAAAAAAdEYXRhS2V5AAAAAAEAAAAAAAAANlRoZSBhZGRyZXNzIG9mIHRoZSBkZXBsb3llZCBTdGVhbHRoQW5ub3VuY2VyIGNvbnRyYWN0LgAAAAAACUFubm91bmNlcgAAAA==",
        "AAAAAAAAAJxCYXRjaCB2ZXJzaW9uIG9mIGBzZW5kYCDigJQgdHJhbnNmZXJzIHRva2VucyB0byBtdWx0aXBsZSBzdGVhbHRoIGFkZHJlc3NlcwphbmQgZW1pdHMgYW4gYW5ub3VuY2VtZW50IGZvciBlYWNoLgoKQWxsIGlucHV0IHZlY3RvcnMgbXVzdCBoYXZlIHRoZSBzYW1lIGxlbmd0aC4AAAAKYmF0Y2hfc2VuZAAAAAAABwAAAAAAAAAGc2VuZGVyAAAAAAATAAAAAAAAAAV0b2tlbgAAAAAAABMAAAAAAAAACXNjaGVtZV9pZAAAAAAAAAQAAAAAAAAAEXN0ZWFsdGhfYWRkcmVzc2VzAAAAAAAD6gAAABMAAAAAAAAAEmVwaGVtZXJhbF9wdWJfa2V5cwAAAAAD6gAAA+4AAAAgAAAAAAAAAAltZXRhZGF0YXMAAAAAAAPqAAAADgAAAAAAAAAHYW1vdW50cwAAAAPqAAAACwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAC1NlbmRlckVycm9yAA==",
        "AAAABAAAACxFcnJvcnMgdGhhdCB0aGUgc2VuZGVyIGNvbnRyYWN0IGNhbiBwcm9kdWNlLgAAAAAAAAALU2VuZGVyRXJyb3IAAAAAAwAAACpUaGUgY29udHJhY3QgaGFzIGFscmVhZHkgYmVlbiBpbml0aWFsaXNlZC4AAAAAABJBbHJlYWR5SW5pdGlhbGl6ZWQAAAAAAAEAAAAqVGhlIGNvbnRyYWN0IGhhcyBub3QgYmVlbiBpbml0aWFsaXNlZCB5ZXQuAAAAAAAOTm90SW5pdGlhbGl6ZWQAAAAAAAIAAAAwVGhlIGJhdGNoIGlucHV0IHZlY3RvcnMgaGF2ZSBtaXNtYXRjaGVkIGxlbmd0aHMuAAAADkxlbmd0aE1pc21hdGNoAAAAAAAD" ]),
      options
    )
  }
  public readonly fromJSON = {
    init: this.txFromJSON<Result<void>>,
        send: this.txFromJSON<Result<void>>,
        batch_send: this.txFromJSON<Result<void>>
  }
}