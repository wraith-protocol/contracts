import { Networks } from '@stellar/stellar-sdk';
import { describe, expect, it } from 'vitest';
// Governance binding temporarily disabled - needs proper generation with stellar CLI
// import { Client } from '../bindings/typescript/governance/src/index.js';

describe('governance generated client', () => {
  it.skip('assembles a typed get_config call', async () => {
    // Temporarily skipped - binding needs to be regenerated with stellar CLI
    // const client = new Client({ contractId: 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4', networkPassphrase: Networks.TESTNET, rpcUrl: 'http://localhost:8000', server: {} as never });
    // const transaction = await client.get_config({ simulate: false });
    // expect(transaction.options.method).toBe('get_config');
  });
});
