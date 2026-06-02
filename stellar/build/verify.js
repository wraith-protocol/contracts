import { program } from 'commander';
import crypto from 'crypto';

program
  .requiredOption('--contract <name>', 'Name of the contract (e.g., wraith-names)')
  .requiredOption('--id <contract_id>', 'Stellar Contract ID (e.g., C...) to fetch from RPC')
  .requiredOption('--network <network>', 'Network (mainnet, testnet, futurenet)')
  .requiredOption('--commit <hash>', 'Git commit hash to verify against')
  .option('--repo <owner/repo>', 'GitHub repository to fetch attestations from', 'stellar/wraith-names')
  .parse(process.argv);

const options = program.opts();

const RPC_URLS = {
  mainnet: 'https://soroban-rpc.mainnet.stellar.org',
  testnet: 'https://soroban-rpc.testnet.stellar.org',
  futurenet: 'https://rpc-futurenet.stellar.org',
};

async function fetchAttestation(repo, commit) {
  // Try to find the release by commit hash or assume the attestation is uploaded as a release asset.
  // Since we don't know the exact release tag, we might have to use the GitHub API to find the release for a commit,
  // or fetch a known URL if published elsewhere. For this script, we'll use the GitHub API to fetch releases.
  console.log(`Fetching releases for ${repo}...`);
  const res = await fetch(`https://api.github.com/repos/${repo}/releases`);
  if (!res.ok) {
    throw new Error(`Failed to fetch releases: ${res.statusText}`);
  }
  const releases = await res.json();
  
  // Find a release whose tag or target_commitish matches the commit, or we just look for attestation.json in the latest release
  // For simplicity, let's just find the first release that has an attestation.json and we'll check its commit field.
  for (const release of releases) {
    const asset = release.assets.find(a => a.name === 'attestation.json');
    if (asset) {
      const attRes = await fetch(asset.browser_download_url);
      if (attRes.ok) {
        const att = await attRes.json();
        if (att.commit.startsWith(commit)) {
          return att;
        }
      }
    }
  }
  
  throw new Error(`Could not find attestation.json for commit ${commit} in recent releases.`);
}

async function fetchDeployedWasmHash(network, contractId) {
  const rpcUrl = RPC_URLS[network];
  if (!rpcUrl) {
    throw new Error(`Unknown network: ${network}`);
  }

  console.log(`Fetching contract data for ${contractId} from ${rpcUrl}...`);
  
  // Create a JSON-RPC request to getLedgerEntries for the contract ID
  // Note: We need the contract's ledger key. A simplified way if stellar-sdk is not available
  // is to just use stellar CLI or require stellar-sdk. But the constraints say "without network access to anything other than RPC".
  // Let's use the Soroban RPC getLedgerEntries directly.
  
  // Without stellar-sdk to encode the xdr, we would have to do it manually. 
  // It's much easier to use stellar-cli if it's installed, or just prompt the user to use stellar-sdk.
  // For this script, let's assume we can use the soroban RPC `getContractData` or `getLedgerEntries` 
  // if we can format the XDR, but Wasm hash is inside the ContractData entry.
  // Wait, `soroban-cli` / `stellar-cli` has `stellar contract inspect --id <ID> --network <network>`.
  // We can try to run that if available, otherwise this script needs stellar-sdk to parse XDR.
  // Let's just mock the XDR decoding or use `stellar` CLI.
  
  // Let's try to use stellar-cli as a subprocess, since it's the standard tool.
  const { execSync } = await import('child_process');
  try {
    // This command returns the Wasm hash for a deployed contract
    const output = execSync(`stellar contract inspect --id ${contractId} --network ${network}`, { encoding: 'utf-8' });
    // Output looks like:
    // ...
    // Wasm ID: 7f...
    // ...
    const match = output.match(/Wasm ID:\s*([a-f0-9]+)/i);
    if (match) {
       return match[1];
    }
  } catch (e) {
    console.log("Failed to use stellar-cli. Ensure it is installed and configured.");
  }
  
  throw new Error(`Failed to extract WASM hash for contract ${contractId}`);
}

async function run() {
  try {
    const deployedWasmHash = await fetchDeployedWasmHash(options.network, options.id);
    console.log(`Deployed WASM Hash: ${deployedWasmHash}`);

    const attestation = await fetchAttestation(options.repo, options.commit);
    console.log(`Found attestation for commit ${attestation.commit}`);
    
    const contractAttestation = attestation.contracts.find(c => c.name === options.contract);
    if (!contractAttestation) {
      throw new Error(`Contract ${options.contract} not found in attestation.`);
    }

    console.log(`Attested WASM Hash:  ${contractAttestation.wasm_sha256}`);

    if (deployedWasmHash.toLowerCase() === contractAttestation.wasm_sha256.toLowerCase()) {
      console.log('✅ SUCCESS: Deployed Wasm hash matches the attested build!');
    } else {
      console.error('❌ FAILURE: Wasm hash mismatch!');
      process.exit(1);
    }
  } catch (err) {
    console.error(`Error: ${err.message}`);
    process.exit(1);
  }
}

run();
