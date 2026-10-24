import { Networks } from '@stellar/stellar-sdk';
import { describe, expect, it } from 'vitest';
import { Client as BatchSenderClient } from '../bindings/typescript/stealth-batch-sender/src/index.js';
import { Client as SplitterClient } from '../bindings/typescript/stealth-splitter/src/index.js';
import { Client as VaultClient } from '../bindings/typescript/stealth-vault/src/index.js';
import { Client as AssetPolicyClient } from '../bindings/typescript/wraith-asset-policy/src/index.js';
// Governance binding temporarily disabled - needs proper generation
// import { Client as GovernanceClient } from '../bindings/typescript/governance/src/index.js';

describe('stellar bindings portability check', () => {
  it('stealth-batch-sender client assembles a typed call', async () => {
    const client = new BatchSenderClient({ contractId: 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4', networkPassphrase: Networks.TESTNET, rpcUrl: 'http://localhost:8000', server: {} as never });
    const transaction = await client.max_batch_size({ simulate: false });
    expect(transaction.options.method).toBe('max_batch_size');
  });

  it('stealth-splitter client exports are available', async () => {
    const client = new SplitterClient({ contractId: 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4', networkPassphrase: Networks.TESTNET, rpcUrl: 'http://localhost:8000', server: {} as never });
    expect(client).toBeDefined();
  });

  it('stealth-vault client exports are available', async () => {
    const client = new VaultClient({ contractId: 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4', networkPassphrase: Networks.TESTNET, rpcUrl: 'http://localhost:8000', server: {} as never });
    expect(client).toBeDefined();
  });

  it('wraith-asset-policy client exports are available', async () => {
    const client = new AssetPolicyClient({ contractId: 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4', networkPassphrase: Networks.TESTNET, rpcUrl: 'http://localhost:8000', server: {} as never });
    expect(client).toBeDefined();
  });

  // Governance binding temporarily disabled - needs proper generation with stellar CLI
  // it('governance client exports are available', async () => {
  //   const client = new GovernanceClient({ contractId: 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4', networkPassphrase: Networks.TESTNET, rpcUrl: 'http://localhost:8000', server: {} as never });
  //   expect(client).toBeDefined();
  // });
});
