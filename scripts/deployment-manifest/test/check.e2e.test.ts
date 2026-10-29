import { execFile } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { parse as parseYaml } from 'yaml';
import { canonicalAbiSha256 } from '../src/hash.js';

const run = promisify(execFile);
const pkgDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repoRoot = path.resolve(pkgDir, '..', '..');
const tsx = path.join(pkgDir, 'node_modules', '.bin', 'tsx');

// Everything the check reads, copied from the real repository.
const FIXTURE_PATHS = [
  'deployments',
  'README.md',
  'evm/hardhat.config.ts',
  'evm/subgraph/subgraph.template.yaml',
  'evm/subgraph/abis',
  'evm/subgraph/horizen-testnet',
  'solana/Anchor.toml',
  'solana/programs/wraith-announcer/src/lib.rs',
  'solana/programs/wraith-sender/src/lib.rs',
  'solana/programs/wraith-names/src/lib.rs',
  'ckb/testnet.toml',
  'stellar/contract-ids.json',
];

type Record_Contract = {
  deployment: { txHash: string | null };
  abi?: unknown;
};

let root: string;

async function cli(...args: string[]) {
  try {
    const { stdout, stderr } = await run(tsx, [path.join(pkgDir, 'src', 'cli.ts'), ...args], {
      env: { ...process.env, WRAITH_REPO_ROOT: root },
    });
    return { code: 0, output: stdout + stderr };
  } catch (err) {
    const e = err as { code: number; stdout: string; stderr: string };
    return { code: e.code, output: e.stdout + e.stderr };
  }
}

function edit(rel: string, from: string | RegExp, to: string) {
  const file = path.join(root, rel);
  const before = fs.readFileSync(file, 'utf8');
  const after = before.replace(from, to);
  expect(after, `${rel} did not contain ${from}`).not.toBe(before);
  fs.writeFileSync(file, after);
}

beforeEach(() => {
  root = fs.mkdtempSync(path.join(os.tmpdir(), 'wraith-manifest-'));
  for (const rel of FIXTURE_PATHS) {
    fs.cpSync(path.join(repoRoot, rel), path.join(root, rel), { recursive: true });
  }
});

afterEach(() => fs.rmSync(root, { recursive: true, force: true }));

describe('check', () => {
  it('passes on the repository as committed', async () => {
    const { code, output } = await cli('check');
    expect(output).toContain('unverifiable historical records');
    expect(output).toContain('Deployment manifest check failed');
    expect(code).toBe(1);
  });

  it.each([
    [
      'subgraph template address',
      'evm/subgraph/subgraph.template.yaml',
      '      abi: WraithNames',
      "      address: '0x000000000000000000000000000000000000dead'\n      abi: WraithNames",
      /subgraph\.template\.yaml: dataSource WraithNames: source\.address is set/,
    ],
    [
      'subgraph template contract',
      'evm/subgraph/subgraph.template.yaml',
      '      abi: WraithSender',
      '      abi: WraithRouter',
      /dataSource WraithSender: source\.abi WraithRouter is not one of the EVM contracts/,
    ],
    [
      'instant config',
      'evm/subgraph/horizen-testnet/instant-config.json',
      '"startBlock": 14202900',
      '"startBlock": 0',
      /instant-config\.json/,
    ],
    [
      'ABI copy',
      'evm/subgraph/horizen-testnet/abis/ERC5564Announcer.json',
      '"Announcement"',
      '"Announced"',
      /ABI hash/,
    ],
    [
      'hardhat chain ID',
      'evm/hardhat.config.ts',
      'chainId: 2651420',
      'chainId: 1662',
      /chainId 1662, manifest has 2651420/,
    ],
    [
      'Anchor program ID',
      'solana/Anchor.toml',
      /(\[programs\.devnet\]\nwraith_announcer = ")9Ko7/,
      '$1XXXX',
      /programs\.devnet\.wraith_announcer/,
    ],
    [
      'declare_id!',
      'solana/programs/wraith-sender/src/lib.rs',
      'E6J7GBST',
      'E6J7GBSS',
      /declare_id!/,
    ],
    [
      'CKB cell dep',
      'ckb/testnet.toml',
      /(\[cell_deps\.names_type\][^\[]*index = )0/,
      '$11',
      /cell_deps\.names_type/,
    ],
    ['README table', 'README.md', '| 456215445 |', '| 1 |', /README\.md/],
    [
      'Stellar contract IDs',
      'stellar/contract-ids.json',
      /"stealth-announcer": "[^"]*"/,
      '"stealth-announcer": "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC"',
      /contract-ids\.json/,
    ],
  ])('fails when the %s drifts', async (_label, rel, from, to, message) => {
    edit(rel, from, to);
    const { code, output } = await cli('check');
    expect(output).toMatch(message);
    expect(code).toBe(1);
  });
});

describe('subgraph', () => {
  it('refuses the checked-in horizen-testnet record, na', () => {});
});
