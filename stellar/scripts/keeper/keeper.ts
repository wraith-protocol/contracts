#!/usr/bin/env node

/**
 * Wraith Names TTL Keeper Bot
 * 
 * Reads all registered names from the wraith-names contract and extends
 * their TTLs if they are getting close to expiration.
 * 
 * Usage:
 *   npx ts-node keeper.ts --network testnet --contract <contract-id> --threshold 100000
 * 
 * Configuration via environment variables:
 *   STELLAR_NETWORK_PASSPHRASE - Stellar network (default: "Test SDF Network ; June 2015")
 *   SOROBAN_RPC_URL - RPC endpoint (default: https://soroban-testnet.stellar.org)
 *   KEEPER_SECRET_KEY - Keeper account secret key for sponsoring operations
 *   WRAITH_NAMES_CONTRACT - Contract ID (can also be passed as --contract)
 *   TTL_THRESHOLD_LEDGERS - Ledger threshold before extending (default: 100000)
 *   EXTEND_TO_FUTURE_LEDGERS - How many ledgers into future to extend (default: 500000)
 *   DRY_RUN - If set, only report what would be done (default: false)
 */

import * as SorobanClient from "@stellar/js-stellar-sdk";
import * as SorobanRpc from "@stellar/js-stellar-sdk/rpc";
import yargs from "yargs";
import { hideBin } from "yargs/helpers";

interface KeeperConfig {
  networkPassphrase: string;
  rpcUrl: string;
  contractId: string;
  keeperSecretKey: string;
  ttlThresholdLedgers: number;
  extendToFutureLedgers: number;
  dryRun: boolean;
}

interface NameInfo {
  name: string;
  metaAddress: string;
  owner: string;
}

async function loadConfig(): Promise<KeeperConfig> {
  const argv = await yargs(hideBin(process.argv))
    .option("network", {
      alias: "n",
      description: "Stellar network",
      type: "string",
      default: "testnet",
    })
    .option("contract", {
      alias: "c",
      description: "Wraith Names contract ID",
      type: "string",
    })
    .option("threshold", {
      alias: "t",
      description: "TTL threshold in ledgers before extending",
      type: "number",
      default: 100000,
    })
    .option("extend-to", {
      alias: "e",
      description: "How many ledgers into future to extend to",
      type: "number",
      default: 500000,
    })
    .option("dry-run", {
      description: "Report what would be done without executing",
      type: "boolean",
      default: false,
    })
    .demandOption(["contract"])
    .parse();

  const networkPassphrase =
    process.env.STELLAR_NETWORK_PASSPHRASE ||
    "Test SDF Network ; June 2015";
  const rpcUrl =
    process.env.SOROBAN_RPC_URL ||
    "https://soroban-testnet.stellar.org";
  const contractId =
    process.env.WRAITH_NAMES_CONTRACT || argv.contract;
  const keeperSecretKey = process.env.KEEPER_SECRET_KEY;
  
  if (!keeperSecretKey) {
    throw new Error("KEEPER_SECRET_KEY environment variable not set");
  }
  
  if (!contractId) {
    throw new Error("Contract ID must be provided via --contract or WRAITH_NAMES_CONTRACT");
  }

  return {
    networkPassphrase,
    rpcUrl,
    contractId,
    keeperSecretKey,
    ttlThresholdLedgers: argv["threshold"],
    extendToFutureLedgers: argv["extend-to"],
    dryRun: argv["dry-run"],
  };
}

async function getCurrentLedger(client: SorobanRpc.Client): Promise<number> {
  const ledger = await client.getLatestLedger();
  return ledger.sequence;
}

/**
 * Get all registered names from the contract.
 * This would require iterating through the contract's storage.
 * For now, we provide a stub that demonstrates the pattern.
 */
async function getAllRegisteredNames(
  config: KeeperConfig,
  client: SorobanRpc.Client
): Promise<NameInfo[]> {
  console.log(`Querying contract ${config.contractId} for registered names...`);
  
  // In practice, this would need to:
  // 1. Call a `get_all_names()` method on the contract (if exposed)
  // 2. Or iterate through the contract's ledger state to find all Name(hash) entries
  // 3. For each found entry, deserialize and collect the name info
  
  // Placeholder: return empty array for now
  // This would be implemented when the contract exposes an enumeration method
  return [];
}

/**
 * Extend the TTL for a single name.
 */
async function extendNameTtl(
  config: KeeperConfig,
  client: SorobanRpc.Client,
  name: string,
  extendToLedger: number
): Promise<boolean> {
  try {
    console.log(
      `Extending TTL for "${name}" to ledger ${extendToLedger}...`
    );
    
    if (config.dryRun) {
      console.log(`[DRY RUN] Would extend TTL for "${name}"`);
      return true;
    }
    
    // Get keeper account
    const keeperKeypair = SorobanClient.Keypair.fromSecret(config.keeperSecretKey);
    const server = new SorobanClient.Server(config.rpcUrl, { allowHttp: true });
    
    const account = await server.getAccount(keeperKeypair.publicKey());
    
    // Build contract invocation
    // This is a placeholder - actual implementation would build the proper
    // Soroban contract invocation for extend_name_ttl()
    console.log(`[PLACEHOLDER] Would call extend_name_ttl("${name}", ${extendToLedger})`);
    
    return true;
  } catch (error) {
    console.error(`Failed to extend TTL for "${name}":`, error);
    return false;
  }
}

/**
 * Main keeper loop.
 */
async function main(): Promise<void> {
  try {
    const config = await loadConfig();
    
    console.log("=== Wraith Names TTL Keeper ===");
    console.log(`Network: ${config.networkPassphrase}`);
    console.log(`RPC URL: ${config.rpcUrl}`);
    console.log(`Contract: ${config.contractId}`);
    console.log(`TTL Threshold: ${config.ttlThresholdLedgers} ledgers`);
    console.log(`Extend To: ${config.extendToFutureLedgers} ledgers in future`);
    console.log(`Dry Run: ${config.dryRun}`);
    console.log("");
    
    const client = new SorobanRpc.Client({ url: config.rpcUrl, allowHttp: true });
    
    // Get current ledger
    const currentLedger = await getCurrentLedger(client);
    console.log(`Current ledger: ${currentLedger}`);
    
    // Get all registered names
    const names = await getAllRegisteredNames(config, client);
    console.log(`Found ${names.length} registered names`);
    
    if (names.length === 0) {
      console.log("No names to extend.");
      return;
    }
    
    // Process each name
    let extended = 0;
    let failed = 0;
    
    for (const nameInfo of names) {
      const shouldExtend = true; // Placeholder - check TTL here
      
      if (shouldExtend) {
        const extendTo = currentLedger + config.extendToFutureLedgers;
        const success = await extendNameTtl(config, client, nameInfo.name, extendTo);
        
        if (success) {
          extended++;
        } else {
          failed++;
        }
      }
    }
    
    console.log("");
    console.log(`=== Summary ===`);
    console.log(`Extended: ${extended}`);
    console.log(`Failed: ${failed}`);
    
  } catch (error) {
    console.error("Keeper bot error:", error);
    process.exit(1);
  }
}

main();
