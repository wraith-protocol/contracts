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




export type DataKey = {tag: "Deposit", values: readonly [Buffer]} | {tag: "Announcer", values: void} | {tag: "Admin", values: void} | {tag: "Paused", values: void} | {tag: "GracePeriod", values: void};

export const VaultError = {
  1: {message:"AlreadyInitialized"},
  2: {message:"NotInitialized"},
  3: {message:"InvalidWindow"},
  4: {message:"DepositNotFound"},
  5: {message:"NotYetUnlocked"},
  6: {message:"NotYetRefundable"},
  7: {message:"WrongRecipient"},
  /**
   * The contract is paused; `deposit` is unavailable until an admin unpauses.
   */
  8: {message:"Paused"},
  /**
   * The permissionless refund window has not opened yet.
   */
  9: {message:"NotYetPermissionless"},
  /**
   * `set_grace_period` was called with zero.
   */
  10: {message:"InvalidGracePeriod"}
}


export interface DepositEntry {
  amount: i128;
  asset: string;
  recipient: string;
  refund_after: u32;
  sender: string;
  unlock_ledger: u32;
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
   * Initialise the vault with its pause admin and announcer address.
   * 
   * Must be called exactly once before any `deposit`. The grace period is
   * seeded to `DEFAULT_GRACE_PERIOD` and can be retuned by the admin.
   */
  init: ({admin, announcer}: {admin: string, announcer: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a admin transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the pause admin.
   */
  admin: (options?: MethodOptions) => Promise<AssembledTransaction<Result<string>>>

  /**
   * Construct and simulate a claim transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Claim a deposit. Callable while paused so recipients can always exit.
   */
  claim: ({deposit_id, recipient}: {deposit_id: Buffer, recipient: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a pause transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Pause the contract — admin only.
   * 
   * Blocks new deposits. `claim`, `refund`, and `refund_permissionless`
   * stay callable so depositors and recipients can always exit.
   */
  pause: ({caller}: {caller: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a refund transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Refund an unclaimed deposit to its depositor. Callable while paused.
   * 
   * Requires the depositor's authorisation and opens at `refund_after`.
   */
  refund: ({deposit_id}: {deposit_id: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a deposit transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  deposit: ({sender, recipient, amount, asset, unlock_ledger, refund_after, ephemeral_pub_key}: {sender: string, recipient: string, amount: i128, asset: string, unlock_ledger: u32, refund_after: u32, ephemeral_pub_key: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Buffer>>>

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
   * Construct and simulate a get_deposit transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Look up a stored deposit without mutating it.
   */
  get_deposit: ({deposit_id}: {deposit_id: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<DepositEntry>>>

  /**
   * Construct and simulate a grace_period transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the configured grace period in ledgers.
   */
  grace_period: (options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a set_grace_period transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Retune the grace period — admin only.
   * 
   * Applies to deposits made after the change; already-stored deposits keep
   * the absolute `unlock_ledger` / `refund_after` they were created with,
   * but their permissionless refund window moves with the new value.
   */
  set_grace_period: ({caller, grace_period}: {caller: string, grace_period: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a refund_permissionless transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Refund an unclaimed deposit to its depositor without the depositor's
   * signature. Callable while paused.
   * 
   * Opens one grace period after `refund_after`, so a depositor who has lost
   * access to their key cannot strand funds in the vault forever. Funds
   * always go to the recorded depositor — `caller` only pays the fee and is
   * authorised so the invocation is attributable.
   */
  refund_permissionless: ({caller, deposit_id}: {caller: string, deposit_id: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

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
      new ContractSpec([ "AAAAAAAAAMlJbml0aWFsaXNlIHRoZSB2YXVsdCB3aXRoIGl0cyBwYXVzZSBhZG1pbiBhbmQgYW5ub3VuY2VyIGFkZHJlc3MuCgpNdXN0IGJlIGNhbGxlZCBleGFjdGx5IG9uY2UgYmVmb3JlIGFueSBgZGVwb3NpdGAuIFRoZSBncmFjZSBwZXJpb2QgaXMKc2VlZGVkIHRvIGBERUZBVUxUX0dSQUNFX1BFUklPRGAgYW5kIGNhbiBiZSByZXR1bmVkIGJ5IHRoZSBhZG1pbi4AAAAAAAAEaW5pdAAAAAIAAAAAAAAABWFkbWluAAAAAAAAEwAAAAAAAAAJYW5ub3VuY2VyAAAAAAAAEwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAClZhdWx0RXJyb3IAAA==",
        "AAAAAAAAABhSZXR1cm5zIHRoZSBwYXVzZSBhZG1pbi4AAAAFYWRtaW4AAAAAAAAAAAAAAQAAA+kAAAATAAAH0AAAAApWYXVsdEVycm9yAAA=",
        "AAAAAAAAAEVDbGFpbSBhIGRlcG9zaXQuIENhbGxhYmxlIHdoaWxlIHBhdXNlZCBzbyByZWNpcGllbnRzIGNhbiBhbHdheXMgZXhpdC4AAAAAAAAFY2xhaW0AAAAAAAACAAAAAAAAAApkZXBvc2l0X2lkAAAAAAPuAAAAIAAAAAAAAAAJcmVjaXBpZW50AAAAAAAAEwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAClZhdWx0RXJyb3IAAA==",
        "AAAAAAAAAKNQYXVzZSB0aGUgY29udHJhY3Qg4oCUIGFkbWluIG9ubHkuCgpCbG9ja3MgbmV3IGRlcG9zaXRzLiBgY2xhaW1gLCBgcmVmdW5kYCwgYW5kIGByZWZ1bmRfcGVybWlzc2lvbmxlc3NgCnN0YXkgY2FsbGFibGUgc28gZGVwb3NpdG9ycyBhbmQgcmVjaXBpZW50cyBjYW4gYWx3YXlzIGV4aXQuAAAAAAVwYXVzZQAAAAAAAAEAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAClZhdWx0RXJyb3IAAA==",
        "AAAAAAAAAIlSZWZ1bmQgYW4gdW5jbGFpbWVkIGRlcG9zaXQgdG8gaXRzIGRlcG9zaXRvci4gQ2FsbGFibGUgd2hpbGUgcGF1c2VkLgoKUmVxdWlyZXMgdGhlIGRlcG9zaXRvcidzIGF1dGhvcmlzYXRpb24gYW5kIG9wZW5zIGF0IGByZWZ1bmRfYWZ0ZXJgLgAAAAAAAAZyZWZ1bmQAAAAAAAEAAAAAAAAACmRlcG9zaXRfaWQAAAAAA+4AAAAgAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAKVmF1bHRFcnJvcgAA",
        "AAAAAAAAAAAAAAAHZGVwb3NpdAAAAAAHAAAAAAAAAAZzZW5kZXIAAAAAABMAAAAAAAAACXJlY2lwaWVudAAAAAAAABMAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAAFYXNzZXQAAAAAAAATAAAAAAAAAA11bmxvY2tfbGVkZ2VyAAAAAAAABAAAAAAAAAAMcmVmdW5kX2FmdGVyAAAABAAAAAAAAAARZXBoZW1lcmFsX3B1Yl9rZXkAAAAAAAPuAAAAIAAAAAEAAAPpAAAD7gAAACAAAAfQAAAAClZhdWx0RXJyb3IAAA==",
        "AAAAAAAAACRVbnBhdXNlIHRoZSBjb250cmFjdCDigJQgYWRtaW4gb25seS4AAAAHdW5wYXVzZQAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAAApWYXVsdEVycm9yAAA=",
        "AAAAAAAAACdSZXR1cm5zIHRydWUgaWYgdGhlIGNvbnRyYWN0IGlzIHBhdXNlZC4AAAAACWlzX3BhdXNlZAAAAAAAAAAAAAABAAAAAQ==",
        "AAAAAgAAAAAAAAAAAAAAB0RhdGFLZXkAAAAABQAAAAEAAAAoTWFwcyBhIGRlcG9zaXQgaWQgdG8gaXRzIGBEZXBvc2l0RW50cnlgLgAAAAdEZXBvc2l0AAAAAAEAAAPuAAAAIAAAAAAAAAAqQWRkcmVzcyBvZiB0aGUgc3RlYWx0aCBhbm5vdW5jZXIgY29udHJhY3QuAAAAAAAJQW5ub3VuY2VyAAAAAAAAAAAAABRQYXVzZSBhZG1pbiBhZGRyZXNzLgAAAAVBZG1pbgAAAAAAAAAAAAAfV2hldGhlciB0aGUgY29udHJhY3QgaXMgcGF1c2VkLgAAAAAGUGF1c2VkAAAAAAAAAAAAJkNvbmZpZ3VyYWJsZSBncmFjZSBwZXJpb2QsIGluIGxlZGdlcnMuAAAAAAALR3JhY2VQZXJpb2QA",
        "AAAAAAAAAC1Mb29rIHVwIGEgc3RvcmVkIGRlcG9zaXQgd2l0aG91dCBtdXRhdGluZyBpdC4AAAAAAAALZ2V0X2RlcG9zaXQAAAAAAQAAAAAAAAAKZGVwb3NpdF9pZAAAAAAD7gAAACAAAAABAAAD6QAAB9AAAAAMRGVwb3NpdEVudHJ5AAAH0AAAAApWYXVsdEVycm9yAAA=",
        "AAAAAAAAAC9SZXR1cm5zIHRoZSBjb25maWd1cmVkIGdyYWNlIHBlcmlvZCBpbiBsZWRnZXJzLgAAAAAMZ3JhY2VfcGVyaW9kAAAAAAAAAAEAAAAE",
        "AAAABAAAAAAAAAAAAAAAClZhdWx0RXJyb3IAAAAAAAoAAAAAAAAAEkFscmVhZHlJbml0aWFsaXplZAAAAAAAAQAAAAAAAAAOTm90SW5pdGlhbGl6ZWQAAAAAAAIAAAAAAAAADUludmFsaWRXaW5kb3cAAAAAAAADAAAAAAAAAA9EZXBvc2l0Tm90Rm91bmQAAAAABAAAAAAAAAAOTm90WWV0VW5sb2NrZWQAAAAAAAUAAAAAAAAAEE5vdFlldFJlZnVuZGFibGUAAAAGAAAAAAAAAA5Xcm9uZ1JlY2lwaWVudAAAAAAABwAAAElUaGUgY29udHJhY3QgaXMgcGF1c2VkOyBgZGVwb3NpdGAgaXMgdW5hdmFpbGFibGUgdW50aWwgYW4gYWRtaW4gdW5wYXVzZXMuAAAAAAAABlBhdXNlZAAAAAAACAAAADRUaGUgcGVybWlzc2lvbmxlc3MgcmVmdW5kIHdpbmRvdyBoYXMgbm90IG9wZW5lZCB5ZXQuAAAAFE5vdFlldFBlcm1pc3Npb25sZXNzAAAACQAAAChgc2V0X2dyYWNlX3BlcmlvZGAgd2FzIGNhbGxlZCB3aXRoIHplcm8uAAAAEkludmFsaWRHcmFjZVBlcmlvZAAAAAAACg==",
        "AAAAAQAAAAAAAAAAAAAADERlcG9zaXRFbnRyeQAAAAYAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAAFYXNzZXQAAAAAAAATAAAAAAAAAAlyZWNpcGllbnQAAAAAAAATAAAAAAAAAAxyZWZ1bmRfYWZ0ZXIAAAAEAAAAAAAAAAZzZW5kZXIAAAAAABMAAAAAAAAADXVubG9ja19sZWRnZXIAAAAAAAAE",
        "AAAAAAAAAPdSZXR1bmUgdGhlIGdyYWNlIHBlcmlvZCDigJQgYWRtaW4gb25seS4KCkFwcGxpZXMgdG8gZGVwb3NpdHMgbWFkZSBhZnRlciB0aGUgY2hhbmdlOyBhbHJlYWR5LXN0b3JlZCBkZXBvc2l0cyBrZWVwCnRoZSBhYnNvbHV0ZSBgdW5sb2NrX2xlZGdlcmAgLyBgcmVmdW5kX2FmdGVyYCB0aGV5IHdlcmUgY3JlYXRlZCB3aXRoLApidXQgdGhlaXIgcGVybWlzc2lvbmxlc3MgcmVmdW5kIHdpbmRvdyBtb3ZlcyB3aXRoIHRoZSBuZXcgdmFsdWUuAAAAABBzZXRfZ3JhY2VfcGVyaW9kAAAAAgAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAAxncmFjZV9wZXJpb2QAAAAEAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAKVmF1bHRFcnJvcgAA",
        "AAAAAAAAAWxSZWZ1bmQgYW4gdW5jbGFpbWVkIGRlcG9zaXQgdG8gaXRzIGRlcG9zaXRvciB3aXRob3V0IHRoZSBkZXBvc2l0b3IncwpzaWduYXR1cmUuIENhbGxhYmxlIHdoaWxlIHBhdXNlZC4KCk9wZW5zIG9uZSBncmFjZSBwZXJpb2QgYWZ0ZXIgYHJlZnVuZF9hZnRlcmAsIHNvIGEgZGVwb3NpdG9yIHdobyBoYXMgbG9zdAphY2Nlc3MgdG8gdGhlaXIga2V5IGNhbm5vdCBzdHJhbmQgZnVuZHMgaW4gdGhlIHZhdWx0IGZvcmV2ZXIuIEZ1bmRzCmFsd2F5cyBnbyB0byB0aGUgcmVjb3JkZWQgZGVwb3NpdG9yIOKAlCBgY2FsbGVyYCBvbmx5IHBheXMgdGhlIGZlZSBhbmQgaXMKYXV0aG9yaXNlZCBzbyB0aGUgaW52b2NhdGlvbiBpcyBhdHRyaWJ1dGFibGUuAAAAFXJlZnVuZF9wZXJtaXNzaW9ubGVzcwAAAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAKZGVwb3NpdF9pZAAAAAAD7gAAACAAAAABAAAD6QAAA+0AAAAAAAAH0AAAAApWYXVsdEVycm9yAAA=",
        "AAAAAQAAAKpXcmFpdGggUHJvdG9jb2wgc3RhbmRhcmQgbWV0cmljIGV2ZW50IHNjaGVtYS4KCkFsbCBXcmFpdGggY29udHJhY3RzIGVtaXQgbWV0cmljIGV2ZW50cyB1c2luZyB0aGlzIHN0cnVjdHVyZSB0byBlbmFibGUKc3RhbmRhcmRpemVkIG9mZi1jaGFpbiBvYnNlcnZhYmlsaXR5IGFuZCBtb25pdG9yaW5nLgAAAAAAAAAAABFXcmFpdGhNZXRyaWNFdmVudAAAAAAAAAQAAABAQ29udHJhY3QgaWRlbnRpZmllciAoZS5nLiwgInN0ZWFsdGgtcmVnaXN0cnkiLCAic3RlYWx0aC1zZW5kZXIiKQAAAAhjb250cmFjdAAAABEAAABLT3B0aW9uYWwgZGltZW5zaW9ucyBmb3IgZmlsdGVyaW5nL2dyb3VwaW5nIChlLmcuLCB0b2tlbl9hZGRyZXNzLCBzY2hlbWVfaWQpAAAAAApkaW1lbnNpb25zAAAAAAPqAAAD7QAAAAIAAAARAAAAAAAAADNNZXRyaWMgbmFtZSAoZS5nLiwgInJlZ2lzdGVyX2NvdW50IiwgInNlbmRfdm9sdW1lIikAAAAAC21ldHJpY19uYW1lAAAAABEAAAAbTnVtZXJpYyB2YWx1ZSBvZiB0aGUgbWV0cmljAAAAAAV2YWx1ZQAAAAAAAAs=" ]),
      options
    )
  }
  public readonly fromJSON = {
    init: this.txFromJSON<Result<void>>,
        admin: this.txFromJSON<Result<string>>,
        claim: this.txFromJSON<Result<void>>,
        pause: this.txFromJSON<Result<void>>,
        refund: this.txFromJSON<Result<void>>,
        deposit: this.txFromJSON<Result<Buffer>>,
        unpause: this.txFromJSON<Result<void>>,
        is_paused: this.txFromJSON<boolean>,
        get_deposit: this.txFromJSON<Result<DepositEntry>>,
        grace_period: this.txFromJSON<u32>,
        set_grace_period: this.txFromJSON<Result<void>>,
        refund_permissionless: this.txFromJSON<Result<void>>
  }
}