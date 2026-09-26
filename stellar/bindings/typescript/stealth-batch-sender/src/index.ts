// Auto-generated from stealth-batch-sender's Soroban contract specification.
import { Buffer } from "buffer";
import {
  AssembledTransaction,
  Client as ContractClient,
  ClientOptions as ContractClientOptions,
  MethodOptions,
  Result,
  Spec as ContractSpec,
} from "@stellar/stellar-sdk/contract";
import type { i128, u32, u64, Option } from "@stellar/stellar-sdk/contract";

export * from "@stellar/stellar-sdk";
export * as contract from "@stellar/stellar-sdk/contract";
export * as rpc from "@stellar/stellar-sdk/rpc";

if (typeof window !== "undefined") {
  // @ts-ignore Buffer exists
  window.Buffer = window.Buffer || Buffer;
}

export type DataKey =
  | { tag: "Admin"; values: void }
  | { tag: "Announcer"; values: void }
  | { tag: "AssetPolicy"; values: void }
  | { tag: "Paused"; values: void }
  | { tag: "MultisigSigners"; values: void }
  | { tag: "MultisigThreshold"; values: void }
  | { tag: "PendingRotation"; values: void };

export interface Transfer {
  stealth_address: string;
  ephemeral_pub_key: Buffer;
  amount: i128;
  metadata: Buffer;
}

export interface RotationProposal {
  new_signers: Array<string>;
  new_threshold: u32;
  executable_at: u64;
  approvals: Array<string>;
}

export const BatchSenderError = {
  1300: { message: "AlreadyInitialized" },
  1301: { message: "NotInitialized" },
  1302: { message: "EmptyBatch" },
  1303: { message: "BatchTooLarge" },
  1304: { message: "NonPositiveAmount" },
  1305: { message: "EmptyEphemeralKey" },
  1306: { message: "Paused" },
  1307: { message: "AssetNotAllowed" },
  1308: { message: "MultisigNotInitialized" },
  1309: { message: "MultisigAlreadyInitialized" },
  1310: { message: "NotSigner" },
  1311: { message: "InvalidThreshold" },
  1312: { message: "RotationAlreadyPending" },
  1313: { message: "NoPendingRotation" },
  1314: { message: "AlreadyApprovedRotation" },
  1315: { message: "QuorumNotMet" },
  1316: { message: "TimelockNotElapsed" },
} as const;

export interface Client {
  init: ({ admin, announcer, asset_policy }: { admin: string; announcer: string; asset_policy: Option<string> }, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>;
  pause: ({ caller }: { caller: string }, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>;
  unpause: ({ caller }: { caller: string }, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>;
  is_paused: (options?: MethodOptions) => Promise<AssembledTransaction<boolean>>;
  batch_send: ({ from, transfers, asset }: { from: string; transfers: Array<Transfer>; asset: string }, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>;
  max_batch_size: (options?: MethodOptions) => Promise<AssembledTransaction<u32>>;
  init_multisig: ({ signers, threshold }: { signers: Array<string>; threshold: u32 }, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>;
  signers: (options?: MethodOptions) => Promise<AssembledTransaction<Array<string>>>;
  threshold: (options?: MethodOptions) => Promise<AssembledTransaction<u32>>;
  pending_rotation: (options?: MethodOptions) => Promise<AssembledTransaction<Option<RotationProposal>>>;
  propose_rotate_signers: ({ caller, new_signers, new_threshold }: { caller: string; new_signers: Array<string>; new_threshold: u32 }, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>;
  approve_rotate_signers: ({ caller }: { caller: string }, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>;
  execute_rotate_signers: ({ caller }: { caller: string }, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>;
  cancel_rotate_signers: ({ caller }: { caller: string }, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>;
}

export class Client extends ContractClient {
  static async deploy<T = Client>(options: MethodOptions & Omit<ContractClientOptions, "contractId"> & { wasmHash: Buffer | string; salt?: Buffer | Uint8Array; format?: "hex" | "base64" }): Promise<AssembledTransaction<T>> {
    return ContractClient.deploy(null, options);
  }

  constructor(public readonly options: ContractClientOptions) {
    super(new ContractSpec([
      "AAAAAQAAAAAAAAAAAAAACFRyYW5zZmVyAAAABAAAAAAAAAAPc3RlYWx0aF9hZGRyZXNzAAAAABMAAAAAAAAAEWVwaGVtZXJhbF9wdWJfa2V5AAAAAAAADgAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAAhtZXRhZGF0YQAAAA4=",
      "AAAAAQAAAAAAAAAAAAAAEFJvdGF0aW9uUHJvcG9zYWwAAAAEAAAAAAAAAAtuZXdfc2lnbmVycwAAAAPqAAAAEwAAAAAAAAANbmV3X3RocmVzaG9sZAAAAAAAAAQAAAAAAAAADWV4ZWN1dGFibGVfYXQAAAAAAAAGAAAAAAAAAAlhcHByb3ZhbHMAAAAAAAPqAAAAEw==",
      "AAAABAAAAAAAAAAAAAAAEEJhdGNoU2VuZGVyRXJyb3IAAAARAAAAAAAAABJBbHJlYWR5SW5pdGlhbGl6ZWQAAAAABRQAAAAAAAAADk5vdEluaXRpYWxpemVkAAAAAAUVAAAAAAAAAApFbXB0eUJhdGNoAAAAAAUWAAAAAAAAAA1CYXRjaFRvb0xhcmdlAAAAAAAFFwAAAAAAAAARTm9uUG9zaXRpdmVBbW91bnQAAAAAAAUYAAAAAAAAABFFbXB0eUVwaGVtZXJhbEtleQAAAAAABRkAAAAAAAAABlBhdXNlZAAAAAAFGgAAAAAAAAAPQXNzZXROb3RBbGxvd2VkAAAABRsAAAAAAAAAFk11bHRpc2lnTm90SW5pdGlhbGl6ZWQAAAAABRwAAAAAAAAAGk11bHRpc2lnQWxyZWFkeUluaXRpYWxpemVkAAAAAAUdAAAAAAAAAAlOb3RTaWduZXIAAAAAAAUeAAAAAAAAABBJbnZhbGlkVGhyZXNob2xkAAAFHwAAAAAAAAAWUm90YXRpb25BbHJlYWR5UGVuZGluZwAAAAAFIAAAAAAAAAARTm9QZW5kaW5nUm90YXRpb24AAAAAAAUhAAAAAAAAABdBbHJlYWR5QXBwcm92ZWRSb3RhdGlvbgAAAAUiAAAAAAAAAAxRdW9ydW1Ob3RNZXQAAAUjAAAAAAAAABJUaW1lbG9ja05vdEVsYXBzZWQAAAAABSQ=",
      "AAAAAAAAAAAAAAAEaW5pdAAAAAMAAAAAAAAABWFkbWluAAAAAAAAEwAAAAAAAAAJYW5ub3VuY2VyAAAAAAAAEwAAAAAAAAAMYXNzZXRfcG9saWN5AAAD6AAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAABBCYXRjaFNlbmRlckVycm9y",
      "AAAAAAAAAAAAAAAFcGF1c2UAAAAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAABBCYXRjaFNlbmRlckVycm9y",
      "AAAAAAAAAAAAAAAHdW5wYXVzZQAAAAABAAAAAAAAAAZjYWxsZXIAAAAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAABBCYXRjaFNlbmRlckVycm9y",
      "AAAAAAAAAAAAAAAJaXNfcGF1c2VkAAAAAAAAAAAAAAEAAAAB",
      "AAAAAAAAAAAAAAAKYmF0Y2hfc2VuZAAAAAAAAwAAAAAAAAAEZnJvbQAAABMAAAAAAAAACXRyYW5zZmVycwAAAAAAA+oAAAfQAAAACFRyYW5zZmVyAAAAAAAAAAVhc3NldAAAAAAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAABBCYXRjaFNlbmRlckVycm9y",
      "AAAAAAAAAAAAAAAObWF4X2JhdGNoX3NpemUAAAAAAAAAAAABAAAABA==",
      "AAAAAAAAAAAAAAANaW5pdF9tdWx0aXNpZwAAAAAAAAIAAAAAAAAAB3NpZ25lcnMAAAAD6gAAABMAAAAAAAAACXRocmVzaG9sZAAAAAAAAAQAAAABAAAD6QAAA+0AAAAAAAAH0AAAABBCYXRjaFNlbmRlckVycm9y",
      "AAAAAAAAAAAAAAAHc2lnbmVycwAAAAAAAAAAAQAAA+oAAAAT",
      "AAAAAAAAAAAAAAAJdGhyZXNob2xkAAAAAAAAAAAAAAEAAAAE",
      "AAAAAAAAAAAAAAAQcGVuZGluZ19yb3RhdGlvbgAAAAAAAAABAAAD6AAAB9AAAAAQUm90YXRpb25Qcm9wb3NhbA==",
      "AAAAAAAAAAAAAAAWcHJvcG9zZV9yb3RhdGVfc2lnbmVycwAAAAAAAwAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAAtuZXdfc2lnbmVycwAAAAPqAAAAEwAAAAAAAAANbmV3X3RocmVzaG9sZAAAAAAAAAQAAAABAAAD6QAAA+0AAAAAAAAH0AAAABBCYXRjaFNlbmRlckVycm9y",
      "AAAAAAAAAAAAAAAWYXBwcm92ZV9yb3RhdGVfc2lnbmVycwAAAAAAAQAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAQQmF0Y2hTZW5kZXJFcnJvcg==",
      "AAAAAAAAAAAAAAAWZXhlY3V0ZV9yb3RhdGVfc2lnbmVycwAAAAAAAQAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAQQmF0Y2hTZW5kZXJFcnJvcg==",
      "AAAAAAAAAAAAAAAVY2FuY2VsX3JvdGF0ZV9zaWduZXJzAAAAAAAAAQAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAQQmF0Y2hTZW5kZXJFcnJvcg==",
    ]), options);
  }

  public readonly fromJSON = {
    init: this.txFromJSON<Result<void>>, pause: this.txFromJSON<Result<void>>, unpause: this.txFromJSON<Result<void>>,
    is_paused: this.txFromJSON<boolean>, batch_send: this.txFromJSON<Result<void>>, max_batch_size: this.txFromJSON<u32>,
    init_multisig: this.txFromJSON<Result<void>>, signers: this.txFromJSON<Array<string>>, threshold: this.txFromJSON<u32>,
    pending_rotation: this.txFromJSON<Option<RotationProposal>>, propose_rotate_signers: this.txFromJSON<Result<void>>,
    approve_rotate_signers: this.txFromJSON<Result<void>>, execute_rotate_signers: this.txFromJSON<Result<void>>,
    cancel_rotate_signers: this.txFromJSON<Result<void>>,
  };
}
