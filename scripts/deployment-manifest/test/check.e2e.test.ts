import { execFile } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { parse as parseYaml } from 'yaml';

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
    expect(output).toContain('consumers agree');
    expect(code).toBe(0);
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
      '"stealth-announcer": ""',
      '"stealth-announcer": "CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC"',
      /contract-ids\.json/,
    ],
  ])('fails when the %s drifts', async (_label, rel, from, to, message) => {
    edit(rel, from, to);
    const { code, output } = await cli('check');
    expect(output).toMatch(message);
    expect(code).toBe(1);
  });

  it('fails when a record changes without rebuilding the manifest', async () => {
    edit('deployments/solana/solana-devnet.json', '"height": 456215445', '"height": 456215446');
    const { code, output } = await cli('check');
    expect(output).toMatch(/manifest\.json is out of date/);
    expect(code).toBe(1);
  });

  it('fails when a record is stored under the wrong name', async () => {
    fs.renameSync(
      path.join(root, 'deployments/ckb/ckb-testnet.json'),
      path.join(root, 'deployments/ckb/ckb-mainnet.json'),
    );
    const { code, output } = await cli('check');
    expect(output).toMatch(/does not match file name ckb-mainnet\.json/);
    expect(code).toBe(1);
  });
});

describe('build', () => {
  it('regenerates the manifest and derived consumers from a new record', async () => {
    const record = {
      chain: 'stellar',
      network: 'stellar-testnet',
      networkId: 'Test SDF Network ; September 2015',
      rpcUrl: 'https://soroban-testnet.stellar.org',
      recordedBy: 'deploy-script',
      contracts: [
        {
          name: 'stealth-announcer',
          id: 'CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC',
          version: '0.1.0',
          sourceCommit: 'a'.repeat(40),
          artifactHash: { algorithm: 'sha256', value: `0x${'11'.repeat(32)}` },
          deployment: { height: 123, txHash: null, deployer: null, timestamp: null },
        },
      ],
    };
    fs.mkdirSync(path.join(root, 'deployments/stellar'), { recursive: true });
    fs.writeFileSync(
      path.join(root, 'deployments/stellar/stellar-testnet.json'),
      JSON.stringify(record),
    );

    expect((await cli('check')).code).toBe(1);
    const built = await cli('build');
    expect(built.code).toBe(0);

    const ids = JSON.parse(fs.readFileSync(path.join(root, 'stellar/contract-ids.json'), 'utf8'));
    expect(ids['stealth-announcer']).toBe(record.contracts[0].id);
    expect(ids['stealth-registry']).toBe('');
    expect(fs.readFileSync(path.join(root, 'README.md'), 'utf8')).toContain(
      '### Stellar: stellar-testnet',
    );

    const { code, output } = await cli('check');
    expect(output).toContain('4 deployments');
    expect(code).toBe(0);
  });

  it('refuses to build from an invalid record', async () => {
    edit(
      'deployments/evm/horizen-testnet.json',
      '"networkId": "2651420"',
      '"networkId": "horizen"',
    );
    const { code, output } = await cli('build');
    expect(output).toMatch(/networkId/);
    expect(code).toBe(1);
  });
});

describe('subgraph', () => {
  const RECORD = 'deployments/evm/horizen-testnet.json';
  const TEMPLATE = 'evm/subgraph/subgraph.template.yaml';
  const GENERATED = 'evm/subgraph/subgraph.yaml';

  // The three contracts horizen-testnet does not record yet, with made-up
  // addresses and blocks. They only ever live in the temporary fixture.
  const missing = [
    { name: 'WraithNames', id: `0x${'1'.repeat(40)}`, height: 14300000 },
    { name: 'WraithSender', id: `0x${'2'.repeat(40)}`, height: 14300001 },
    { name: 'WraithWithdrawer', id: `0x${'3'.repeat(40)}`, height: 14300002 },
  ];

  type EvmRecord = {
    network: string;
    contracts: Array<{ name: string; id: string; deployment: { height?: number } }>;
  };
  const readJson = (rel: string) => JSON.parse(fs.readFileSync(path.join(root, rel), 'utf8'));
  const readYaml = (rel: string) => parseYaml(fs.readFileSync(path.join(root, rel), 'utf8'));
  const generated = () => fs.existsSync(path.join(root, GENERATED));

  function recordAll(change?: (record: EvmRecord) => void) {
    const record: EvmRecord = readJson(RECORD);
    for (const { name, id, height } of missing) {
      record.contracts.push({
        name,
        id,
        version: null,
        sourceCommit: null,
        artifactHash: { algorithm: 'keccak256', value: `0x${'00'.repeat(32)}` },
        deployment: { height },
      } as EvmRecord['contracts'][number]);
    }
    change?.(record);
    fs.writeFileSync(path.join(root, RECORD), JSON.stringify(record, null, 2));
    return record;
  }

  it('refuses the checked-in horizen-testnet record, naming the three contracts it lacks', async () => {
    const { code, output } = await cli('subgraph', '--network', 'horizen-testnet');
    for (const name of ['WraithNames', 'WraithSender', 'WraithWithdrawer']) {
      expect(output).toContain(`${name}: not recorded in ${RECORD}`);
    }
    expect(output).not.toMatch(/ERC5564Announcer|ERC6538Registry/);
    expect(code).toBe(1);
    expect(generated()).toBe(false);
  });

  it('fills network, address and start block from a record with every contract', async () => {
    const record = recordAll();
    const { code, output } = await cli('subgraph', '--network', 'horizen-testnet');
    expect(output).toContain(`✓ ${GENERATED} for horizen-testnet`);
    expect(code).toBe(0);

    const template = readYaml(TEMPLATE);
    const manifest = readYaml(GENERATED);
    expect(manifest.dataSources).toHaveLength(5);
    manifest.dataSources.forEach((ds: any, i: number) => {
      const contract = record.contracts.find((c) => c.name === ds.source.abi)!;
      expect(ds.network).toBe('horizen-testnet');
      expect(ds.source.address).toBe(contract.id);
      expect(ds.source.startBlock).toBe(contract.deployment.height);
      // Nothing else differs from the template.
      const { network: _n, ...rest } = ds;
      const { address: _a, startBlock: _s, ...source } = ds.source;
      expect({ ...rest, source }).toEqual(template.dataSources[i]);
    });
    expect({ ...manifest, dataSources: [] }).toEqual({ ...template, dataSources: [] });
    expect(manifest.dataSources.map((ds: any) => ds.source.startBlock)).toEqual([
      14202900, 14202905, 14300000, 14300001, 14300002,
    ]);
    expect(fs.readFileSync(path.join(root, GENERATED), 'utf8')).toMatch(
      /^# Generated from subgraph\.template\.yaml and deployments\/evm\/horizen-testnet\.json/,
    );
  });

  it.each([
    ['has no start block', (c: EvmRecord['contracts'][number]) => delete c.deployment.height],
    ['has start block 0', (c: EvmRecord['contracts'][number]) => (c.deployment.height = 0)],
  ])('fails when a contract %s', async (_label, change) => {
    recordAll((record) => change(record.contracts.find((c) => c.name === 'WraithSender')!));
    const { code, output } = await cli('subgraph', '--network', 'horizen-testnet');
    expect(output).toMatch(/WraithSender: (no start block|start block 0)/);
    expect(output).not.toMatch(/WraithNames|WraithWithdrawer/);
    expect(code).toBe(1);
    expect(generated()).toBe(false);
  });

  it('fails when a contract has the placeholder address', async () => {
    recordAll((record) => {
      record.contracts.find((c) => c.name === 'WraithNames')!.id =
        '0x000000000000000000000000000000000000dEaD';
    });
    const { code, output } = await cli('subgraph', '--network', 'horizen-testnet');
    expect(output).toMatch(/WraithNames: .* has the placeholder address/);
    expect(code).toBe(1);
    expect(generated()).toBe(false);
  });

  it('fails for a network without a deployment record', async () => {
    const { code, output } = await cli('subgraph', '--network', 'base-sepolia');
    expect(output).toContain(
      'no deployment record deployments/evm/base-sepolia.json (recorded EVM networks: horizen-testnet)',
    );
    expect(code).toBe(1);
    expect(generated()).toBe(false);
  });

  it('--check passes only while the generated manifest matches the template and record', async () => {
    const none = await cli('subgraph', '--check');
    expect(none.output).toMatch(/subgraph\.yaml has not been generated/);
    expect(none.code).toBe(1);

    recordAll();
    expect((await cli('subgraph', '--network', 'horizen-testnet')).code).toBe(0);
    const ok = await cli('subgraph', '--check');
    expect(ok.output).toContain(`✓ ${GENERATED} is current for horizen-testnet`);
    expect(ok.code).toBe(0);

    edit(GENERATED, 'startBlock: 14300001', 'startBlock: 1');
    const edited = await cli('subgraph', '--check');
    expect(edited.output).toMatch(/subgraph\.yaml differs from/);
    expect(edited.code).toBe(1);

    // A record that loses a contract after generation blocks the deploy too.
    expect((await cli('subgraph', '--network', 'horizen-testnet')).code).toBe(0);
    fs.cpSync(path.join(repoRoot, RECORD), path.join(root, RECORD));
    const stale = await cli('subgraph', '--check');
    expect(stale.output).toContain(`WraithWithdrawer: not recorded in ${RECORD}`);
    expect(stale.code).toBe(1);
  });

  it('--check refuses a manifest whose data sources name more than one network', async () => {
    recordAll();
    expect((await cli('subgraph', '--network', 'horizen-testnet')).code).toBe(0);
    edit(GENERATED, 'network: horizen-testnet', 'network: base-sepolia');
    const { code, output } = await cli('subgraph', '--check');
    expect(output).toMatch(/subgraph\.yaml does not name exactly one network/);
    expect(code).toBe(1);
  });

  it('lists every problem of a contract, not only the first', async () => {
    recordAll((record) => {
      const names = record.contracts.find((c) => c.name === 'WraithNames')!;
      names.deployment.height = 0;
      names.id = '0x000000000000000000000000000000000000dEaD';
    });
    const { code, output } = await cli('subgraph', '--network', 'horizen-testnet');
    expect(output).toMatch(/WraithNames: start block 0/);
    expect(output).toMatch(/WraithNames: .* has the placeholder address/);
    expect(code).toBe(1);
    expect(generated()).toBe(false);
  });

  it('reports a record that fails validation and writes nothing', async () => {
    recordAll((record) => {
      record.network = 'base-sepolia';
    });
    const { code, output } = await cli('subgraph', '--network', 'horizen-testnet');
    expect(output).toContain(`${RECORD}: network "base-sepolia" does not match file name`);
    expect(code).toBe(1);
    expect(generated()).toBe(false);
  });

  it('reports a record that is not a JSON object', async () => {
    fs.writeFileSync(path.join(root, RECORD), 'null');
    const { code, output } = await cli('subgraph', '--network', 'horizen-testnet');
    expect(output).toContain(`${RECORD}: not a JSON object`);
    expect(code).toBe(1);
    expect(generated()).toBe(false);
  });

  it('refuses a template with dynamic data source templates', async () => {
    recordAll();
    fs.appendFileSync(
      path.join(root, TEMPLATE),
      '\ntemplates:\n  - kind: ethereum\n    name: Dynamic\n    source:\n      abi: WraithNames\n',
    );
    const { code, output } = await cli('subgraph', '--network', 'horizen-testnet');
    expect(output).toMatch(/has templates \(dynamic data sources\)/);
    expect(code).toBe(1);
    expect(generated()).toBe(false);
  });

  it.each([
    ['a network that is not a slug', ['--network', '../horizen-testnet'], /is not a network slug/],
    ['neither --network nor --check', [], /subgraph takes either --network <slug> or --check/],
    [
      'both --network and --check',
      ['--network', 'horizen-testnet', '--check'],
      /subgraph takes either --network <slug> or --check/,
    ],
  ])('refuses %s', async (_label, args, message) => {
    const { code, output } = await cli('subgraph', ...args);
    expect(output).toMatch(message);
    expect(code).toBe(1);
    expect(generated()).toBe(false);
  });
});
