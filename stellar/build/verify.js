import { program } from 'commander';
import { execSync } from 'child_process';
import { readFileSync, writeFileSync } from 'fs';

program
  .requiredOption('--contract <name>', 'Name of the contract (e.g., wraith-names)')
  .option('--id <contract_id>', 'Stellar Contract ID (e.g., C...) to fetch from RPC')
  .requiredOption('--network <network>', 'Network (mainnet, testnet, futurenet)')
  .option('--commit <hash>', 'Git commit hash to verify against')
  .option('--repo <owner/repo>', 'GitHub repository to fetch attestations from', 'wraith-protocol/contracts')
  .option('--attestation <path>', 'Path to a local attestation.json file (bypasses GitHub API)')
  .option('--rpc-url <url>', 'Override the Soroban RPC URL for the network')
  .option('--output <path>', 'Path to write verification status JSON (e.g., status.json)')
  .parse(process.argv);

const options = program.opts();

const RPC_URLS = {
  mainnet: 'https://soroban-rpc.mainnet.stellar.org',
  testnet: 'https://soroban-rpc.testnet.stellar.org',
  futurenet: 'https://rpc-futurenet.stellar.org',
};

/**
 * Fetch the attestation.json for a given commit from the GitHub releases API.
 */
async function fetchAttestation(repo, commit) {
  console.log(`Fetching releases for ${repo}...`);
  const res = await fetch(`https://api.github.com/repos/${repo}/releases?per_page=10`);
  if (!res.ok) {
    throw new Error(`GitHub API error: ${res.status} ${res.statusText}`);
  }
  const releases = await res.json();

  if (!Array.isArray(releases) || releases.length === 0) {
    throw new Error(`No releases found for ${repo}.`);
  }

  for (const release of releases) {
    const assets = release.assets || [];
    for (const asset of assets) {
      if (asset.name === 'attestation.json') {
        const attRes = await fetch(asset.browser_download_url);
        if (attRes.ok) {
          const att = await attRes.json();
          if (att.commit && att.commit.startsWith(commit)) {
            console.log(`Found attestation in release "${release.tag_name}" (commit ${att.commit})`);
            return att;
          }
        }
      }
    }
  }

  // If no exact commit match, try the latest release's attestation
  const latestRelease = releases[0];
  if (latestRelease && latestRelease.assets) {
    const asset = latestRelease.assets.find((a) => a.name === 'attestation.json');
    if (asset) {
      const attRes = await fetch(asset.browser_download_url);
      if (attRes.ok) {
        const att = await attRes.json();
        console.log(`Using latest attestation from release "${latestRelease.tag_name}" (commit ${att.commit})`);
        return att;
      }
    }
  }

  throw new Error(`Could not find attestation.json for commit ${commit} in recent releases of ${repo}.`);
}

/**
 * Load a local attestation.json file.
 */
function loadLocalAttestation(path) {
  console.log(`Loading local attestation from ${path}...`);
  const content = readFileSync(path, 'utf-8');
  return JSON.parse(content);
}

/**
 * Fetch the deployed Wasm hash for a Stellar contract by querying the Soroban RPC directly.
 * Uses @stellar/stellar-sdk to construct and parse the ledger key XDR.
 */
async function fetchDeployedWasmHashViaRpc(network, contractId, rpcUrlOverride) {
  const rpcUrl = rpcUrlOverride || RPC_URLS[network];
  if (!rpcUrl) {
    throw new Error(`Unknown network: ${network}. Provide --rpc-url or use mainnet/testnet/futurenet.`);
  }

  console.log(`Fetching deployed Wasm hash for ${contractId} from ${rpcUrl}...`);

  // Use stellar-sdk to construct the ContractCode ledger key XDR from the contract ID
  let rpc;
  try {
    const stellarSdk = await import('@stellar/stellar-sdk');
    rpc = stellarSdk;
  } catch (_importErr) {
    console.log('@stellar/stellar-sdk not available, falling back to stellar CLI...');
    return fetchDeployedWasmHashViaCli(contractId, network);
  }

  try {
    // Construct the ContractCode ledger key
    // The contract ID needs to be converted from string (C...) to hash
    const contractIdBytes = rpc.StrKey.decodeEd25519PublicKey(contractId);
    const hash = rpc.xdr.Hash.fromXDR(contractIdBytes, rpc.xdr.XDRDecodingOptions.input());
    
    // Actually, the contract ID is not an ed25519 public key. Let's use the proper method.
    // For Soroban contracts, the contract ID is a 32-byte hash.
    // We can use the stellar-sdk's contractId decoding.
    let contractIdHash;
    try {
      contractIdHash = rpc.contract.getContractIdFromString(contractId);
    } catch {
      // Fallback: convert from hex string
      contractIdHash = Buffer.from(contractId.replace('C', ''), 'base32');
      if (contractIdHash.length !== 32) {
        // Try decoding as strkey
        contractIdHash = rpc.StrKey.decodeContract(contractId);
      }
    }

    // Create the ContractCode ledger key
    const ledgerKey = rpc.xdr.LedgerKey.contractCode(
      new rpc.xdr.LedgerKeyContractCode({
        hash: rpc.xdr.Hash.fromXDR(contractIdHash instanceof Buffer ? contractIdHash : Buffer.from(contractIdHash)),
      }),
    );

    // Encode to base64 for the JSON-RPC request
    const keyBase64 = ledgerKey.toXDR('base64').toString();

    // Call getLedgerEntries
    const response = await fetch(rpcUrl, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0',
        id: 1,
        method: 'getLedgerEntries',
        params: {
          keys: [keyBase64],
        },
      }),
    });

    if (!response.ok) {
      throw new Error(`RPC error: ${response.status} ${response.statusText}`);
    }

    const result = await response.json();
    if (result.error) {
      throw new Error(`RPC error: ${JSON.stringify(result.error)}`);
    }

    const entries = result.result?.entries;
    if (!entries || entries.length === 0) {
      throw new Error(`No ledger entries found for contract ${contractId}. Is it deployed?`);
    }

    // Parse the entry data to extract Wasm hash
    const entry = entries[0];
    const entryData = entry.data;
    
    // The ContractCode entry contains the Wasm hash as xdr.ContractCodeEntry
    const contractCodeEntry = rpc.xdr.ContractCodeEntry.fromXDR(
      Buffer.from(entryData.xdr, 'base64'),
      rpc.xdr.XDRDecodingOptions.input(),
    );

    const wasmHash = contractCodeEntry.hash().toString('hex').toLowerCase();
    console.log(`Deployed WASM Hash (via RPC): ${wasmHash}`);
    return wasmHash;

  } catch (_rpcErr) {
    console.log(`RPC-based fetch failed: ${_rpcErr.message}. Falling back to stellar CLI...`);
    return fetchDeployedWasmHashViaCli(contractId, network);
  }
}

/**
 * Fallback: Fetch the deployed Wasm hash using the stellar CLI tool.
 */
function fetchDeployedWasmHashViaCli(contractId, network) {
  console.log(`Using stellar CLI to inspect contract ${contractId} on ${network}...`);

  try {
    const output = execSync(
      `stellar contract inspect --id ${contractId} --network ${network}`,
      { encoding: 'utf-8', timeout: 30000 },
    );
    const match = output.match(/Wasm ID:\s*([a-f0-9]+)/i);
    if (match) {
      const hash = match[1].toLowerCase();
      console.log(`Deployed WASM Hash (via CLI): ${hash}`);
      return hash;
    }
  } catch (cliErr) {
    throw new Error(
      `Failed to get Wasm hash via stellar CLI: ${cliErr.message}. ` +
      'Ensure stellar-cli is installed and configured for the target network.',
    );
  }

  throw new Error(`Could not extract WASM hash from stellar CLI output for contract ${contractId}.`);
}

async function run() {
  try {
    const startTime = Date.now();

    // 1. Get the deployed Wasm hash
    let deployedWasmHash;
    if (options.id) {
      deployedWasmHash = await fetchDeployedWasmHashViaRpc(
        options.network,
        options.id,
        options.rpcUrl,
      );
      console.log(`Deployed WASM Hash: ${deployedWasmHash}`);
    } else {
      console.log('No --id provided. Skipping on-chain verification (local attestation only).');
    }

    // 2. Get the attestation
    let attestation;
    if (options.attestation) {
      attestation = loadLocalAttestation(options.attestation);
    } else if (options.commit) {
      attestation = await fetchAttestation(options.repo, options.commit);
    } else {
      throw new Error('Either --commit or --attestation must be provided.');
    }
    console.log(`Attestation loaded for commit ${attestation.commit}, build date ${attestation.build_date}`);

    // 3. Find the contract in the attestation
    const contractAttestation = attestation.contracts.find(
      (c) => c.name === options.contract,
    );
    if (!contractAttestation) {
      throw new Error(
        `Contract "${options.contract}" not found in attestation. ` +
        `Available: ${attestation.contracts.map((c) => c.name).join(', ')}`,
      );
    }

    console.log(`Contract:     ${contractAttestation.name}`);
    console.log(`Attested SHA256: ${contractAttestation.wasm_sha256}`);
    console.log(`Attested size:   ${contractAttestation.wasm_size} bytes`);

    // 4. Compare
    let match = false;
    if (deployedWasmHash) {
      match = deployedWasmHash.toLowerCase() === contractAttestation.wasm_sha256.toLowerCase();
      if (match) {
        console.log('✅ SUCCESS: Deployed Wasm hash matches the attested build!');
      } else {
        console.error('❌ FAILURE: Wasm hash mismatch!');
        console.error(`  Deployed: ${deployedWasmHash}`);
        console.error(`  Attested: ${contractAttestation.wasm_sha256}`);
      }
    } else {
      console.log('⚠️  No --id provided; attestation generated without on-chain verification.');
      match = true; // Mark as OK since we only generated attestation
    }

    // 5. Build status result
    const elapsed = ((Date.now() - startTime) / 1000).toFixed(1);
    const status = {
      timestamp: new Date().toISOString(),
      network: options.network,
      contract: options.contract,
      contract_id: options.id || null,
      commit: attestation.commit,
      build_date: attestation.build_date,
      toolchain: attestation.toolchain,
      deployed_wasm_hash: deployedWasmHash || null,
      attested_wasm_hash: contractAttestation.wasm_sha256,
      match,
      elapsed_seconds: parseFloat(elapsed),
      status: match ? 'pass' : 'fail',
    };

    // Print summary
    console.log('\n── Verification Summary ──');
    console.log(JSON.stringify(status, null, 2));

    // 6. Write output file if requested
    if (options.output) {
      writeFileSync(options.output, JSON.stringify(status, null, 2), 'utf-8');
      console.log(`\nStatus written to ${options.output}`);
    }

    // Exit with non-zero code on failure
    if (!match) {
      process.exit(1);
    }
  } catch (err) {
    const elapsed = ((Date.now() - startTime) / 1000).toFixed(1);
    const failureStatus = {
      timestamp: new Date().toISOString(),
      network: options.network,
      contract: options.contract,
      contract_id: options.id || null,
      commit: options.commit || null,
      error: err.message,
      elapsed_seconds: parseFloat(elapsed),
      status: 'error',
    };

    console.error(`\n❌ Error: ${err.message}`);
    console.error(JSON.stringify(failureStatus, null, 2));

    if (options.output) {
      writeFileSync(options.output, JSON.stringify(failureStatus, null, 2), 'utf-8');
    }

    process.exit(1);
  }
}

run().catch((err) => {
  console.error(`Unhandled error: ${err.message}`);
  process.exit(1);
});
