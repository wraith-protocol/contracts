import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { loadBindingInventory, compareGeneratedTrees } from './bindings-inventory.js';
import {
  normalizeBufferRuntimeSource,
  normalizeReservedArguments,
} from './bindings-normalization.js';
export {
  CLI_BUFFER_SHIM,
  RUNTIME_NEUTRAL_BUFFER_SHIM,
  normalizeBufferRuntimeSource,
} from './bindings-normalization.js';

const stellar = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

export function normalizePackage(directory: string, name: string): void {
  const source = path.join(directory, 'src/index.ts');
  const content = fs
    .readFileSync(source, 'utf8')
    .replaceAll('\r\n', '\n')
    .replace(/[\t ]+$/gm, '')
    .replace(/export default class (\w+)/g, 'export class $1');
  fs.writeFileSync(source, normalizeReservedArguments(normalizeBufferRuntimeSource(content)));
  const ignorePath = path.join(directory, '.gitignore');
  fs.writeFileSync(ignorePath, fs.readFileSync(ignorePath, 'utf8').replaceAll('\r\n', '\n'));
  const packagePath = path.join(directory, 'package.json');
  const pkg = JSON.parse(fs.readFileSync(packagePath, 'utf8'));
  pkg.name = name;
  pkg.exports = { '.': { types: './dist/index.d.ts', import: './dist/index.js' } };
  pkg.files = ['dist'];
  pkg.scripts = { build: 'tsc', check: 'tsc --noEmit' };
  fs.writeFileSync(packagePath, JSON.stringify(pkg, null, 2) + '\n');
  fs.writeFileSync(
    path.join(directory, 'tsconfig.json'),
    JSON.stringify(
      {
        compilerOptions: {
          target: 'ES2022',
          module: 'NodeNext',
          moduleResolution: 'NodeNext',
          strict: true,
          declaration: true,
          outDir: 'dist',
          rootDir: 'src',
          skipLibCheck: true,
        },
        include: ['src/**/*.ts'],
      },
      null,
      2,
    ) + '\n',
  );
  fs.writeFileSync(
    path.join(directory, 'README.md'),
    `# ${name}\n\nGenerated with Stellar CLI 26.1.0 from the local contract WASM.\nDo not edit generated files; run \`pnpm bindings:stellar\` from the repository root.\n\nBuild with \`npm run build\`; import the named \`Client\` export from \`${name}\`.\nSupply the deployment's contractId, networkPassphrase and rpcUrl explicitly.\nSee the parent bindings README for supported contracts and validation.\n`,
  );
}

export function generateBindings(args = process.argv.slice(2)): void {
  const allowed = new Set(['--check', '--abi-only', '--deployed']);
  if (args.some((arg) => !allowed.has(arg)))
    throw new Error('Usage: bindings:stellar [--check] [--abi-only] [--deployed]');
  const check = args.includes('--check');
  const abiOnly = args.includes('--abi-only');
  const deployed = args.includes('--deployed');
  if (check && deployed) throw new Error('Drift checks require local WASM, not deployed contracts');
  const { contracts } = loadBindingInventory(stellar);
  const cli = process.env.STELLAR_CLI ?? 'stellar';
  const version = execFileSync(cli, ['--version'], { encoding: 'utf8' });
  if (!/\b26\.1\.0\b/.test(version))
    throw new Error('Use the reviewed Stellar CLI 26.1.0 pin (see SUPPLY_CHAIN.md)');
  // Always build from current sources. Existing WASM may belong to an older checkout.
  execFileSync('cargo', ['build', '--locked', '--target', 'wasm32-unknown-unknown', '--release'], {
    cwd: stellar,
    stdio: 'inherit',
  });
  const release = path.join(
    process.env.CARGO_TARGET_DIR ?? path.join(stellar, 'target'),
    'wasm32-unknown-unknown/release',
  );
  const configPath = path.join(stellar, 'contract-ids.json');
  const config =
    deployed && fs.existsSync(configPath) ? JSON.parse(fs.readFileSync(configPath, 'utf8')) : {};
  if (deployed)
    for (const contract of contracts) {
      if (!(process.env[contract.envVar] || config[contract.name]))
        throw new Error(`Missing deployed ID for ${contract.name}`);
    }
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'wraith-bindings-'));
  try {
    const bindings = path.join(temporary, 'typescript');
    const abi = path.join(temporary, 'abi');
    fs.mkdirSync(bindings);
    fs.mkdirSync(abi);
    for (const contract of contracts) {
      const wasm = path.join(release, contract.wasm);
      if (!fs.existsSync(wasm)) throw new Error(`Missing WASM for ${contract.name}: ${wasm}`);
      const snapshot = execFileSync(
        cli,
        ['contract', 'info', 'interface', '--wasm', wasm, '--output', 'json-formatted'],
        { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 },
      );
      // Normalize formatting/newlines without changing the pinned CLI's schema.
      fs.writeFileSync(
        path.join(abi, contract.wasm.replace('.wasm', '.json')),
        JSON.stringify(JSON.parse(snapshot), null, 2) + '\n',
      );
      if (abiOnly) continue;
      const output = path.join(bindings, contract.name);
      const source = deployed
        ? [
            '--contract-id',
            process.env[contract.envVar] || config[contract.name],
            '--network',
            process.env.STEALTH_NETWORK || 'testnet',
          ]
        : ['--wasm', wasm];
      execFileSync(cli, ['contract', 'bindings', 'typescript', ...source, '--output-dir', output], {
        cwd: stellar,
        stdio: 'inherit',
      });
      normalizePackage(output, contract.name);
    }
    if (!abiOnly)
      fs.writeFileSync(
        path.join(bindings, 'index.ts'),
        '// Auto-generated Stellar TypeScript bindings re-exports\n\n' +
          contracts
            .map((c) => `export * as ${c.exportName} from './${c.name}/src/index.js';`)
            .join('\n') +
          '\n',
      );
    const destinations = [[abi, path.join(stellar, 'abi')]];
    if (!abiOnly) destinations.push([bindings, path.join(stellar, 'bindings/typescript')]);
    for (const [generated, checkedIn] of destinations) {
      const differences = compareGeneratedTrees(generated, checkedIn);
      if (check && differences.length)
        throw new Error(`Stale or missing generated artifacts:\n${differences.join('\n')}`);
      if (!check) fs.cpSync(generated, checkedIn, { recursive: true });
    }
    console.log(
      check
        ? 'All supported bindings and ABI snapshots are current.'
        : 'Generated all supported bindings and ABI snapshots.',
    );
  } finally {
    // mkdtemp returns a private directory beneath the fixed OS temporary root.
    if (path.dirname(temporary) !== os.tmpdir()) throw new Error('Unexpected temporary path');
    fs.rmSync(temporary, { recursive: true, force: true });
  }
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) generateBindings();
