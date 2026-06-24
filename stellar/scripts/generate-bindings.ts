import { execSync } from 'child_process';
import * as fs from 'fs';
import * as path from 'path';

// Define paths
const STELLAR_DIR = path.resolve(__dirname, '..');
const BINDINGS_DIR = path.join(STELLAR_DIR, 'bindings', 'typescript');
const CONFIG_FILE = path.join(STELLAR_DIR, 'contract-ids.json');

// Contract Crates mapping to their package/class names and compiled WASM filenames
interface ContractMetadata {
  crateName: string;      // Crate directory name, e.g. "stealth-registry"
  wasmName: string;       // Cargo output wasm file, e.g. "stealth_registry.wasm"
  envVar: string;         // Environment variable name, e.g. "STEALTH_REGISTRY_CONTRACT_ID"
}

const CONTRACTS: ContractMetadata[] = [
  {
    crateName: 'stealth-announcer',
    wasmName: 'stealth_announcer.wasm',
    envVar: 'STEALTH_ANNOUNCER_CONTRACT_ID',
  },
  {
    crateName: 'stealth-registry',
    wasmName: 'stealth_registry.wasm',
    envVar: 'STEALTH_REGISTRY_CONTRACT_ID',
  },
  {
    crateName: 'stealth-sender',
    wasmName: 'stealth_sender.wasm',
    envVar: 'STEALTH_SENDER_CONTRACT_ID',
  },
  {
    crateName: 'wraith-names',
    wasmName: 'wraith_names.wasm',
    envVar: 'WRAITH_NAMES_CONTRACT_ID',
  },
];

// Helper to check if a command exists in the system path
function checkCommandExists(cmd: string): boolean {
  try {
    const checkCmd = process.platform === 'win32' ? `where ${cmd}` : `which ${cmd}`;
    execSync(checkCmd, { stdio: 'ignore' });
    return true;
  } catch {
    return false;
  }
}

// Load deployed contract IDs from config file
function loadConfig(): Record<string, string> {
  if (fs.existsSync(CONFIG_FILE)) {
    try {
      return JSON.parse(fs.readFileSync(CONFIG_FILE, 'utf8'));
    } catch (err) {
      console.warn(`⚠️ Failed to parse config file: ${err}`);
    }
  }
  return {};
}

// Post-process generated files to convert default exports to named exports
function postProcessBindings(directory: string): void {
  if (!fs.existsSync(directory)) return;

  const files = fs.readdirSync(directory);
  for (const file of files) {
    const fullPath = path.join(directory, file);
    const stat = fs.statSync(fullPath);

    if (stat.isDirectory()) {
      postProcessBindings(fullPath);
    } else if (file.endsWith('.ts') || file.endsWith('.js')) {
      let content = fs.readFileSync(fullPath, 'utf8');
      let changed = false;

      // 1. Replace "export default class Client" with "export class Client"
      if (content.includes('export default class Client')) {
        content = content.replace(/export default class Client/g, 'export class Client');
        changed = true;
      }

      // 2. Replace "export default class Contract" with "export class Contract"
      if (content.includes('export default class Contract')) {
        content = content.replace(/export default class Contract/g, 'export class Contract');
        changed = true;
      }

      // 3. Replace any other "export default class [Name]" with "export class [Name]"
      const defaultClassRegex = /export default class (\w+)/g;
      if (defaultClassRegex.test(content)) {
        content = content.replace(defaultClassRegex, 'export class $1');
        changed = true;
      }

      // 4. Ensure we append a named export for default-exported entities if any remain
      // and export them by name so both named and default imports work cleanly if desired.
      const defaultExportRegex = /export default (\w+);/g;
      if (defaultExportRegex.test(content)) {
        // If it exports default, make sure we also add a named export just in case
        content = content.replace(defaultExportRegex, (match, name) => {
          return `export { ${name} };\nexport default ${name};`;
        });
        changed = true;
      }

      if (changed) {
        fs.writeFileSync(fullPath, content, 'utf8');
        console.log(`✨ Post-processed exports in: ${path.relative(STELLAR_DIR, fullPath)}`);
      }
    }
  }
}

// Main runner function
async function main() {
  console.log('🚀 Starting Stellar TypeScript bindings generation...\n');

  // Verify CLI tool presence
  let cliCmd = 'stellar';
  if (!checkCommandExists('stellar')) {
    if (checkCommandExists('soroban')) {
      console.log('ℹ️ "stellar" CLI not found. Falling back to legacy "soroban" CLI.');
      cliCmd = 'soroban';
    } else {
      // Fallback for default Windows installation paths when PATH is not refreshed yet
      const windowsPaths = [
        'C:\\Program Files (x86)\\Stellar CLI\\stellar.exe',
        'C:\\Program Files\\Stellar CLI\\stellar.exe',
      ];
      let foundWindowsFallback = false;
      for (const winPath of windowsPaths) {
        if (fs.existsSync(winPath)) {
          console.log(`ℹ️ "stellar" CLI not found on PATH, but found at Windows default path: "${winPath}"`);
          cliCmd = `"${winPath}"`;
          foundWindowsFallback = true;
          break;
        }
      }

      if (!foundWindowsFallback) {
        console.error('❌ Error: Neither "stellar" nor "soroban" CLI commands were found on your PATH.');
        console.error('Please install the Stellar CLI: https://developers.stellar.org/docs/tools/developer-tools/cli/install-cli');
        process.exit(1);
      }
    }
  }

  // Load contract IDs from config and environment variables
  const config = loadConfig();
  const network = process.env.STEALTH_NETWORK || 'testnet';

  // Create bindings target directory
  if (!fs.existsSync(BINDINGS_DIR)) {
    fs.mkdirSync(BINDINGS_DIR, { recursive: true });
  }

  // Determine local compilation requirement
  let needsLocalCompilation = false;
  for (const contract of CONTRACTS) {
    const contractId = process.env[contract.envVar] || config[contract.crateName];
    if (!contractId) {
      needsLocalCompilation = true;
      break;
    }
  }

  // Local compilation if in WASM mode
  if (needsLocalCompilation) {
    console.log('⚙️ No contract IDs detected for some or all contracts. Using local WASM mode.');
    console.log('🔨 Compiling Soroban contracts to WASM locally (cargo build)...');
    try {
      execSync('cargo build --target wasm32-unknown-unknown --release', {
        cwd: STELLAR_DIR,
        stdio: 'inherit',
      });
      console.log('✅ Local compilation succeeded!\n');
    } catch (err) {
      console.error('❌ Local cargo build failed. Make sure you have the rust/wasm32 toolchain installed.');
      process.exit(1);
    }
  }

  // Generate bindings for each contract
  for (const contract of CONTRACTS) {
    const contractId = process.env[contract.envVar] || config[contract.crateName];
    const outputDir = path.join(BINDINGS_DIR, contract.crateName);

    // Clean existing bindings output folder
    if (fs.existsSync(outputDir)) {
      fs.rmSync(outputDir, { recursive: true, force: true });
    }

    let command = '';
    if (contractId) {
      console.log(`🌐 Generating bindings for "${contract.crateName}" from deployed Testnet ID: ${contractId}`);
      command = `${cliCmd} contract bindings typescript --contract-id ${contractId} --network ${network} --output-dir "${outputDir}"`;
    } else {
      const wasmPath = path.join(STELLAR_DIR, 'target', 'wasm32-unknown-unknown', 'release', contract.wasmName);
      if (!fs.existsSync(wasmPath)) {
        console.error(`❌ Error: Compiled WASM file not found at ${wasmPath}`);
        process.exit(1);
      }
      console.log(`📦 Generating bindings for "${contract.crateName}" from local WASM file: ${path.relative(STELLAR_DIR, wasmPath)}`);
      command = `${cliCmd} contract bindings typescript --wasm "${wasmPath}" --output-dir "${outputDir}"`;
    }

    try {
      execSync(command, { cwd: STELLAR_DIR, stdio: 'inherit' });
      console.log(`✅ Generated bindings for ${contract.crateName}.`);

      // Post-process output to use named exports
      postProcessBindings(outputDir);
    } catch (err) {
      console.error(`❌ Failed to generate bindings for ${contract.crateName}:`, err);
      process.exit(1);
    }
    console.log();
  }

  // Create global index.ts file re-exporting all clients
  const indexContent = `// Auto-generated Stellar TypeScript bindings re-exports
// Generated on ${new Date().toISOString()}

export * as StealthAnnouncer from './stealth-announcer/src/index';
export * as StealthRegistry from './stealth-registry/src/index';
export * as StealthSender from './stealth-sender/src/index';
export * as WraithNames from './wraith-names/src/index';
`;

  const indexFilePath = path.join(BINDINGS_DIR, 'index.ts');
  fs.writeFileSync(indexFilePath, indexContent, 'utf8');
  console.log(`📝 Generated top-level index.ts: ${path.relative(STELLAR_DIR, indexFilePath)}`);

  console.log('\n🎉 All bindings generated and post-processed successfully!');
}

main().catch((err) => {
  console.error('❌ Script failed with error:', err);
  process.exit(1);
});
