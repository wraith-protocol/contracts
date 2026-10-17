import fs from 'node:fs';
import path from 'node:path';
import { parse as parseToml } from 'smol-toml';
import {
  CHAIN_LABEL,
  EXPECTED_CONTRACTS,
  HEIGHT_LABEL,
  ID_LABEL,
  STELLAR_BINDINGS_NETWORK,
} from './catalogue.js';
import { canonicalAbiSha256 } from './hash.js';
import { CHAINS, findDeployment, serialize, type Deployment, type Manifest } from './manifest.js';
import { repoPath } from './paths.js';
import { SUBGRAPH_TEMPLATE, templateErrors } from './subgraph.js';

/**
 * A checked-in file that repeats deployment data. `check` reports every
 * disagreement with the manifest; `write`, where present, regenerates the file.
 */
export interface Consumer {
  name: string;
  check(manifest: Manifest): string[];
  write?(manifest: Manifest): boolean;
}

const read = (rel: string) => fs.readFileSync(repoPath(rel), 'utf8');
const exists = (rel: string) => fs.existsSync(repoPath(rel));

function writeIfChanged(rel: string, content: string): boolean {
  if (exists(rel) && read(rel) === content) return false;
  fs.writeFileSync(repoPath(rel), content);
  return true;
}

function listDirs(rel: string): string[] {
  if (!exists(rel)) return [];
  return fs
    .readdirSync(repoPath(rel), { withFileTypes: true })
    .filter((d) => d.isDirectory())
    .map((d) => d.name)
    .sort();
}

// ─── EVM ─────────────────────────────────────────────────────────────────────

// The deployable evm/subgraph/subgraph.yaml is generated per network by
// `subgraph --network` and not checked in; only its template is.
const subgraphTemplate: Consumer = {
  name: SUBGRAPH_TEMPLATE,
  check() {
    if (!exists(SUBGRAPH_TEMPLATE)) return [`${SUBGRAPH_TEMPLATE}: missing`];
    return templateErrors(read(SUBGRAPH_TEMPLATE)).map((e) => `${SUBGRAPH_TEMPLATE}: ${e}`);
  },
};

interface InstantConfig {
  instances: Array<{ abi: string; address: string; chain: string; startBlock: number }>;
}

const instantConfigs: Consumer = {
  name: 'evm/subgraph/<network>/instant-config.json',
  check(manifest) {
    const errors: string[] = [];
    for (const network of listDirs('evm/subgraph')) {
      const rel = `evm/subgraph/${network}/instant-config.json`;
      if (!exists(rel)) continue;
      const deployment = findDeployment(manifest, 'evm', network);
      if (!deployment) {
        errors.push(`${rel}: no deployments/evm/${network}.json`);
        continue;
      }
      const expected = serialize(renderInstantConfig(JSON.parse(read(rel)), deployment));
      if (read(rel).trimEnd() !== expected.trimEnd()) {
        errors.push(
          `${rel}: addresses or start blocks differ from the manifest (run \`npm run build\`)`,
        );
      }
    }
    return errors;
  },
  write(manifest) {
    let changed = false;
    for (const network of listDirs('evm/subgraph')) {
      const rel = `evm/subgraph/${network}/instant-config.json`;
      const deployment = findDeployment(manifest, 'evm', network);
      if (!exists(rel) || !deployment) continue;
      changed =
        writeIfChanged(rel, serialize(renderInstantConfig(JSON.parse(read(rel)), deployment))) ||
        changed;
    }
    return changed;
  },
};

function renderInstantConfig(config: InstantConfig, deployment: Deployment): InstantConfig {
  return {
    ...config,
    instances: config.instances.map((instance) => {
      const contract = deployment.contracts.find((c) => c.name === instance.abi);
      if (!contract) return instance;
      return {
        ...instance,
        address: contract.id,
        chain: deployment.network,
        startBlock: contract.deployment.height,
      };
    }),
  };
}

const evmAbis: Consumer = {
  name: 'EVM ABI copies',
  check(manifest) {
    const errors: string[] = [];
    for (const deployment of manifest.deployments.filter((d) => d.chain === 'evm')) {
      for (const contract of deployment.contracts) {
        if (!contract.abi) continue;
        const copies = new Set([
          contract.abi.path,
          `evm/subgraph/abis/${contract.name}.json`,
          `evm/subgraph/${deployment.network}/abis/${contract.name}.json`,
        ]);
        for (const rel of copies) {
          if (!exists(rel)) continue;
          const actual = canonicalAbiSha256(JSON.parse(read(rel)));
          if (actual !== contract.abi.sha256) {
            errors.push(
              `${rel}: ABI hash ${actual}, manifest has ${contract.abi.sha256} for ${contract.name}`,
            );
          }
        }
      }
    }
    return errors;
  },
};

const HARDHAT_CONFIG = 'evm/hardhat.config.ts';

const hardhatNetworks: Consumer = {
  name: HARDHAT_CONFIG,
  check(manifest) {
    if (!exists(HARDHAT_CONFIG)) return [];
    const source = read(HARDHAT_CONFIG);
    const networks = new Map<string, number>();
    for (const m of source.matchAll(/^\s{4}(\w+):\s*\{[^}]*?chainId:\s*(\d+)/gms)) {
      networks.set(m[1], Number(m[2]));
    }
    const errors: string[] = [];
    for (const deployment of manifest.deployments.filter((d) => d.chain === 'evm')) {
      const name = deployment.network.replace(/-/g, '_');
      const chainId = networks.get(name);
      if (chainId === undefined) {
        errors.push(`${HARDHAT_CONFIG}: no network "${name}" for ${deployment.network}`);
      } else if (String(chainId) !== deployment.networkId) {
        errors.push(
          `${HARDHAT_CONFIG}: ${name} chainId ${chainId}, manifest has ${deployment.networkId}`,
        );
      }
    }
    return errors;
  },
};

// ─── Solana ──────────────────────────────────────────────────────────────────

const ANCHOR_TOML = 'solana/Anchor.toml';

const anchor: Consumer = {
  name: `${ANCHOR_TOML} and declare_id!`,
  check(manifest) {
    if (!exists(ANCHOR_TOML)) return [];
    const programs = (parseToml(read(ANCHOR_TOML)).programs ?? {}) as Record<
      string,
      Record<string, string>
    >;
    const deployments = manifest.deployments.filter((d) => d.chain === 'solana');
    const errors: string[] = [];

    for (const [cluster, ids] of Object.entries(programs)) {
      if (cluster === 'localnet') continue;
      const deployment = deployments.find((d) => d.network === `solana-${cluster}`);
      if (!deployment) {
        errors.push(
          `${ANCHOR_TOML}: [programs.${cluster}] has no deployments/solana/solana-${cluster}.json`,
        );
        continue;
      }
      const names = new Set([...Object.keys(ids), ...deployment.contracts.map((c) => c.name)]);
      for (const name of names) {
        const contract = deployment.contracts.find((c) => c.name === name);
        if (ids[name] !== contract?.id) {
          errors.push(
            `${ANCHOR_TOML}: programs.${cluster}.${name} is ${ids[name] ?? 'missing'}, manifest has ${contract?.id ?? 'nothing'}`,
          );
        }
      }
    }

    for (const deployment of deployments) {
      if (!programs[deployment.network.replace(/^solana-/, '')]) {
        errors.push(
          `${ANCHOR_TOML}: no [programs.${deployment.network.replace(/^solana-/, '')}] for ${deployment.network}`,
        );
      }
      for (const contract of deployment.contracts) {
        const rel = `solana/programs/${contract.name.replace(/_/g, '-')}/src/lib.rs`;
        if (!exists(rel)) {
          errors.push(`${rel}: missing for program ${contract.name}`);
          continue;
        }
        const declared = read(rel).match(/declare_id!\("([^"]+)"\)/)?.[1];
        if (declared !== contract.id) {
          errors.push(
            `${rel}: declare_id!("${declared}"), manifest has ${contract.id} on ${deployment.network}`,
          );
        }
      }
    }
    return errors;
  },
};

// ─── CKB ─────────────────────────────────────────────────────────────────────

const ckbKey = (name: string) => name.replace(/^wraith-/, '').replace(/-/g, '_');

const ckbNetworkFiles: Consumer = {
  name: 'ckb/<network>.toml',
  check(manifest) {
    const errors: string[] = [];
    const deployments = manifest.deployments.filter((d) => d.chain === 'ckb');
    const files = exists('ckb')
      ? fs.readdirSync(repoPath('ckb')).filter((f) => /^(testnet|mainnet|devnet)\.toml$/.test(f))
      : [];
    for (const file of files) {
      const rel = `ckb/${file}`;
      const doc = parseToml(read(rel)) as {
        network?: { name?: string };
        contracts?: Record<string, string>;
        cell_deps?: Record<string, { tx_hash: string; index: number }>;
      };
      const network = `ckb-${path.basename(file, '.toml')}`;
      const deployment = deployments.find((d) => d.network === network);
      if (!deployment) {
        errors.push(`${rel}: no deployments/ckb/${network}.json`);
        continue;
      }
      if (doc.network?.name && `ckb-${doc.network.name}` !== network) {
        errors.push(
          `${rel}: [network].name is ${doc.network.name}, expected ${network.replace(/^ckb-/, '')}`,
        );
      }
      const contracts = doc.contracts ?? {};
      const cellDeps = doc.cell_deps ?? {};
      const keys = new Set([
        ...Object.keys(contracts).map((k) => k.replace(/_code_hash$/, '')),
        ...Object.keys(cellDeps),
        ...deployment.contracts.map((c) => ckbKey(c.name)),
      ]);
      for (const key of keys) {
        const contract = deployment.contracts.find((c) => ckbKey(c.name) === key);
        if (!contract) {
          errors.push(`${rel}: ${key} is not in the manifest`);
          continue;
        }
        if (contracts[`${key}_code_hash`] !== contract.id) {
          errors.push(
            `${rel}: contracts.${key}_code_hash is ${contracts[`${key}_code_hash`] ?? 'missing'}, manifest has ${contract.id}`,
          );
        }
        const dep = cellDeps[key];
        const expected = contract.ckb!.cellDep;
        if (!dep || dep.tx_hash !== expected.txHash || dep.index !== expected.index) {
          errors.push(
            `${rel}: cell_deps.${key} is ${dep ? `${dep.tx_hash}:${dep.index}` : 'missing'}, manifest has ${expected.txHash}:${expected.index}`,
          );
        }
      }
    }
    for (const deployment of deployments) {
      if (!files.includes(`${deployment.network.replace(/^ckb-/, '')}.toml`)) {
        errors.push(
          `ckb/${deployment.network.replace(/^ckb-/, '')}.toml: missing for ${deployment.network}`,
        );
      }
    }
    return errors;
  },
};

// ─── Stellar ─────────────────────────────────────────────────────────────────

const CONTRACT_IDS = 'stellar/contract-ids.json';

function renderContractIds(manifest: Manifest): string {
  const deployment = findDeployment(manifest, 'stellar', STELLAR_BINDINGS_NETWORK);
  const names = [...EXPECTED_CONTRACTS.stellar];
  for (const c of deployment?.contracts ?? []) if (!names.includes(c.name)) names.push(c.name);
  const ids: Record<string, string> = {};
  for (const name of names)
    ids[name] = deployment?.contracts.find((c) => c.name === name)?.id ?? '';
  return serialize(ids);
}

const stellarContractIds: Consumer = {
  name: CONTRACT_IDS,
  check(manifest) {
    if (!exists(CONTRACT_IDS)) return [`${CONTRACT_IDS}: missing (run \`npm run build\`)`];
    return read(CONTRACT_IDS).trimEnd() === renderContractIds(manifest).trimEnd()
      ? []
      : [
          `${CONTRACT_IDS}: differs from the ${STELLAR_BINDINGS_NETWORK} manifest entry (run \`npm run build\`)`,
        ];
  },
  write: (manifest) => writeIfChanged(CONTRACT_IDS, renderContractIds(manifest)),
};

// ─── README ──────────────────────────────────────────────────────────────────

const README = 'README.md';
const BEGIN = '<!-- deployments:begin -->';
const END = '<!-- deployments:end -->';

export function renderReadmeBlock(manifest: Manifest): string {
  const lines = [
    BEGIN,
    '<!-- Generated from deployments/manifest.json by scripts/deployment-manifest. Do not edit by hand. -->',
    '',
  ];
  for (const chain of CHAINS) {
    const deployments = manifest.deployments.filter((d) => d.chain === chain);
    if (!deployments.length) {
      lines.push(
        `### ${CHAIN_LABEL[chain]}`,
        '',
        `No deployment recorded yet (${EXPECTED_CONTRACTS[chain].join(', ')}).`,
        '',
      );
      continue;
    }
    for (const d of deployments) {
      lines.push(`### ${CHAIN_LABEL[chain]}: ${d.network}`, '');
      const extra = chain === 'ckb' ? ' | Cell dep' : '';
      lines.push(
        `| Contract | ${ID_LABEL[chain]} | ${HEIGHT_LABEL[chain]}${extra} |`,
        `|---|---|---${extra ? '|---' : ''}|`,
      );
      for (const c of d.contracts) {
        const dep = c.ckb ? ` | \`${c.ckb.cellDep.txHash}:${c.ckb.cellDep.index}\`` : '';
        const name = c.external ? `${c.name} (dependency)` : c.name;
        lines.push(`| ${name} | \`${c.id}\` | ${c.deployment.height}${dep} |`);
      }
      const missing = EXPECTED_CONTRACTS[chain].filter(
        (n) => !d.contracts.some((c) => c.name === n),
      );
      if (missing.length) lines.push('', `Not yet deployed: ${missing.join(', ')}.`);
      lines.push('');
    }
  }
  lines.push(
    'Source commits, artifact hashes, ABI hashes and transaction hashes are in [`deployments/manifest.json`](./deployments/manifest.json).',
    END,
  );
  return lines.join('\n');
}

function renderReadme(current: string, manifest: Manifest): string | null {
  const start = current.indexOf(BEGIN);
  const end = current.indexOf(END);
  if (start === -1 || end === -1 || end < start) return null;
  return current.slice(0, start) + renderReadmeBlock(manifest) + current.slice(end + END.length);
}

const readme: Consumer = {
  name: README,
  check(manifest) {
    const rendered = renderReadme(read(README), manifest);
    if (rendered === null) return [`${README}: missing ${BEGIN} / ${END} markers`];
    return rendered === read(README)
      ? []
      : [`${README}: deployed addresses differ from the manifest (run \`npm run build\`)`];
  },
  write(manifest) {
    const rendered = renderReadme(read(README), manifest);
    return rendered !== null && writeIfChanged(README, rendered);
  },
};

export const CONSUMERS: Consumer[] = [
  subgraphTemplate,
  instantConfigs,
  evmAbis,
  hardhatNetworks,
  anchor,
  ckbNetworkFiles,
  stellarContractIds,
  readme,
];
