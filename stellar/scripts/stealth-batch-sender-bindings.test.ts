import { Networks } from '@stellar/stellar-sdk';
import { describe, expect, it } from 'vitest';
import { Client } from '../bindings/typescript/stealth-batch-sender/src/index.js';

describe('stealth-batch-sender generated client', () => {
  it('assembles a typed max_batch_size call', async () => {
    const client = new Client({ contractId: 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4', networkPassphrase: Networks.TESTNET, rpcUrl: 'http://localhost:8000', server: {} as never });
    const transaction = await client.max_batch_size({ simulate: false });

    expect(transaction.options.method).toBe('max_batch_size');
  });
});
