/**
 * Soroban RPC implementation of KeeperRpc.
 *
 * Name enumeration comes from the contract's own `register`, `update` and `release`
 * events (the contract keeps no on-chain name list). Remaining TTL is read from the
 * persistent `Name(sha256(name))` ledger entry via getLedgerEntries.
 */

import {
  Account,
  Address,
  BASE_FEE,
  Contract,
  Keypair,
  TransactionBuilder,
  nativeToScVal,
  rpc,
  scValToNative,
  xdr,
} from '@stellar/stellar-sdk';
import { DecodedEvent, EventPage, KeeperRpc, nameHash } from './keeper-core';

const EVENT_PAGE_LIMIT = 200;

export class SorobanKeeperRpc implements KeeperRpc {
  private server: rpc.Server;

  constructor(
    private rpcUrl: string,
    private contractId: string,
    private passphrase: string,
    private keypair?: Keypair,
  ) {
    this.server = new rpc.Server(rpcUrl, { allowHttp: rpcUrl.startsWith('http://') });
  }

  async getLatestLedger(): Promise<number> {
    return (await this.server.getLatestLedger()).sequence;
  }

  /** Oldest ledger the RPC retains, via raw getHealth (not exposed by SDK v13 types). */
  private async oldestLedger(fallback: number): Promise<number> {
    const res = await fetch(this.rpcUrl, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'getHealth' }),
    });
    const body = (await res.json()) as { result?: { oldestLedger?: number } };
    return body.result?.oldestLedger ?? fallback;
  }

  async getNameEvents(startLedger: number, cursor?: string): Promise<EventPage> {
    const filters = [{ type: 'contract' as const, contractIds: [this.contractId] }];
    const resp = await this.server.getEvents(
      cursor
        ? { filters, cursor, limit: EVENT_PAGE_LIMIT }
        : { filters, startLedger, limit: EVENT_PAGE_LIMIT },
    );
    const events: DecodedEvent[] = [];
    for (const ev of resp.events) {
      const kind = scValToNative(ev.topic[0]);
      if (kind !== 'register' && kind !== 'update' && kind !== 'release') continue;
      const value = scValToNative(ev.value);
      // register/update: (name, meta_address); release: name
      const name = Array.isArray(value) ? value[0] : value;
      if (typeof name !== 'string') {
        throw new Error(`unexpected ${kind} event payload at ledger ${ev.ledger}`);
      }
      events.push({ ledger: ev.ledger, kind, name });
    }
    return {
      events,
      cursor: resp.events.length >= EVENT_PAGE_LIMIT ? resp.cursor : undefined,
      oldestLedger: await this.oldestLedger(startLedger),
      latestLedger: resp.latestLedger,
    };
  }

  private nameKey(hashHex: string): xdr.LedgerKey {
    return xdr.LedgerKey.contractData(
      new xdr.LedgerKeyContractData({
        contract: new Address(this.contractId).toScAddress(),
        key: xdr.ScVal.scvVec([
          xdr.ScVal.scvSymbol('Name'),
          xdr.ScVal.scvBytes(Buffer.from(hashHex, 'hex')),
        ]),
        durability: xdr.ContractDataDurability.persistent(),
      }),
    );
  }

  async getNameLiveUntil(hashHex: string): Promise<number | null> {
    const res = await this.server.getLedgerEntries(this.nameKey(hashHex));
    const entry = res.entries?.[0];
    if (!entry || entry.liveUntilLedgerSeq === undefined) return null;
    return entry.liveUntilLedgerSeq;
  }

  async extendName(name: string, extendTo: number): Promise<string> {
    if (!this.keypair) throw new Error('no keeper secret key configured');
    const account = await this.server.getAccount(this.keypair.publicKey());
    const op = new Contract(this.contractId).call(
      'extend_name_ttl',
      nativeToScVal(name, { type: 'string' }),
      nativeToScVal(extendTo, { type: 'u32' }),
    );
    const tx = new TransactionBuilder(new Account(account.accountId(), account.sequenceNumber()), {
      fee: BASE_FEE,
      networkPassphrase: this.passphrase,
    })
      .addOperation(op)
      .setTimeout(60)
      .build();

    const prepared = await this.server.prepareTransaction(tx);
    prepared.sign(this.keypair);
    const sent = await this.server.sendTransaction(prepared);
    if (sent.status === 'ERROR' || sent.status === 'TRY_AGAIN_LATER') {
      throw new Error(`sendTransaction returned ${sent.status}`);
    }
    const final = await this.server.pollTransaction(sent.hash, { attempts: 30 });
    if (final.status !== rpc.Api.GetTransactionStatus.SUCCESS) {
      throw new Error(`transaction ${sent.hash} ended as ${final.status}`);
    }
    return sent.hash;
  }
}

export { nameHash };
