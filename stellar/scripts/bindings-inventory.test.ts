import { afterEach, describe, expect, it } from 'vitest';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  compareGeneratedTrees,
  loadBindingInventory,
  validateInventory,
} from './bindings-inventory.js';
import { normalizePackage } from './generate-bindings.js';
import { CLI_BUFFER_SHIM, normalizeReservedArguments } from './bindings-normalization.js';

const stellar = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const inventory = loadBindingInventory(stellar);
const temporary: string[] = [];
function fixture() {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'wraith-binding-test-'));
  temporary.push(directory);
  return directory;
}
afterEach(() => {
  for (const directory of temporary.splice(0)) {
    if (path.dirname(directory) !== os.tmpdir()) throw new Error('Unexpected fixture path');
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

describe('supported Stellar contract inventory', () => {
  it('covers every workspace member and includes the missing application contracts', () => {
    expect(inventory.contracts.map((contract) => contract.name)).toEqual(
      expect.arrayContaining([
        'stealth-splitter',
        'stealth-vault',
        'wraith-asset-policy',
        'governance',
      ]),
    );
    expect(inventory.excluded.map((entry) => entry.path).sort()).toEqual([
      'bench',
      'bench-crossover',
      'integration-tests',
      'wraith-metrics',
    ]);
  });
  it('fails when a newly added workspace member has no binding decision', () => {
    const members = [...inventory.contracts, ...inventory.excluded].map((entry) => entry.path);
    expect(() => validateInventory(inventory, [...members, 'new-contract'])).toThrow(
      'no binding decision: new-contract',
    );
  });
  it('rejects duplicate entries and unjustified exclusions', () => {
    const members = [...inventory.contracts, ...inventory.excluded].map((entry) => entry.path);
    expect(() =>
      validateInventory(
        { ...inventory, contracts: [...inventory.contracts, inventory.contracts[0]] },
        members,
      ),
    ).toThrow('Duplicate');
    expect(() =>
      validateInventory(
        { ...inventory, excluded: inventory.excluded.map((entry) => ({ ...entry, reason: '' })) },
        members,
      ),
    ).toThrow('Missing exclusion reason');
  });
});

describe('complete generated-artifact drift gate', () => {
  it('detects a missing package, a stale ABI and an unexpected generated package', () => {
    const directory = fixture();
    const generated = path.join(directory, 'expected');
    const checkedIn = path.join(directory, 'checked');
    fs.mkdirSync(generated);
    fs.mkdirSync(checkedIn);
    fs.writeFileSync(path.join(generated, 'package.json'), '{"name":"stealth-vault"}');
    expect(compareGeneratedTrees(generated, checkedIn)).toEqual(['Missing: package.json']);
    fs.writeFileSync(path.join(checkedIn, 'package.json'), '{"name":"old-vault"}');
    fs.writeFileSync(path.join(checkedIn, 'extra.ts'), 'stale package');
    expect(compareGeneratedTrees(generated, checkedIn)).toEqual([
      'Stale: package.json',
      'Unexpected: extra.ts',
    ]);
    const abi = path.join(directory, 'abi');
    fs.mkdirSync(abi);
    fs.writeFileSync(path.join(abi, 'vault.json'), '["new signature"]');
    fs.writeFileSync(path.join(checkedIn, 'vault.json'), '["old signature"]');
    expect(compareGeneratedTrees(abi, checkedIn)).toContain('Stale: vault.json');
  });
  it('normalizes package generation deterministically, with no machine-specific paths', () => {
    const directory = fixture();
    fs.mkdirSync(path.join(directory, 'src'));
    fs.writeFileSync(
      path.join(directory, 'src/index.ts'),
      `import { Buffer } from "buffer";\n${CLI_BUFFER_SHIM}\nexport default class Client {}\n`,
    );
    fs.writeFileSync(path.join(directory, '.gitignore'), 'node_modules\r\ndist\r\n');
    fs.writeFileSync(
      path.join(directory, 'package.json'),
      '{"name":"temporary-name","version":"0.0.0","type":"module"}',
    );
    normalizePackage(directory, 'stealth-vault');
    const first = ['src/index.ts', 'package.json', 'tsconfig.json', 'README.md'].map((file) =>
      fs.readFileSync(path.join(directory, file), 'utf8'),
    );
    normalizePackage(directory, 'stealth-vault');
    expect(
      ['src/index.ts', 'package.json', 'tsconfig.json', 'README.md'].map((file) =>
        fs.readFileSync(path.join(directory, file), 'utf8'),
      ),
    ).toEqual(first);
    expect(first.join('')).not.toContain(directory);
    expect(first[0]).not.toContain('@ts-ignore');
  });
});

it('uses a plain typed parameter for a reserved ABI argument without changing the public field', () => {
  const source = 'propose: ({function, args}: {function: string, args: Buffer}) => void;';
  const normalized = normalizeReservedArguments(source);
  expect(normalized).toBe('propose: (args: {function: string, args: Buffer}) => void;');
  expect(normalizeReservedArguments(normalized)).toBe(normalized);
});
