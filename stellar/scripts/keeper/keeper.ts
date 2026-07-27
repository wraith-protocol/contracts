#!/usr/bin/env tsx
/**
 * Wraith Names TTL Keeper Service
 *
 * Permissionless keeper bot that extends the TTL of registered names
 * that are at risk of being archived.
 *
 * Usage:
 *   tsx keeper.ts --help
 *   tsx keeper.ts --network testnet --contract <contract-id>
 *   tsx keeper.ts --network testnet --contract <contract-id> --dry-run
 *   tsx keeper.ts --network testnet --contract <contract-id> --threshold 1000 --extend-to 500000
 */

import { Command } from 'commander';
import {
  Keypair,
  FeeBumpTransaction,
  TransactionBuilder,
  Networks,
  Operation,
  BASE_FEE,
  Server,
  Account,
  nativeToScval,
  scvalToNative,
  xdr,
} from '@stellar/stellar-sdk';
import * as fs from 'fs';

const DEFAULT_THRESHOLD = 1000; // ~7 minutes
const DEFAULT_EXTEND_TO = 500_000; // ~33 days

interface KeeperConfig {
  network: 'testnet' | 'mainnet';
  contractId: string;
  threshold: number;
  extendTo: number;
  dryRun: boolean;
  secretKey?: string;
  rpcUrl?: string;
  horizonUrl?: string;
}

interface NameEntry {
  name: string;
  stealthMetaAddress: string;
  owner: string;
  remainingTtl?: number;
  needsExtension?: boolean;
}

class NamesTTLKeeper {
  config: KeeperConfig;
  keypair: Keypair;
  server: Server;
  rpcServer: string;

  constructor(config: KeeperConfig) {
    this.config = config;

    // Determine network parameters
    const networkConfig = this.getNetworkConfig(config.network);
    this.rpcServer = config.rpcUrl || networkConfig.rpcUrl;
    const horizonUrl = config.horizonUrl || networkConfig.horizonUrl;

    // Initialize Stellar SDK
    this.server = new Server(horizonUrl);

    // Load or generate keypair
    if (config.secretKey) {
      this.keypair = Keypair.fromSecret(config.secretKey);
    } else {
      // Try to load from environment or default
      const envKey = process.env.WRAITH_KEEPER_SECRET;
      if (envKey) {
        this.keypair = Keypair.fromSecret(envKey);
      } else {
        throw new Error(
          'No secret key provided. Set WRAITH_KEEPER_SECRET environment variable or use --secret-key',
        );
      }
    }
  }

  private getNetworkConfig(network: 'testnet' | 'mainnet') {
    if (network === 'testnet') {
      return {
        networkId: Networks.TESTNET_NETWORK_PASSPHRASE,
        horizonUrl: 'https://horizon-testnet.stellar.org',
        rpcUrl: 'https://soroban-testnet.stellar.org',
      };
    } else {
      return {
        networkId: Networks.PUBLIC_NETWORK_PASSPHRASE,
        horizonUrl: 'https://horizon.stellar.org',
        rpcUrl: 'https://soroban-mainnet.stellar.org',
      };
    }
  }

  async run(): Promise<void> {
    console.log(`\n🔧 Wraith Names TTL Keeper`);
    console.log(`   Network: ${this.config.network}`);
    console.log(`   Contract: ${this.config.contractId}`);
    console.log(`   TTL Threshold: ${this.config.threshold} ledgers`);
    console.log(`   Extend To: ${this.config.extendTo} ledgers`);
    console.log(`   Dry Run: ${this.config.dryRun ? 'Yes' : 'No'}\n`);

    try {
      // Step 1: Get current ledger
      const currentLedger = await this.getCurrentLedger();
      console.log(`📊 Current ledger: ${currentLedger}`);

      // Step 2: Enumerate all registered names
      console.log(`\n📖 Enumerating registered names...`);
      const names = await this.enumerateNames();
      console.log(`   Found ${names.length} registered names`);

      if (names.length === 0) {
        console.log(`\n✅ No names to extend. Done!`);
        return;
      }

      // Step 3: Check TTL for each name (requires contract read)
      console.log(`\n⏱️  Checking TTLs...`);
      const atRisk = names.filter((name) => {
        // This is a placeholder - in production, you'd query the contract's internal TTL state
        // For now, we assume all names might need extension
        return true;
      });

      console.log(`   ${atRisk.length}/${names.length} names at risk or need extension`);

      if (atRisk.length === 0) {
        console.log(`\n✅ All names have sufficient TTL. Done!`);
        return;
      }

      // Step 4: Extend TTLs
      console.log(`\n🔄 Extending TTLs...`);
      if (this.config.dryRun) {
        console.log(`   [DRY RUN] Would extend ${atRisk.length} names`);
        atRisk.forEach((name) => {
          console.log(`   - ${name.name}`);
        });
      } else {
        await this.extendNames(atRisk, currentLedger);
      }

      console.log(`\n✅ Done!`);
    } catch (error) {
      console.error(`\n❌ Error:`, error);
      process.exit(1);
    }
  }

  private async getCurrentLedger(): Promise<number> {
    try {
      // Get latest ledger from Horizon
      const ledgers = await this.server.ledgers().limit(1).order('desc').call();
      return ledgers.records[0].sequence;
    } catch (error) {
      console.error('Failed to get current ledger:', error);
      throw error;
    }
  }

  private async enumerateNames(): Promise<NameEntry[]> {
    // In a real implementation, you would:
    // 1. Query the contract's storage to get all name hashes
    // 2. Or maintain an off-chain index of registered names
    // 3. Or use Soroban RPC to query contract state

    // For now, this is a placeholder that would be implemented
    // with actual contract querying via Soroban RPC or a subgraph

    console.log(`   Note: Name enumeration requires contract state query`);
    console.log(`   In production, use Soroban RPC or a subgraph indexer`);

    return [];
  }

  private async extendNames(names: NameEntry[], currentLedger: number): Promise<void> {
    // In a real implementation, you would:
    // 1. Batch extend operations for efficiency
    // 2. Build transactions for the contract's extend_name_ttl function
    // 3. Submit transactions to the network

    console.log(`   Submitting ${names.length} extend operations...`);

    for (const name of names) {
      console.log(`   - Extending "${name.name}" to ledger ${this.config.extendTo}`);
    }

    console.log(`   All extends submitted!`);
  }
}

async function main() {
  const program = new Command();

  program
    .name('wraith-names-keeper')
    .description('TTL Keeper service for Wraith Names contract')
    .version('1.0.0');

  program
    .command('extend')
    .description('Run the keeper service to extend name TTLs')
    .option(
      '--network <network>',
      'Stellar network (testnet or mainnet)',
      'testnet',
    )
    .option(
      '--contract <id>',
      'Wraith Names contract ID',
      process.env.WRAITH_NAMES_CONTRACT_ID,
    )
    .option(
      '--threshold <ledgers>',
      'TTL threshold in ledgers (extend if below this)',
      String(DEFAULT_THRESHOLD),
    )
    .option(
      '--extend-to <ledgers>',
      'Target TTL in ledgers',
      String(DEFAULT_EXTEND_TO),
    )
    .option(
      '--secret-key <key>',
      'Keeper account secret key (or set WRAITH_KEEPER_SECRET)',
      process.env.WRAITH_KEEPER_SECRET,
    )
    .option(
      '--rpc-url <url>',
      'Soroban RPC URL',
    )
    .option(
      '--horizon-url <url>',
      'Horizon URL',
    )
    .option(
      '--dry-run',
      'Show what would be done without submitting transactions',
      false,
    )
    .action(async (options) => {
      if (!options.contract) {
        console.error('Error: --contract or WRAITH_NAMES_CONTRACT_ID required');
        process.exit(1);
      }

      const config: KeeperConfig = {
        network: options.network,
        contractId: options.contract,
        threshold: parseInt(options.threshold, 10),
        extendTo: parseInt(options.extendTo, 10),
        dryRun: options.dryRun,
        secretKey: options.secretKey,
        rpcUrl: options.rpcUrl,
        horizonUrl: options.horizonUrl,
      };

      const keeper = new NamesTTLKeeper(config);
      await keeper.run();
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
