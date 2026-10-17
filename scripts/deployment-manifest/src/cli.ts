import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { parseArgs } from 'node:util';
import { CONSUMERS } from './consumers.js';
import {
  buildManifest,
  loadRecords,
  readManifest,
  serialize,
  validateDeployment,
  validateManifest,
  type Chain,
  type Contract,
  type Deployment,
} from './manifest.js';
import { MANIFEST_PATH, REPO_ROOT, recordPath, repoPath } from './paths.js';
import { ckbNetworkId, recordCkbScript } from './record/ckb.js';
import { evmNetworkId, recordEvmContract } from './record/evm.js';
import { recordSolanaProgram, solanaNetworkId } from './record/solana.js';
import {
  recordStellarContract,
  stellarInstance,
  stellarNetworkId,
  wasmHashFromInstanceEntry,
} from './record/stellar.js';
import {
  generateSubgraph,
  generatedNetwork,
  SUBGRAPH_MANIFEST,
  SUBGRAPH_TEMPLATE,
} from './subgraph.js';

const USAGE = `Usage: tsx src/cli.ts <command>

  build     Build deployments/manifest.json from deployments/<chain>/<network>.json
            and regenerate the README block, stellar/contract-ids.json and the
            subgraph instant configs.
  check     Fail if a record is invalid, the manifest is stale, or any checked-in
            consumer disagrees with it. Offline; this is what CI runs.
  verify    Re-read every deployment from its chain and compare IDs, heights and
            artifact hashes. Needs network access.
  subgraph --network <slug>
            Write evm/subgraph/subgraph.yaml from evm/subgraph/subgraph.template.yaml
            and deployments/evm/<slug>.json. Fails, naming each one, if a contract
            the template indexes or its start block is not recorded.
  subgraph --check
            Fail unless evm/subgraph/subgraph.yaml is what --network would write now
            for the network it names. The evm/subgraph deploy scripts run it first.
  record <evm|stellar|solana|ckb> --network <slug> --rpc <https-url> [targets...]
            Read deployments from the chain and write deployments/<chain>/<network>.json,
            merging with any existing record, then run build.
    evm      --contract Name=0xAddress[@0xDeployTx] [--abi Name=path]
    stellar  --contract name=CContractId --wasm name=path/to.wasm [--deployer G...]
    solana   --program name=ProgramId
    ckb      --script name=0xTxHash:index:hashType[:external]
    common   --recorded-by deploy-script|chain-query  --source-commit <sha|HEAD>
             --version <semver> | --version name=<semver>   (repeatable)
             --public-rpc <https-url>   stored as rpcUrl for \`verify\`. --rpc is never
                                        stored, so it may carry an API key.
`;

const userPath = (p: string) => path.resolve(process.env.INIT_CWD ?? process.cwd(), p);

function pairs(values: string[] | undefined, flag: string): Map<string, string> {
  const map = new Map<string, string>();
  for (const value of values ?? []) {
    const eq = value.indexOf('=');
    if (eq < 1) throw new Error(`${flag} expects name=value, got "${value}"`);
    map.set(value.slice(0, eq), value.slice(eq + 1));
  }
  return map;
}

function fail(lines: string[]): never {
  for (const line of lines) console.error(`  ✗ ${line}`);
  process.exit(1);
}

function build(): void {
  const { records, errors } = loadRecords();
  if (errors.length) fail(errors);
  const manifest = buildManifest(records.map((r) => r.deployment));
  const manifestErrors = validateManifest(manifest);
  if (manifestErrors.length) fail(manifestErrors);
  fs.writeFileSync(MANIFEST_PATH, serialize(manifest));
  console.log(`✓ ${path.relative(REPO_ROOT, MANIFEST_PATH)} (${records.length} deployments)`);
  for (const consumer of CONSUMERS) {
    if (consumer.write?.(manifest)) console.log(`✓ updated ${consumer.name}`);
  }
}

function check(): void {
  const { records, errors } = loadRecords();
  const problems = [...errors];
  const expected = serialize(buildManifest(records.map((r) => r.deployment)));
  if (!fs.existsSync(MANIFEST_PATH)) {
    problems.push('deployments/manifest.json is missing (run `npm run build`)');
  } else {
    const manifest = readManifest();
    problems.push(...validateManifest(manifest).map((e) => `deployments/manifest.json: ${e}`));
    if (fs.readFileSync(MANIFEST_PATH, 'utf8') !== expected) {
      problems.push(
        'deployments/manifest.json is out of date with deployments/<chain>/*.json (run `npm run build`)',
      );
    }
    for (const consumer of CONSUMERS) problems.push(...consumer.check(manifest));

    const unknown = manifest.deployments.flatMap((d) =>
      d.contracts
        .filter((c) => !c.sourceCommit && !c.external)
        .map((c) => `${d.network}/${c.name}`),
    );
    if (unknown.length) console.log(`ℹ source commit not recorded for: ${unknown.join(', ')}`);
  }
  if (problems.length) {
    console.error(`Deployment manifest check failed (${problems.length}):`);
    fail(problems);
  }
  console.log(
    `✓ manifest valid; ${records.length} deployments; ${CONSUMERS.length} consumers agree`,
  );
}

function subgraph(argv: string[]): void {
  const { values } = parseArgs({
    args: argv,
    options: { network: { type: 'string' }, check: { type: 'boolean', default: false } },
  });
  if (values.check === Boolean(values.network)) {
    throw new Error('subgraph takes either --network <slug> or --check');
  }
  const out = repoPath(SUBGRAPH_MANIFEST);

  if (values.network) {
    const { manifest, errors } = generateSubgraph(values.network);
    if (manifest === null) {
      console.error(`Cannot generate ${SUBGRAPH_MANIFEST} for ${values.network}:`);
      fail(errors);
    }
    fs.writeFileSync(out, manifest);
    console.log(`✓ ${SUBGRAPH_MANIFEST} for ${values.network}`);
    return;
  }

  if (!fs.existsSync(out)) {
    fail([
      `${SUBGRAPH_MANIFEST} has not been generated (run \`npm run manifest -- --network <network>\` in evm/subgraph)`,
    ]);
  }
  const current = fs.readFileSync(out, 'utf8');
  const network = generatedNetwork(current);
  if (!network) fail([`${SUBGRAPH_MANIFEST} does not name exactly one network; regenerate it`]);
  const { manifest, errors } = generateSubgraph(network);
  if (manifest === null) {
    console.error(`${SUBGRAPH_MANIFEST} can no longer be generated for ${network}:`);
    fail(errors);
  }
  if (manifest !== current) {
    fail([
      `${SUBGRAPH_MANIFEST} differs from ${SUBGRAPH_TEMPLATE} and deployments/evm/${network}.json (run \`npm run manifest -- --network ${network}\`)`,
    ]);
  }
  console.log(`✓ ${SUBGRAPH_MANIFEST} is current for ${network}`);
}

type TargetArgs = Partial<
  Record<'contract' | 'program' | 'script' | 'wasm' | 'abi' | 'deployer', string[]>
>;

async function recordContracts(chain: Chain, rpc: string, args: TargetArgs, existing?: Deployment) {
  const out: Array<Omit<Contract, 'version' | 'sourceCommit'>> = [];
  if (chain === 'evm') {
    const abis = pairs(args.abi, '--abi');
    for (const [name, value] of pairs(args.contract, '--contract')) {
      const [address, txHash] = value.split('@');
      out.push(await recordEvmContract(rpc, { name, address, txHash, abiPath: abis.get(name) }));
    }
  } else if (chain === 'solana') {
    for (const [name, id] of pairs(args.program, '--program'))
      out.push(await recordSolanaProgram(rpc, name, id));
  } else if (chain === 'ckb') {
    for (const [name, value] of pairs(args.script, '--script')) {
      const [txHash, index, hashType, external] = value.split(':');
      if (!['data', 'data1', 'data2', 'type'].includes(hashType))
        throw new Error(`--script ${name}: bad hashType "${hashType}"`);
      out.push(
        await recordCkbScript(rpc, {
          name,
          txHash,
          index: Number(index),
          hashType: hashType as 'data',
          external: external === 'external',
        }),
      );
    }
  } else {
    const wasm = pairs(args.wasm, '--wasm');
    for (const [name, contractId] of pairs(args.contract, '--contract')) {
      const wasmPath = wasm.get(name);
      if (!wasmPath) throw new Error(`--contract ${name} needs --wasm ${name}=<path>`);
      out.push(
        await recordStellarContract(rpc, {
          name,
          contractId,
          wasmPath: userPath(wasmPath),
          deployer: args.deployer?.[0],
        }),
      );
    }
  }
  if (!out.length && !existing) throw new Error('nothing to record');
  return out;
}

const NETWORK_ID: Record<Chain, (rpc: string) => Promise<string>> = {
  evm: evmNetworkId,
  stellar: stellarNetworkId,
  solana: solanaNetworkId,
  ckb: ckbNetworkId,
};

async function record(chain: Chain, argv: string[]): Promise<void> {
  const { values } = parseArgs({
    args: argv,
    options: {
      network: { type: 'string' },
      rpc: { type: 'string' },
      contract: { type: 'string', multiple: true },
      program: { type: 'string', multiple: true },
      script: { type: 'string', multiple: true },
      wasm: { type: 'string', multiple: true },
      abi: { type: 'string', multiple: true },
      deployer: { type: 'string', multiple: true },
      'recorded-by': { type: 'string', default: 'chain-query' },
      'source-commit': { type: 'string' },
      version: { type: 'string', multiple: true },
      'public-rpc': { type: 'string' },
    },
  });
  if (!values.network || !values.rpc) throw new Error('--network and --rpc are required');

  const file = recordPath(chain, values.network);
  const existing: Deployment | undefined = fs.existsSync(file)
    ? JSON.parse(fs.readFileSync(file, 'utf8'))
    : undefined;
  const networkId = await NETWORK_ID[chain](values.rpc);
  if (existing && existing.networkId !== networkId) {
    throw new Error(
      `${values.rpc} is network ${networkId}, but ${path.relative(REPO_ROOT, file)} is ${existing.networkId}`,
    );
  }

  let sourceCommit = values['source-commit'] ?? null;
  if (sourceCommit === 'HEAD') {
    sourceCommit = execFileSync('git', ['rev-parse', 'HEAD'], {
      cwd: REPO_ROOT,
      encoding: 'utf8',
    }).trim();
    const dirty = execFileSync('git', ['status', '--porcelain', '--untracked-files=no'], {
      cwd: REPO_ROOT,
      encoding: 'utf8',
    });
    if (dirty.trim())
      console.warn(
        '⚠ working tree has uncommitted changes; sourceCommit may not match the deployed code',
      );
  }

  const versions = new Map<string, string>();
  let defaultVersion: string | null = null;
  for (const v of values.version ?? []) {
    if (v.includes('=')) versions.set(v.slice(0, v.indexOf('=')), v.slice(v.indexOf('=') + 1));
    else defaultVersion = v;
  }

  const recorded = await recordContracts(chain, values.rpc, values, existing);
  const contracts = new Map((existing?.contracts ?? []).map((c) => [c.name, c]));
  for (const { name, id, ...rest } of recorded) {
    contracts.set(name, {
      name,
      id,
      version: rest.external ? null : (versions.get(name) ?? defaultVersion),
      sourceCommit: rest.external ? null : sourceCommit,
      ...rest,
    });
  }

  const deployment: Deployment = {
    chain,
    network: values.network,
    networkId,
    ...((values['public-rpc'] ?? existing?.rpcUrl)
      ? { rpcUrl: values['public-rpc'] ?? existing?.rpcUrl }
      : {}),
    recordedBy: values['recorded-by'] as Deployment['recordedBy'],
    contracts: [...contracts.values()],
  };
  const errors = validateDeployment(deployment);
  if (errors.length) fail(errors);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, serialize(deployment));
  console.log(`✓ ${path.relative(REPO_ROOT, file)}: ${recorded.map((c) => c.name).join(', ')}`);
  build();
}

async function verify(): Promise<void> {
  const manifest = readManifest();
  const problems: string[] = [];
  for (const d of manifest.deployments) {
    if (!d.rpcUrl) {
      console.log(`- ${d.chain}/${d.network}: no rpcUrl, skipped`);
      continue;
    }
    const networkId = await NETWORK_ID[d.chain](d.rpcUrl);
    if (networkId !== d.networkId)
      problems.push(`${d.network}: RPC reports network ${networkId}, manifest has ${d.networkId}`);
    for (const c of d.contracts) {
      const where = `${d.network}/${c.name}`;
      try {
        if (d.chain === 'stellar') {
          const entry = await stellarInstance(d.rpcUrl, c.id);
          const hash = entry && wasmHashFromInstanceEntry(entry.xdr);
          if (hash !== c.artifactHash.value)
            problems.push(
              `${where}: on-chain wasm hash ${hash}, manifest has ${c.artifactHash.value}`,
            );
          continue;
        }
        const live =
          d.chain === 'evm'
            ? await recordEvmContract(d.rpcUrl, {
                name: c.name,
                address: c.id,
                txHash: c.deployment.txHash ?? undefined,
                abiPath: c.abi?.path,
              })
            : d.chain === 'solana'
              ? await recordSolanaProgram(d.rpcUrl, c.name, c.id)
              : await recordCkbScript(d.rpcUrl, {
                  name: c.name,
                  ...c.ckb!.cellDep,
                  hashType: c.ckb!.hashType,
                  external: c.external,
                });
        if (live.id.toLowerCase() !== c.id.toLowerCase())
          problems.push(`${where}: on-chain id ${live.id}, manifest has ${c.id}`);
        if (live.artifactHash.value !== c.artifactHash.value) {
          problems.push(
            `${where}: on-chain artifact hash ${live.artifactHash.value}, manifest has ${c.artifactHash.value}`,
          );
        }
        if (live.deployment.height !== c.deployment.height) {
          problems.push(
            `${where}: on-chain height ${live.deployment.height}, manifest has ${c.deployment.height}`,
          );
        }
      } catch (err) {
        problems.push(`${where}: ${(err as Error).message}`);
      }
      console.log(`- ${where}`);
    }
  }
  if (problems.length) fail(problems);
  console.log('✓ every deployment matches its chain');
}

async function main(): Promise<void> {
  const [command, ...rest] = process.argv.slice(2);
  if (command === 'build') return build();
  if (command === 'check') return check();
  if (command === 'verify') return verify();
  if (command === 'subgraph') return subgraph(rest);
  if (command === 'record' && ['evm', 'stellar', 'solana', 'ckb'].includes(rest[0])) {
    return record(rest[0] as Chain, rest.slice(1));
  }
  console.error(USAGE);
  process.exit(command ? 1 : 0);
}

main().catch((err) => {
  console.error(`✗ ${(err as Error).message}`);
  process.exit(1);
});
