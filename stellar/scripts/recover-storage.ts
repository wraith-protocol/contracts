#!/usr/bin/env ts-node

import { Command } from "commander";
import {
  rpc,
  xdr,
  Address,
  Operation,
  TransactionBuilder,
  Keypair,
  SorobanDataBuilder,
  scValToNative,
  Account,
  Transaction
} from "@stellar/stellar-sdk";
import * as crypto from "crypto";

// Define network configurations
interface NetworkConfig {
  passphrase: string;
  rpcUrl: string;
  expertApiUrl: string;
}

const NETWORKS: Record<string, NetworkConfig> = {
  futurenet: {
    passphrase: "Test SDF Future Network ; October 2022",
    rpcUrl: "https://rpc-futurenet.stellar.org",
    expertApiUrl: "https://api.stellar.expert/explorer/futurenet"
  },
  testnet: {
    passphrase: "Test SDF Network ; September 2015",
    rpcUrl: "https://soroban-testnet.stellar.org",
    expertApiUrl: "https://api.stellar.expert/explorer/testnet"
  },
  mainnet: {
    passphrase: "Public Global Stellar Network ; October 2015",
    rpcUrl: "https://soroban-rpc.stellar.org",
    expertApiUrl: "https://api.stellar.expert/explorer/public"
  }
};

// Helper to get network configuration
function getNetworkConfig(network: string, customRpcUrl?: string): NetworkConfig {
  const config = NETWORKS[network.toLowerCase()];
  if (!config) {
    throw new Error(`Unsupported network: ${network}. Choose from: ${Object.keys(NETWORKS).join(", ")}`);
  }
  return {
    ...config,
    rpcUrl: customRpcUrl || config.rpcUrl
  };
}

// Convert Stroops to XLM
function stroopsToXlm(stroops: string | number | bigint): string {
  const s = BigInt(stroops);
  const xlmVal = Number(s) / 10_000_000;
  return `${xlmVal.toFixed(7)} XLM`;
}

// Helper to build contract data ledger key
function buildContractDataLedgerKey(
  contractId: string,
  keyScVal: xdr.ScVal,
  durability: xdr.ContractDataDurability
): xdr.LedgerKey {
  const contractAddress = Address.fromString(contractId).toScAddress();
  return xdr.LedgerKey.contractData(
    new xdr.LedgerKeyContractData({
      contract: contractAddress,
      key: keyScVal,
      durability
    })
  );
}

// Build DataKey::MetaAddress(Address, u32)
function buildMetaAddressKey(registrantStr: string, schemeId: number): xdr.ScVal {
  const registrantScAddress = Address.fromString(registrantStr).toScAddress();
  const registrantScVal = xdr.ScVal.scvAddress(registrantScAddress);
  const schemeIdScVal = xdr.ScVal.scvU32(schemeId);
  const symbolScVal = xdr.ScVal.scvSymbol("MetaAddress");
  
  return xdr.ScVal.scvVec([
    symbolScVal,
    registrantScVal,
    schemeIdScVal
  ]);
}

// Build DataKey::Name(BytesN<32>)
function buildNameKey(nameHashHex: string): xdr.ScVal {
  const nameHashBuffer = Buffer.from(nameHashHex, "hex");
  const bytesScVal = xdr.ScVal.scvBytes(nameHashBuffer);
  const symbolScVal = xdr.ScVal.scvSymbol("Name");
  
  return xdr.ScVal.scvVec([
    symbolScVal,
    bytesScVal
  ]);
}

// Build DataKey::Reverse(BytesN<32>)
function buildReverseKey(metaHashHex: string): xdr.ScVal {
  const metaHashBuffer = Buffer.from(metaHashHex, "hex");
  const bytesScVal = xdr.ScVal.scvBytes(metaHashBuffer);
  const symbolScVal = xdr.ScVal.scvSymbol("Reverse");
  
  return xdr.ScVal.scvVec([
    symbolScVal,
    bytesScVal
  ]);
}

// Fetch WASM hash from StellarExpert
async function fetchWasmHashFromStellarExpert(
  expertApiUrl: string,
  contractId: string
): Promise<Buffer | null> {
  const url = `${expertApiUrl}/contract/${contractId}`;
  try {
    const response = await fetch(url);
    if (!response.ok) {
      return null;
    }
    const data = await response.json() as any;
    if (data && data.wasm_hash) {
      return Buffer.from(data.wasm_hash, "hex");
    }
  } catch (e) {
    // ignore
  }
  return null;
}

// Query RPC getLedgerEntries to partition keys into live and archived
async function checkLedgerKeysStatus(
  rpcServer: rpc.Server,
  keys: xdr.LedgerKey[]
): Promise<{
  live: Map<string, { lastModifiedLedgerSeq: number; liveUntilLedgerSeq?: number }>;
  archived: xdr.LedgerKey[];
}> {
  const liveMap = new Map<string, { lastModifiedLedgerSeq: number; liveUntilLedgerSeq?: number }>();
  const archivedKeys: xdr.LedgerKey[] = [];

  const chunkSize = 100;
  for (let i = 0; i < keys.length; i += chunkSize) {
    const chunk = keys.slice(i, i + chunkSize);
    
    const response = await rpcServer.getLedgerEntries(...chunk);
    const liveKeysSet = new Set<string>();
    
    if (response.entries) {
      for (const entry of response.entries) {
        const entryKeyBase64 = entry.key.toXDR("base64");
        liveKeysSet.add(entryKeyBase64);
        let liveUntil: number | undefined;
        try {
          if ("liveUntilLedgerSeq" in entry) {
            liveUntil = (entry as any).liveUntilLedgerSeq;
          }
        } catch (e) {
          // ignore
        }
        liveMap.set(entryKeyBase64, {
          lastModifiedLedgerSeq: entry.lastModifiedLedgerSeq ?? 0,
          liveUntilLedgerSeq: liveUntil
        });
      }
    }

    for (const key of chunk) {
      const keyBase64 = key.toXDR("base64");
      if (!liveKeysSet.has(keyBase64)) {
        archivedKeys.push(key);
      }
    }
  }

  return { live: liveMap, archived: archivedKeys };
}

// Scan contract events to discover registration/name keys
async function scanContractEvents(
  rpcServer: rpc.Server,
  contractId: string,
  startLedger: number
): Promise<{
  registries: { registrant: string; schemeId: number }[];
  names: { hash: string; name: string; metaHash: string }[];
  releases: string[];
}> {
  const registries: { registrant: string; schemeId: number }[] = [];
  const names: { hash: string; name: string; metaHash: string }[] = [];
  const releases: string[] = [];

  const latestLedgerRes = await rpcServer.getLatestLedger();
  const endLedger = latestLedgerRes.sequence;

  if (startLedger > endLedger) {
    console.log(`Start ledger ${startLedger} is greater than latest ledger ${endLedger}. Skipping event scan.`);
    return { registries: [], names: [], releases: [] };
  }

  console.log(`Scanning contract events from ledger ${startLedger} to ${endLedger}...`);

  let currentLedger = startLedger;
  let cursor: string | undefined;
  let done = false;

  while (!done) {
    const params: any = {
      filters: [
        {
          type: "contract",
          contractIds: [contractId]
        }
      ],
      limit: 100
    };

    if (cursor) {
      params.cursor = cursor;
    } else {
      params.startLedger = currentLedger;
      params.endLedger = endLedger;
    }

    console.log("Querying events with params:", JSON.stringify(params));

    let response;
    try {
      response = await rpcServer.getEvents(params);
    } catch (e: any) {
      console.warn("Error querying events:", e.message || e);
      break;
    }

    if (!response.events || response.events.length === 0) {
      done = true;
      break;
    }

    for (const event of response.events) {
      if (event.topic && event.topic.length > 0) {
        try {
          const firstTopic = scValToNative(event.topic[0]);
          if (firstTopic === "register") {
            // stealth-registry: ["register", registrant Address, scheme_id u32]
            if (event.topic.length === 3) {
              const registrant = scValToNative(event.topic[1]);
              const schemeId = scValToNative(event.topic[2]);
              if (typeof registrant === "string" && typeof schemeId === "number") {
                registries.push({ registrant, schemeId });
              }
            }
            // wraith-names: ["register", name_hash BytesN<32>]
            // Value: (name String, stealth_meta_address Bytes)
            else if (event.topic.length === 2) {
              const nameHash = scValToNative(event.topic[1]);
              if (Buffer.isBuffer(nameHash)) {
                const val = scValToNative(event.value);
                if (Array.isArray(val) && val.length >= 2) {
                  const name = val[0];
                  const metaAddress = val[1];
                  if (typeof name === "string" && Buffer.isBuffer(metaAddress)) {
                    const metaHash = crypto.createHash("sha256").update(metaAddress).digest("hex");
                    names.push({
                      hash: nameHash.toString("hex"),
                      name,
                      metaHash
                    });
                  }
                }
              }
            }
          } else if (firstTopic === "release") {
            // Topics: ["release", name_hash BytesN<32>]
            if (event.topic.length === 2) {
              const nameHash = scValToNative(event.topic[1]);
              if (Buffer.isBuffer(nameHash)) {
                releases.push(nameHash.toString("hex"));
              }
            }
          }
        } catch (e) {
          // ignore parsing error for non-matching events
        }
      }
    }

    // Set cursor for the next iteration
    cursor = response.events[response.events.length - 1].id;
    if (!cursor) {
      done = true;
    }
  }

  return { registries, names, releases };
}

// Discover all potential keys for the contract
async function discoverContractKeys(
  rpcServer: rpc.Server,
  networkConfig: NetworkConfig,
  contractId: string,
  startLedger: number,
  userWasmHash?: string
): Promise<{
  keys: { description: string; key: xdr.LedgerKey }[];
  wasmHash: Buffer | null;
  instanceArchived: boolean;
}> {
  const discoveredKeys: { description: string; key: xdr.LedgerKey }[] = [];
  
  // 1. Contract Instance Key
  const instanceKey = xdr.LedgerKey.contractData(
    new xdr.LedgerKeyContractData({
      contract: Address.fromString(contractId).toScAddress(),
      key: xdr.ScVal.scvLedgerKeyContractInstance(),
      durability: xdr.ContractDataDurability.persistent()
    })
  );
  discoveredKeys.push({
    description: "Contract Instance",
    key: instanceKey
  });

  // Check instance status
  console.log("Checking contract instance status...");
  const instanceStatus = await checkLedgerKeysStatus(rpcServer, [instanceKey]);
  const instanceArchived = instanceStatus.archived.length > 0;

  let wasmHash: Buffer | null = null;
  if (userWasmHash) {
    wasmHash = Buffer.from(userWasmHash, "hex");
  }

  if (!instanceArchived) {
    console.log("Contract instance is live. Fetching WASM hash from ledger...");
    const response = await rpcServer.getLedgerEntries(instanceKey);
    if (response.entries && response.entries.length > 0) {
      try {
        const entryData = response.entries[0].val;
        const val = entryData.contractData().val();
        const instance = val.instance();
        const executable = instance.executable();
        if (executable.switch().value === xdr.ContractExecutableType.contractExecutableWasm().value) {
          wasmHash = executable.wasmHash();
        }
      } catch (e) {
        console.warn("Failed to extract WASM hash from contract instance:", e);
      }
    }
  } else {
    console.log("Contract instance is ARCHIVED.");
    if (!wasmHash) {
      console.log("Attempting to fetch WASM hash from StellarExpert API...");
      wasmHash = await fetchWasmHashFromStellarExpert(networkConfig.expertApiUrl, contractId);
      if (wasmHash) {
        console.log(`Successfully retrieved WASM hash from StellarExpert: ${wasmHash.toString("hex")}`);
      } else {
        console.warn("Could not retrieve WASM hash from StellarExpert. Wasm code recovery will be skipped unless `--wasm-hash` is provided.");
      }
    }
  }

  // 3. Contract Code Key
  if (wasmHash) {
    const codeKey = xdr.LedgerKey.contractCode(
      new xdr.LedgerKeyContractCode({
        hash: wasmHash
      })
    );
    discoveredKeys.push({
      description: `Contract Wasm Code (${wasmHash.toString("hex").substring(0, 8)}...)`,
      key: codeKey
    });
  }

  // 4. Scan events for storage keys
  try {
    const { registries, names, releases } = await scanContractEvents(rpcServer, contractId, startLedger);
    
    // Process Stealth Registry keys
    if (registries.length > 0) {
      console.log(`Discovered ${registries.length} potential Stealth Registry registration entries.`);
      const uniqueRegs = new Map<string, { registrant: string; schemeId: number }>();
      for (const reg of registries) {
        uniqueRegs.set(`${reg.registrant}-${reg.schemeId}`, reg);
      }

      for (const reg of uniqueRegs.values()) {
        const keyVal = buildMetaAddressKey(reg.registrant, reg.schemeId);
        const lKey = buildContractDataLedgerKey(contractId, keyVal, xdr.ContractDataDurability.persistent());
        discoveredKeys.push({
          description: `Stealth Registry MetaAddress: registrant=${reg.registrant.substring(0, 8)}..., scheme=${reg.schemeId}`,
          key: lKey
        });
      }
    }

    // Process Wraith Names keys
    if (names.length > 0) {
      console.log(`Discovered ${names.length} potential Wraith Name registration entries.`);
      const releasedSet = new Set(releases);
      const activeNames = new Map<string, { hash: string; name: string; metaHash: string }>();
      for (const name of names) {
        if (!releasedSet.has(name.hash)) {
          activeNames.set(name.hash, name);
        }
      }

      for (const nameEntry of activeNames.values()) {
        // Name key
        const nameKeyVal = buildNameKey(nameEntry.hash);
        const nameLedgerKey = buildContractDataLedgerKey(contractId, nameKeyVal, xdr.ContractDataDurability.persistent());
        discoveredKeys.push({
          description: `Wraith Name Entry: "${nameEntry.name}"`,
          key: nameLedgerKey
        });

        // Reverse key
        const reverseKeyVal = buildReverseKey(nameEntry.metaHash);
        const reverseLedgerKey = buildContractDataLedgerKey(contractId, reverseKeyVal, xdr.ContractDataDurability.persistent());
        discoveredKeys.push({
          description: `Wraith Name Reverse Entry: metaHash=${nameEntry.metaHash.substring(0, 8)}...`,
          key: reverseLedgerKey
        });
      }
    }
  } catch (e) {
    console.warn("Failed to scan contract events. Storage keys discovery skipped:", e);
  }

  return { keys: discoveredKeys, wasmHash, instanceArchived };
}

// Subcommands implementations
async function handleListArchived(contractId: string, options: any) {
  const config = getNetworkConfig(options.network, options.rpcUrl);
  const rpcServer = new rpc.Server(config.rpcUrl);
  const startLedger = parseInt(options.startLedger);

  console.log(`Contract ID: ${contractId}`);
  console.log(`Network: ${options.network}`);
  console.log(`RPC URL: ${config.rpcUrl}`);
  console.log(`Start Ledger: ${startLedger}`);

  const { keys } = await discoverContractKeys(rpcServer, config, contractId, startLedger, options.wasmHash);

  console.log(`\nVerifying status of ${keys.length} discovered keys...`);
  const status = await checkLedgerKeysStatus(rpcServer, keys.map((k) => k.key));

  console.log("\n=================== LIVE ENTRIES ===================");
  if (status.live.size === 0) {
    console.log("No live entries found.");
  } else {
    for (const item of keys) {
      const keyBase64 = item.key.toXDR("base64");
      const liveInfo = status.live.get(keyBase64);
      if (liveInfo) {
        console.log(`[LIVE] ${item.description}`);
        console.log(`   Last Modified: Ledger ${liveInfo.lastModifiedLedgerSeq}`);
        if (liveInfo.liveUntilLedgerSeq) {
          console.log(`   Live Until: Ledger ${liveInfo.liveUntilLedgerSeq}`);
        }
      }
    }
  }

  console.log("\n================= ARCHIVED ENTRIES =================");
  if (status.archived.length === 0) {
    console.log("No archived entries found.");
  } else {
    const archivedBase64 = new Set(status.archived.map((k) => k.toXDR("base64")));
    for (const item of keys) {
      const keyBase64 = item.key.toXDR("base64");
      if (archivedBase64.has(keyBase64)) {
        console.log(`[ARCHIVED] ${item.description}`);
      }
    }
  }
}

async function handleEstimateRestore(contractId: string, options: any) {
  const config = getNetworkConfig(options.network, options.rpcUrl);
  const rpcServer = new rpc.Server(config.rpcUrl);
  const startLedger = parseInt(options.startLedger);

  const { keys } = await discoverContractKeys(rpcServer, config, contractId, startLedger, options.wasmHash);
  const status = await checkLedgerKeysStatus(rpcServer, keys.map((k) => k.key));

  if (status.archived.length === 0) {
    console.log("\nAll contract storage entries are already live. No-op.");
    return;
  }

  console.log(`\nFound ${status.archived.length} archived entries to restore.`);
  
  // Use a dummy source account for simulation
  const dummyPublicKey = "GDYH62HW5R57ZFCJE77Q32YVUANQPK2A4663BWFVKAIMINNWVV6QEI5P";
  const sourceAccount = new Account(dummyPublicKey, "0");

  const sorobanData = new SorobanDataBuilder()
    .setReadWrite(status.archived)
    .setReadOnly([])
    .build();

  const tx = new TransactionBuilder(sourceAccount, {
    fee: "100000",
    networkPassphrase: config.passphrase
  })
    .setSorobanData(sorobanData)
    .addOperation(Operation.restoreFootprint({}))
    .build();

  console.log("Simulating restoration transaction...");
  const preparedTx = await rpcServer.prepareTransaction(tx) as any;
  
  const resourceFee = preparedTx.sorobanData.resources().fee().toString();
  const inclusionFee = (BigInt(preparedTx.fee) - BigInt(resourceFee)).toString();
  const totalFee = preparedTx.fee;

  console.log("\n================ ESTIMATED RESTORE COST ================");
  console.log(`Network Base Fee (Inclusion): ${inclusionFee} stroops (${stroopsToXlm(inclusionFee)})`);
  console.log(`Soroban Resource Fee:         ${resourceFee} stroops (${stroopsToXlm(resourceFee)})`);
  console.log(`--------------------------------------------------------`);
  console.log(`Total Estimated Transaction Cost: ${totalFee} stroops (${stroopsToXlm(totalFee)})`);
  console.log("========================================================");
}

async function handleRestore(contractId: string, options: any) {
  if (!options.secretKey) {
    console.error("Error: --secret-key is required to sign the restore transaction.");
    process.exit(1);
  }

  const config = getNetworkConfig(options.network, options.rpcUrl);
  const rpcServer = new rpc.Server(config.rpcUrl);
  const startLedger = parseInt(options.startLedger);

  const { keys } = await discoverContractKeys(rpcServer, config, contractId, startLedger, options.wasmHash);
  const status = await checkLedgerKeysStatus(rpcServer, keys.map((k) => k.key));

  if (status.archived.length === 0) {
    console.log("\nAll contract storage entries are already live. No-op.");
    return;
  }

  console.log(`\nFound ${status.archived.length} archived entries to restore.`);

  const keypair = Keypair.fromSecret(options.secretKey);
  const sourcePublicKey = keypair.publicKey();
  console.log(`Fee payer account: ${sourcePublicKey}`);

  console.log("Fetching account sequence number...");
  const sourceAccount = await rpcServer.getAccount(sourcePublicKey);

  const sorobanData = new SorobanDataBuilder()
    .setReadWrite(status.archived)
    .setReadOnly([])
    .build();

  const tx = new TransactionBuilder(sourceAccount, {
    fee: "100000",
    networkPassphrase: config.passphrase
  })
    .setSorobanData(sorobanData)
    .addOperation(Operation.restoreFootprint({}))
    .build();

  console.log("Simulating restoration transaction to finalize resources...");
  const preparedTx = await rpcServer.prepareTransaction(tx) as any;
  
  const resourceFee = preparedTx.sorobanData.resources().fee().toString();
  const inclusionFee = (BigInt(preparedTx.fee) - BigInt(resourceFee)).toString();
  const totalFee = preparedTx.fee;

  console.log("\n================ FINAL RESTORE COST ================");
  console.log(`Network Base Fee (Inclusion): ${inclusionFee} stroops (${stroopsToXlm(inclusionFee)})`);
  console.log(`Soroban Resource Fee:         ${resourceFee} stroops (${stroopsToXlm(resourceFee)})`);
  console.log(`--------------------------------------------------------`);
  console.log(`Total Transaction Cost:       ${totalFee} stroops (${stroopsToXlm(totalFee)})`);
  console.log("====================================================");

  // Sign transaction
  console.log("\nSigning transaction...");
  preparedTx.sign(keypair);

  console.log("Submitting transaction to the network...");
  const sendRes = await rpcServer.sendTransaction(preparedTx);

  if (sendRes.status === "ERROR") {
    console.error(`Transaction submission failed: ${sendRes.errorResult ? sendRes.errorResult.toXDR("base64") : "Unknown Error"}`);
    process.exit(1);
  }

  console.log(`Transaction sent. Hash: ${sendRes.hash}`);
  console.log("Waiting for transaction confirmation...");

  let confirmRes = await rpcServer.getTransaction(sendRes.hash);
  while (confirmRes.status === rpc.Api.GetTransactionStatus.NOT_FOUND) {
    await new Promise((resolve) => setTimeout(resolve, 1000));
    confirmRes = await rpcServer.getTransaction(sendRes.hash);
  }

  if (confirmRes.status === rpc.Api.GetTransactionStatus.SUCCESS) {
    console.log("\nSUCCESS! Restoration complete. All archived storage entries have been restored to live state.");
  } else {
    console.error(`\nFAILED! Restoration transaction failed. Status: ${confirmRes.status}`);
    if (confirmRes.status === rpc.Api.GetTransactionStatus.FAILED) {
      console.error(`Result XDR: ${confirmRes.resultXdr.toXDR("base64")}`);
    }
    process.exit(1);
  }
}

// CLI Initialization
const program = new Command();

program
  .name("recover-storage")
  .description("Soroban storage entry recovery tooling for Wraith Protocol smart contracts")
  .version("0.1.0");

// Common options
const addCommonOptions = (cmd: Command) => {
  cmd
    .requiredOption("-c, --contract-id <id>", "Contract ID to check and restore")
    .option("-n, --network <network>", "Stellar network: futurenet, testnet, mainnet", "futurenet")
    .option("-r, --rpc-url <url>", "Custom Soroban RPC URL override")
    .option("-w, --wasm-hash <hash>", "Hex-encoded custom Wasm code hash to verify code entry status")
    .option("-s, --start-ledger <ledger>", "Start ledger sequence for scanning creation/registration events", "1");
};

// list-archived subcommand
const listCmd = program.command("list-archived");
listCmd.description("Surface live and archived contract data entries");
addCommonOptions(listCmd);
listCmd.action((options) => {
  handleListArchived(options.contractId, options).catch((err) => {
    console.error("Execution failed:", err);
    process.exit(1);
  });
});

// estimate-restore subcommand
const estimateCmd = program.command("estimate-restore");
estimateCmd.description("Pre-compute XLM fee estimation for restoring archived entries");
addCommonOptions(estimateCmd);
estimateCmd.action((options) => {
  handleEstimateRestore(options.contractId, options).catch((err) => {
    console.error("Execution failed:", err);
    process.exit(1);
  });
});

// restore subcommand
const restoreCmd = program.command("restore");
restoreCmd.description("Restore archived contract data entries idempotently");
addCommonOptions(restoreCmd);
restoreCmd.requiredOption("-k, --secret-key <key>", "Secret key of the account paying restoration fees");
restoreCmd.action((options) => {
  handleRestore(options.contractId, options).catch((err) => {
    console.error("Execution failed:", err);
    process.exit(1);
  });
});

program.parse(process.argv);
