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

if (typeof Reflect.get(globalThis, "Buffer") === "undefined") {
  Object.assign(globalThis, { Buffer });
}





/**
 * A single vote cast by a token holder.
 */
export interface Vote {
  /**
 * true = for, false = against.
 */
support: boolean;
  /**
 * Voting weight (token balance at vote time).
 */
weight: i128;
}

export type DataKey = {tag: "Admin", values: void} | {tag: "Token", values: void} | {tag: "Quorum", values: void} | {tag: "VotingPeriod", values: void} | {tag: "Timelock", values: void} | {tag: "NextProposalId", values: void} | {tag: "Proposal", values: readonly [u32]} | {tag: "Vote", values: readonly [u32, string]};


/**
 * A governance proposal.
 */
export interface Proposal {
  /**
 * Total tokens voted against.
 */
against_votes: i128;
  /**
 * Raw argument bytes forwarded to the target function.
 */
args: Buffer;
  /**
 * Whether the proposal has been cancelled.
 */
cancelled: boolean;
  /**
 * Human-readable description.
 */
description: string;
  /**
 * Ledger at which voting closes.
 */
end_ledger: u32;
  /**
 * Whether the proposal has been executed.
 */
executed: boolean;
  /**
 * Total tokens voted in favour.
 */
for_votes: i128;
  /**
 * Function to call on the target.
 */
function: string;
  /**
 * Sequential proposal ID.
 */
id: u32;
  /**
 * Address that created the proposal.
 */
proposer: string;
  /**
 * Ledger at which voting opens.
 */
start_ledger: u32;
  /**
 * Contract to call on execution.
 */
target: string;
}

export const GovernanceError = {
  1: {message:"AlreadyInitialized"},
  2: {message:"NotInitialized"},
  3: {message:"NotAdmin"},
  4: {message:"ProposalNotFound"},
  5: {message:"AlreadyVoted"},
  6: {message:"VotingNotActive"},
  7: {message:"VotingStillActive"},
  8: {message:"QuorumNotMet"},
  9: {message:"ProposalDefeated"},
  10: {message:"TimelockNotElapsed"},
  11: {message:"AlreadyExecuted"},
  12: {message:"AlreadyCancelled"},
  13: {message:"ExecutionFailed"},
  14: {message:"NoVotingPower"}
}


/**
 * Read-only governance configuration.
 */
export interface GovernanceConfig {
  quorum: i128;
  timelock: u32;
  token: string;
  voting_period: u32;
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
   * Initialise the governance contract.
   *
   * Must be called exactly once. After init the admin can create and cancel
   * proposals — in production this role should be renounced or replaced by
   * the governance process itself.
   *
   * # Arguments
   * * `admin`          — Address with super-admin powers (PoC only).
   * * `token`          — SAC token used for voting weight.
   * * `quorum`         — Absolute minimum total tokens required for a valid vote.
   * * `voting_period`  — Duration of voting window in ledgers.
   * * `timelock`       — Delay after voting closes before execution (ledgers).
   */
  init: ({admin, token, quorum, voting_period, timelock}: {admin: string, token: string, quorum: i128, voting_period: u32, timelock: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a vote transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Cast a vote on an active proposal.
   *
   * Voting weight equals the voter's token balance at the time the vote is
   * cast. Each address may vote once per proposal.
   *
   * # Arguments
   * * `voter`       — Address casting the vote (must auth).
   * * `proposal_id` — Target proposal.
   * * `support`     — `true` = for, `false` = against.
   */
  vote: ({voter, proposal_id, support}: {voter: string, proposal_id: u32, support: boolean}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a cancel transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Cancel a proposal.
   *
   * Rules (PoC):
   * - During voting: only the admin may cancel.
   * - After voting, without quorum: anyone may cancel (failed proposal).
   * - After voting, with quorum: only the admin may cancel (emergency
   * override — a production system would remove this power).
   */
  cancel: ({proposal_id}: {proposal_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a execute transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Execute a proposal that has passed.
   *
   * Conditions (all must hold):
   * 1. Voting window has closed.
   * 2. Timelock delay has elapsed since voting closed.
   * 3. Total votes cast >= quorum.
   * 4. `for_votes > against_votes`.
   * 5. Not already executed or cancelled.
   */
  execute: ({proposal_id}: {proposal_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a propose transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Create a new governance proposal.
   *
   * Anyone may propose. The proposer must authorise the call.
   *
   * # Arguments
   * * `proposer`    — Address creating the proposal (must auth).
   * * `target`      — Contract to call on execution.
   * * `function`    — Function name to invoke on the target.
   * * `args`        — Raw argument bytes forwarded to the target.
   * * `description` — Human-readable proposal description.
   *
   * # Returns
   * The new proposal ID.
   */
  propose: (args: {proposer: string, target: string, function: string, args: Buffer, description: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u32>>>

  /**
   * Construct and simulate a get_vote transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Return the vote record for a given voter on a proposal.
   */
  get_vote: ({proposal_id, voter}: {proposal_id: u32, voter: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Vote>>>

  /**
   * Construct and simulate a get_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Return the current governance configuration.
   */
  get_config: (options?: MethodOptions) => Promise<AssembledTransaction<Result<GovernanceConfig>>>

  /**
   * Construct and simulate a get_proposal transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Return a proposal by ID.
   */
  get_proposal: ({proposal_id}: {proposal_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Proposal>>>

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
      new ContractSpec([ "AAAAAAAAAjdJbml0aWFsaXNlIHRoZSBnb3Zlcm5hbmNlIGNvbnRyYWN0LgoKTXVzdCBiZSBjYWxsZWQgZXhhY3RseSBvbmNlLiBBZnRlciBpbml0IHRoZSBhZG1pbiBjYW4gY3JlYXRlIGFuZCBjYW5jZWwKcHJvcG9zYWxzIOKAlCBpbiBwcm9kdWN0aW9uIHRoaXMgcm9sZSBzaG91bGQgYmUgcmVub3VuY2VkIG9yIHJlcGxhY2VkIGJ5CnRoZSBnb3Zlcm5hbmNlIHByb2Nlc3MgaXRzZWxmLgoKIyBBcmd1bWVudHMKKiBgYWRtaW5gICAgICAgICAgIOKAlCBBZGRyZXNzIHdpdGggc3VwZXItYWRtaW4gcG93ZXJzIChQb0Mgb25seSkuCiogYHRva2VuYCAgICAgICAgICDigJQgU0FDIHRva2VuIHVzZWQgZm9yIHZvdGluZyB3ZWlnaHQuCiogYHF1b3J1bWAgICAgICAgICDigJQgQWJzb2x1dGUgbWluaW11bSB0b3RhbCB0b2tlbnMgcmVxdWlyZWQgZm9yIGEgdmFsaWQgdm90ZS4KKiBgdm90aW5nX3BlcmlvZGAgIOKAlCBEdXJhdGlvbiBvZiB2b3Rpbmcgd2luZG93IGluIGxlZGdlcnMuCiogYHRpbWVsb2NrYCAgICAgICDigJQgRGVsYXkgYWZ0ZXIgdm90aW5nIGNsb3NlcyBiZWZvcmUgZXhlY3V0aW9uIChsZWRnZXJzKS4AAAAABGluaXQAAAAFAAAAAAAAAAVhZG1pbgAAAAAAABMAAAAAAAAABXRva2VuAAAAAAAAEwAAAAAAAAAGcXVvcnVtAAAAAAALAAAAAAAAAA12b3RpbmdfcGVyaW9kAAAAAAAABAAAAAAAAAAIdGltZWxvY2sAAAAEAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAPR292ZXJuYW5jZUVycm9yAA==",
        "AAAAAAAAATpDYXN0IGEgdm90ZSBvbiBhbiBhY3RpdmUgcHJvcG9zYWwuCgpWb3Rpbmcgd2VpZ2h0IGVxdWFscyB0aGUgdm90ZXIncyB0b2tlbiBiYWxhbmNlIGF0IHRoZSB0aW1lIHRoZSB2b3RlIGlzCmNhc3QuIEVhY2ggYWRkcmVzcyBtYXkgdm90ZSBvbmNlIHBlciBwcm9wb3NhbC4KCiMgQXJndW1lbnRzCiogYHZvdGVyYCAgICAgICDigJQgQWRkcmVzcyBjYXN0aW5nIHRoZSB2b3RlIChtdXN0IGF1dGgpLgoqIGBwcm9wb3NhbF9pZGAg4oCUIFRhcmdldCBwcm9wb3NhbC4KKiBgc3VwcG9ydGAgICAgIOKAlCBgdHJ1ZWAgPSBmb3IsIGBmYWxzZWAgPSBhZ2FpbnN0LgAAAAAABHZvdGUAAAADAAAAAAAAAAV2b3RlcgAAAAAAABMAAAAAAAAAC3Byb3Bvc2FsX2lkAAAAAAQAAAAAAAAAB3N1cHBvcnQAAAAAAQAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAD0dvdmVybmFuY2VFcnJvcgA=",
        "AAAAAAAAAQ5DYW5jZWwgYSBwcm9wb3NhbC4KClJ1bGVzIChQb0MpOgotIER1cmluZyB2b3Rpbmc6IG9ubHkgdGhlIGFkbWluIG1heSBjYW5jZWwuCi0gQWZ0ZXIgdm90aW5nLCB3aXRob3V0IHF1b3J1bTogYW55b25lIG1heSBjYW5jZWwgKGZhaWxlZCBwcm9wb3NhbCkuCi0gQWZ0ZXIgdm90aW5nLCB3aXRoIHF1b3J1bTogb25seSB0aGUgYWRtaW4gbWF5IGNhbmNlbCAoZW1lcmdlbmN5Cm92ZXJyaWRlIOKAlCBhIHByb2R1Y3Rpb24gc3lzdGVtIHdvdWxkIHJlbW92ZSB0aGlzIHBvd2VyKS4AAAAAAAZjYW5jZWwAAAAAAAEAAAAAAAAAC3Byb3Bvc2FsX2lkAAAAAAQAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA9Hb3Zlcm5hbmNlRXJyb3IA",
        "AAAAAQAAACVBIHNpbmdsZSB2b3RlIGNhc3QgYnkgYSB0b2tlbiBob2xkZXIuAAAAAAAAAAAAAARWb3RlAAAAAgAAABx0cnVlID0gZm9yLCBmYWxzZSA9IGFnYWluc3QuAAAAB3N1cHBvcnQAAAAAAQAAACtWb3Rpbmcgd2VpZ2h0ICh0b2tlbiBiYWxhbmNlIGF0IHZvdGUgdGltZSkuAAAAAAZ3ZWlnaHQAAAAAAAs=",
        "AAAAAAAAAPVFeGVjdXRlIGEgcHJvcG9zYWwgdGhhdCBoYXMgcGFzc2VkLgoKQ29uZGl0aW9ucyAoYWxsIG11c3QgaG9sZCk6CjEuIFZvdGluZyB3aW5kb3cgaGFzIGNsb3NlZC4KMi4gVGltZWxvY2sgZGVsYXkgaGFzIGVsYXBzZWQgc2luY2Ugdm90aW5nIGNsb3NlZC4KMy4gVG90YWwgdm90ZXMgY2FzdCA+PSBxdW9ydW0uCjQuIGBmb3Jfdm90ZXMgPiBhZ2FpbnN0X3ZvdGVzYC4KNS4gTm90IGFscmVhZHkgZXhlY3V0ZWQgb3IgY2FuY2VsbGVkLgAAAAAAAAdleGVjdXRlAAAAAAEAAAAAAAAAC3Byb3Bvc2FsX2lkAAAAAAQAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA9Hb3Zlcm5hbmNlRXJyb3IA",
        "AAAAAAAAAa9DcmVhdGUgYSBuZXcgZ292ZXJuYW5jZSBwcm9wb3NhbC4KCkFueW9uZSBtYXkgcHJvcG9zZS4gVGhlIHByb3Bvc2VyIG11c3QgYXV0aG9yaXNlIHRoZSBjYWxsLgoKIyBBcmd1bWVudHMKKiBgcHJvcG9zZXJgICAgIOKAlCBBZGRyZXNzIGNyZWF0aW5nIHRoZSBwcm9wb3NhbCAobXVzdCBhdXRoKS4KKiBgdGFyZ2V0YCAgICAgIOKAlCBDb250cmFjdCB0byBjYWxsIG9uIGV4ZWN1dGlvbi4KKiBgZnVuY3Rpb25gICAgIOKAlCBGdW5jdGlvbiBuYW1lIHRvIGludm9rZSBvbiB0aGUgdGFyZ2V0LgoqIGBhcmdzYCAgICAgICAg4oCUIFJhdyBhcmd1bWVudCBieXRlcyBmb3J3YXJkZWQgdG8gdGhlIHRhcmdldC4KKiBgZGVzY3JpcHRpb25gIOKAlCBIdW1hbi1yZWFkYWJsZSBwcm9wb3NhbCBkZXNjcmlwdGlvbi4KCiMgUmV0dXJucwpUaGUgbmV3IHByb3Bvc2FsIElELgAAAAAHcHJvcG9zZQAAAAAFAAAAAAAAAAhwcm9wb3NlcgAAABMAAAAAAAAABnRhcmdldAAAAAAAEwAAAAAAAAAIZnVuY3Rpb24AAAARAAAAAAAAAARhcmdzAAAADgAAAAAAAAALZGVzY3JpcHRpb24AAAAAEAAAAAEAAAPpAAAABAAAB9AAAAAPR292ZXJuYW5jZUVycm9yAA==",
        "AAAAAAAAADdSZXR1cm4gdGhlIHZvdGUgcmVjb3JkIGZvciBhIGdpdmVuIHZvdGVyIG9uIGEgcHJvcG9zYWwuAAAAAAhnZXRfdm90ZQAAAAIAAAAAAAAAC3Byb3Bvc2FsX2lkAAAAAAQAAAAAAAAABXZvdGVyAAAAAAAAEwAAAAEAAAPpAAAH0AAAAARWb3RlAAAH0AAAAA9Hb3Zlcm5hbmNlRXJyb3IA",
        "AAAAAgAAAAAAAAAAAAAAB0RhdGFLZXkAAAAACAAAAAAAAAAAAAAABUFkbWluAAAAAAAAAAAAAAAAAAAFVG9rZW4AAAAAAAAAAAAAAAAAAAZRdW9ydW0AAAAAAAAAAAAAAAAADFZvdGluZ1BlcmlvZAAAAAAAAAAAAAAACFRpbWVsb2NrAAAAAAAAAAAAAAAOTmV4dFByb3Bvc2FsSWQAAAAAAAEAAAAAAAAACFByb3Bvc2FsAAAAAQAAAAQAAAABAAAAAAAAAARWb3RlAAAAAgAAAAQAAAAT",
        "AAAAAAAAACxSZXR1cm4gdGhlIGN1cnJlbnQgZ292ZXJuYW5jZSBjb25maWd1cmF0aW9uLgAAAApnZXRfY29uZmlnAAAAAAAAAAAAAQAAA+kAAAfQAAAAEEdvdmVybmFuY2VDb25maWcAAAfQAAAAD0dvdmVybmFuY2VFcnJvcgA=",
        "AAAAAQAAABZBIGdvdmVybmFuY2UgcHJvcG9zYWwuAAAAAAAAAAAACFByb3Bvc2FsAAAADAAAABtUb3RhbCB0b2tlbnMgdm90ZWQgYWdhaW5zdC4AAAAADWFnYWluc3Rfdm90ZXMAAAAAAAALAAAANFJhdyBhcmd1bWVudCBieXRlcyBmb3J3YXJkZWQgdG8gdGhlIHRhcmdldCBmdW5jdGlvbi4AAAAEYXJncwAAAA4AAAAoV2hldGhlciB0aGUgcHJvcG9zYWwgaGFzIGJlZW4gY2FuY2VsbGVkLgAAAAljYW5jZWxsZWQAAAAAAAABAAAAG0h1bWFuLXJlYWRhYmxlIGRlc2NyaXB0aW9uLgAAAAALZGVzY3JpcHRpb24AAAAAEAAAAB5MZWRnZXIgYXQgd2hpY2ggdm90aW5nIGNsb3Nlcy4AAAAAAAplbmRfbGVkZ2VyAAAAAAAEAAAAJ1doZXRoZXIgdGhlIHByb3Bvc2FsIGhhcyBiZWVuIGV4ZWN1dGVkLgAAAAAIZXhlY3V0ZWQAAAABAAAAHVRvdGFsIHRva2VucyB2b3RlZCBpbiBmYXZvdXIuAAAAAAAACWZvcl92b3RlcwAAAAAAAAsAAAAfRnVuY3Rpb24gdG8gY2FsbCBvbiB0aGUgdGFyZ2V0LgAAAAAIZnVuY3Rpb24AAAARAAAAF1NlcXVlbnRpYWwgcHJvcG9zYWwgSUQuAAAAAAJpZAAAAAAABAAAACJBZGRyZXNzIHRoYXQgY3JlYXRlZCB0aGUgcHJvcG9zYWwuAAAAAAAIcHJvcG9zZXIAAAATAAAAHUxlZGdlciBhdCB3aGljaCB2b3Rpbmcgb3BlbnMuAAAAAAAADHN0YXJ0X2xlZGdlcgAAAAQAAAAeQ29udHJhY3QgdG8gY2FsbCBvbiBleGVjdXRpb24uAAAAAAAGdGFyZ2V0AAAAAAAT",
        "AAAAAAAAABhSZXR1cm4gYSBwcm9wb3NhbCBieSBJRC4AAAAMZ2V0X3Byb3Bvc2FsAAAAAQAAAAAAAAALcHJvcG9zYWxfaWQAAAAABAAAAAEAAAPpAAAH0AAAAAhQcm9wb3NhbAAAB9AAAAAPR292ZXJuYW5jZUVycm9yAA==",
        "AAAABAAAAAAAAAAAAAAAD0dvdmVybmFuY2VFcnJvcgAAAAAOAAAAAAAAABJBbHJlYWR5SW5pdGlhbGl6ZWQAAAAAAAEAAAAAAAAADk5vdEluaXRpYWxpemVkAAAAAAACAAAAAAAAAAhOb3RBZG1pbgAAAAMAAAAAAAAAEFByb3Bvc2FsTm90Rm91bmQAAAAEAAAAAAAAAAxBbHJlYWR5Vm90ZWQAAAAFAAAAAAAAAA9Wb3RpbmdOb3RBY3RpdmUAAAAABgAAAAAAAAARVm90aW5nU3RpbGxBY3RpdmUAAAAAAAAHAAAAAAAAAAxRdW9ydW1Ob3RNZXQAAAAIAAAAAAAAABBQcm9wb3NhbERlZmVhdGVkAAAACQAAAAAAAAASVGltZWxvY2tOb3RFbGFwc2VkAAAAAAAKAAAAAAAAAA9BbHJlYWR5RXhlY3V0ZWQAAAAACwAAAAAAAAAQQWxyZWFkeUNhbmNlbGxlZAAAAAwAAAAAAAAAD0V4ZWN1dGlvbkZhaWxlZAAAAAANAAAAAAAAAA1Ob1ZvdGluZ1Bvd2VyAAAAAAAADg==",
        "AAAAAQAAACNSZWFkLW9ubHkgZ292ZXJuYW5jZSBjb25maWd1cmF0aW9uLgAAAAAAAAAAEEdvdmVybmFuY2VDb25maWcAAAAEAAAAAAAAAAZxdW9ydW0AAAAAAAsAAAAAAAAACHRpbWVsb2NrAAAABAAAAAAAAAAFdG9rZW4AAAAAAAATAAAAAAAAAA12b3RpbmdfcGVyaW9kAAAAAAAABA==",
        "AAAAAQAAAKpXcmFpdGggUHJvdG9jb2wgc3RhbmRhcmQgbWV0cmljIGV2ZW50IHNjaGVtYS4KCkFsbCBXcmFpdGggY29udHJhY3RzIGVtaXQgbWV0cmljIGV2ZW50cyB1c2luZyB0aGlzIHN0cnVjdHVyZSB0byBlbmFibGUKc3RhbmRhcmRpemVkIG9mZi1jaGFpbiBvYnNlcnZhYmlsaXR5IGFuZCBtb25pdG9yaW5nLgAAAAAAAAAAABFXcmFpdGhNZXRyaWNFdmVudAAAAAAAAAQAAABAQ29udHJhY3QgaWRlbnRpZmllciAoZS5nLiwgInN0ZWFsdGgtcmVnaXN0cnkiLCAic3RlYWx0aC1zZW5kZXIiKQAAAAhjb250cmFjdAAAABEAAABLT3B0aW9uYWwgZGltZW5zaW9ucyBmb3IgZmlsdGVyaW5nL2dyb3VwaW5nIChlLmcuLCB0b2tlbl9hZGRyZXNzLCBzY2hlbWVfaWQpAAAAAApkaW1lbnNpb25zAAAAAAPqAAAD7QAAAAIAAAARAAAAAAAAADNNZXRyaWMgbmFtZSAoZS5nLiwgInJlZ2lzdGVyX2NvdW50IiwgInNlbmRfdm9sdW1lIikAAAAAC21ldHJpY19uYW1lAAAAABEAAAAbTnVtZXJpYyB2YWx1ZSBvZiB0aGUgbWV0cmljAAAAAAV2YWx1ZQAAAAAAAAs=" ]),
      options
    )
  }
  public readonly fromJSON = {
    init: this.txFromJSON<Result<void>>,
        vote: this.txFromJSON<Result<void>>,
        cancel: this.txFromJSON<Result<void>>,
        execute: this.txFromJSON<Result<void>>,
        propose: this.txFromJSON<Result<u32>>,
        get_vote: this.txFromJSON<Result<Vote>>,
        get_config: this.txFromJSON<Result<GovernanceConfig>>,
        get_proposal: this.txFromJSON<Result<Proposal>>
  }
}