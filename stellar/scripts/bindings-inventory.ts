import * as fs from 'node:fs';
import * as path from 'node:path';

export interface BindingContract {
  name: string;
  path: string;
  wasm: string;
  exportName: string;
  envVar: string;
}
export interface BindingInventory {
  contracts: BindingContract[];
  excluded: { path: string; reason: string }[];
}

export function validateInventory(inventory: BindingInventory, members: string[]): void {
  const paths = [...inventory.contracts, ...inventory.excluded].map((entry) => entry.path);
  for (const member of members)
    if (!paths.includes(member))
      throw new Error(`Workspace member has no binding decision: ${member}`);
  for (const entry of paths)
    if (!members.includes(entry))
      throw new Error(`Binding inventory refers to an unknown workspace member: ${entry}`);
  if (new Set(paths).size !== paths.length)
    throw new Error('Duplicate workspace member in binding inventory');
  for (const key of ['name', 'wasm', 'exportName', 'envVar'] as const) {
    const values = inventory.contracts.map((contract) => contract[key]);
    if (new Set(values).size !== values.length)
      throw new Error(`Duplicate ${key} in binding inventory`);
  }
  for (const excluded of inventory.excluded)
    if (!excluded.reason.trim()) throw new Error(`Missing exclusion reason for ${excluded.path}`);
}

export function loadBindingInventory(stellar: string): BindingInventory {
  const inventory: BindingInventory = JSON.parse(
    fs.readFileSync(path.join(stellar, 'bindings/contracts.json'), 'utf8'),
  );
  const workspace = fs.readFileSync(path.join(stellar, 'Cargo.toml'), 'utf8');
  const membersSection = workspace.match(/^members\s*=\s*\[([\s\S]*?)\]/m)?.[1];
  if (!membersSection) throw new Error('Missing explicit Cargo workspace members');
  const members = [...membersSection.replace(/#.*$/gm, '').matchAll(/"([^"\n]+)"/g)].map(
    (entry) => entry[1],
  );
  validateInventory(inventory, members);
  for (const contract of inventory.contracts) {
    if (
      !/^[a-z][a-z0-9-]*$/.test(contract.name) ||
      contract.wasm !== contract.name.replaceAll('-', '_') + '.wasm'
    )
      throw new Error(`Invalid binding package metadata: ${contract.name}`);
    const manifest = path.resolve(stellar, contract.path, 'Cargo.toml');
    if (!manifest.startsWith(path.resolve(stellar) + path.sep))
      throw new Error('Contract path escapes the workspace');
    const cargo = fs.readFileSync(manifest, 'utf8');
    if (
      !cargo.includes('"cdylib"') ||
      !new RegExp(`^name\\s*=\\s*"${contract.name}"`, 'm').test(cargo)
    )
      throw new Error(`Binding target must match a contract cdylib: ${contract.name}`);
  }
  return inventory;
}

function generatedFiles(directory: string, abi: boolean, prefix = ''): string[] {
  if (!fs.existsSync(directory)) return [];
  return fs
    .readdirSync(directory, { withFileTypes: true })
    .sort((a, b) => a.name.localeCompare(b.name))
    .flatMap((entry) => {
      if (['node_modules', 'dist', '.git'].includes(entry.name)) return [];
      const relative = path.posix.join(prefix, entry.name);
      if (entry.isDirectory())
        return generatedFiles(path.join(directory, entry.name), abi, relative);
      if (abi && !entry.name.endsWith('.json')) return [];
      return [relative];
    });
}

/** Detect missing, stale and extra artifacts; build outputs/dependencies are never inputs. */
export function compareGeneratedTrees(generated: string, checkedIn: string): string[] {
  const abi = path.basename(generated) === 'abi';
  const expected = generatedFiles(generated, abi);
  const actual = generatedFiles(checkedIn, abi);
  const differences: string[] = [];
  for (const relative of new Set([...expected, ...actual])) {
    if (!expected.includes(relative)) differences.push(`Unexpected: ${relative}`);
    else if (!actual.includes(relative)) differences.push(`Missing: ${relative}`);
    else if (
      !fs
        .readFileSync(path.join(generated, relative))
        .equals(fs.readFileSync(path.join(checkedIn, relative)))
    )
      differences.push(`Stale: ${relative}`);
  }
  return differences.sort();
}
