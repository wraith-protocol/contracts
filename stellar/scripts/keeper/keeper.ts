#!/usr/bin/env tsx
/**
 * Wraith Names TTL Keeper
 *
 * Permissionless keeper that finds registered names, reads their remaining TTL
 * from Soroban RPC and submits real `extend_name_ttl` transactions for names
 * below the threshold. Exits nonzero if names cannot be enumerated, a TTL cannot
 * be read, or a submission fails.
 *
 * Usage:
 *   tsx keeper.ts extend --network testnet --contract <id> --start-ledger <n> [--index-file names.json] [--dry-run]
 */

import { Command } from 'commander';
import { Keypair, Networks } from '@stellar/stellar-sdk';
import { KeeperError, loadIndexFile, runKeeper } from './keeper-core';
import { SorobanKeeperRpc } from './keeper-rpc';

const DEFAULT_THRESHOLD = 17_280; // ~1 day, matches TTL_THRESHOLD in the contract
const DEFAULT_EXTEND_TO = 518_400; // ~30 days, matches TTL_EXTEND_TO in the contract

const NETWORKS = {
  testnet: { passphrase: Networks.TESTNET, rpcUrl: 'https://soroban-testnet.stellar.org' },
  mainnet: { passphrase: Networks.PUBLIC, rpcUrl: 'https://mainnet.sorobanrpc.com' },
} as const;

async function main() {
  const program = new Command();
  program.name('wraith-names-keeper').description('TTL keeper for the Wraith Names contract');

  program
    .command('extend')
    .description('Extend the TTL of registered names that are close to expiry')
    .option('--network <network>', 'testnet or mainnet', 'testnet')
    .option('--contract <id>', 'Wraith Names contract ID', process.env.WRAITH_NAMES_CONTRACT_ID)
    .option('--start-ledger <n>', 'first ledger to scan events from (contract deployment ledger)')
    .option('--index-file <path>', 'JSON array of names from an off-chain index (merged with events)')
    .option('--threshold <ledgers>', 'extend when remaining TTL is below this', String(DEFAULT_THRESHOLD))
    .option('--extend-to <ledgers>', 'ledgers from now to extend to', String(DEFAULT_EXTEND_TO))
    .option('--secret-key <key>', 'keeper secret key (or WRAITH_KEEPER_SECRET)', process.env.WRAITH_KEEPER_SECRET)
    .option('--rpc-url <url>', 'Soroban RPC URL')
    .option('--dry-run', 'read TTLs and report, but submit nothing', false)
    .action(async (o) => {
      try {
        if (!o.contract) throw new KeeperError('config', '--contract or WRAITH_NAMES_CONTRACT_ID required');
        if (o.network !== 'testnet' && o.network !== 'mainnet') {
          throw new KeeperError('config', `unknown network ${o.network}`);
        }
        if (!o.dryRun && !o.secretKey) {
          throw new KeeperError('config', 'set WRAITH_KEEPER_SECRET or --secret-key (not needed with --dry-run)');
        }
        const net = NETWORKS[o.network as 'testnet' | 'mainnet'];
        const rpc = new SorobanKeeperRpc(
          o.rpcUrl ?? net.rpcUrl,
          o.contract,
          net.passphrase,
          o.secretKey ? Keypair.fromSecret(o.secretKey) : undefined,
        );
        const report = await runKeeper(
          rpc,
          {
            threshold: parseInt(o.threshold, 10),
            extendTo: parseInt(o.extendTo, 10),
            dryRun: o.dryRun,
            startLedger: o.startLedger !== undefined ? parseInt(o.startLedger, 10) : undefined,
            indexedNames: o.indexFile ? loadIndexFile(o.indexFile) : undefined,
          },
          (m) => console.log(m),
        );
        console.log(
          `done: ${report.total} names, ${report.atRisk} below threshold, ${report.extended.length} extended${report.dryRun ? ' (dry run)' : ''}`,
        );
      } catch (e) {
        const stage = e instanceof KeeperError ? e.stage : 'unexpected';
        console.error(`keeper failed [${stage}]: ${(e as Error).message}`);
        process.exit(1);
      }
    });

  program
    .command('cost-estimate')
    .description('Estimate cost of extending names')
    .option(
      '--network <network>',
      'Stellar network (testnet or mainnet)',
      'testnet',
    )
    .option(
      '--names-count <count>',
      'Number of names to estimate for',
      '1000',
    )
    .action((options) => {
      const namesCount = parseInt(options.namesCount, 10);
      const baseFeeXLM = 0.00001; // Base Stellar fee
      const sorobanResourceFeeXLM = 0.00001; // Estimated per-name resource fee

      const estimatedCostPerName = baseFeeXLM + sorobanResourceFeeXLM;
      const estimatedTotalCost = estimatedCostPerName * namesCount;
      const annualCost = estimatedTotalCost * 10; // ~10 extension cycles per year

      console.log(`\n💰 Cost Estimate for Wraith Names TTL Extensions`);
      console.log(`   Network: ${options.network}`);
      console.log(`   Names: ${namesCount}`);
      console.log(`   Cost per extension: ~${estimatedCostPerName} XLM`);
      console.log(`   Total for ${namesCount} names: ~${estimatedTotalCost} XLM`);
      console.log(`   Estimated annual cost: ~${annualCost} XLM per ${namesCount} names\n`);
    });

  await program.parseAsync(process.argv);
}

main().catch((error) => {
  console.error('Fatal error:', error);
  process.exit(1);
});
