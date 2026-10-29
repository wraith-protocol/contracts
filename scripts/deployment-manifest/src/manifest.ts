import fs from 'node:fs';
import path from 'node:path';
import { Ajv2020, type ValidateFunction } from 'ajv/dist/2020.js';
import { EXPECTED_CONTRACTS, EXPLICIT_COVERAGE_CHAINS } from './catalogue.js';
import { ID_CHECKS } from './ids.js';
import { DEPLOYMENTS_DIR, MANIFEST_PATH, REPO_ROOT, SCHEMA_PATH } from './paths.js';

export const CHAINS = ['evm', 'stellar', 'solana', 'ckb'] as const;
export type Chain = (typeof CHAINS)[number];

export interface Provenance {
  status: 'source-verified' | 'unverifiable';
  reason?: string;
}

export const COMMIT_PATTERN = /^[0-9a-f]{40}$/;
const NULL_COMMIT = '0'.repeat(40);

/** null when `value` is a usable full git commit hash, otherwise why it is not. */
export function commitError(value: unknown): string | null {
  if (typeof value !== 'string') return 'must be a 40-character lowercase hex git commit';
  if (!COMMIT_PATTERN.test(value))
    return `"${value}" is not a full 40-character lowercase hex git commit`;
  if (value === NULL_COMMIT) return 'is the all-zero null commit';
  return null;
}

/**
 * Provenance rules JSON Schema cannot express. A first-party contract is either
 * source-verified (a well-formed commit that `verify-source` reproduces) or an
 * explicitly unverifiable historical record. Records written by a deploy script
 * can only be the former.
 */
export function provenanceErrors(deployment: Deployment, contract: Contract): string[] {
  if (contract.external) return [];
  const { provenance, sourceCommit } = contract;
  if (!provenance) {
    return ['provenance is required on first-party contracts (source-verified or unverifiable)'];
  }
  const errors: string[] = [];
  if (provenance.status === 'source-verified') {
    if (sourceCommit === null || sourceCommit === undefined) {
      errors.push('provenance source-verified requires a sourceCommit, got null');
    } else {
      const problem = commitError(sourceCommit);
      if (problem) errors.push(`sourceCommit ${problem}`);
    }
  } else {
    if (deployment.recordedBy === 'deploy-script') {
      errors.push('records written by a deploy script must be source-verified, not unverifiable');
    }
    if (sourceCommit !== null) {
      errors.push('provenance unverifiable contradicts a non-null sourceCommit');
    }
    if (!provenance.reason) errors.push('provenance unverifiable requires a reason');
  }
  return errors;
}

export interface Contract {
  name: string;
  id: string;
  version: string | null;
  sourceCommit: string | null;
  provenance?: Provenance;
  external?: boolean;
  artifactHash: { algorithm: 'keccak256' | 'sha256' | 'blake2b-256'; value: string };
  abi?: { path: string; sha256: string };
  deployment: {
    height: number;
    txHash?: string | null;
    deployer?: string | null;
    timestamp?: string | null;
  };
  ckb?: {
    hashType: 'data' | 'data1' | 'data2' | 'type';
    cellDep: { txHash: string; index: number; depType: 'code' | 'dep_group' };
  };
  solana?: { programData: string; upgradeAuthority: string | null };
  stellar?: { wasmHash: string; governance: { admin: string; upgradeable: boolean } };
}

export interface Undeployed {
  name: string;
  reason: string;
}

export interface Deployment {
  chain: Chain;
  network: string;
  networkId: string;
  rpcUrl?: string;
  recordedBy: 'deploy-script' | 'chain-query';
  contracts: Contract[];
  undeployed?: Undeployed[];
}

export interface Manifest {
  $schema?: string;
  schemaVersion: 1;
  deployments: Deployment[];
}

const schema = JSON.parse(fs.readFileSync(SCHEMA_PATH, 'utf8'));
const ajv = new Ajv2020({ allErrors: true, strict: false });
ajv.addSchema(schema);
const validateManifestSchema = ajv.getSchema(schema.$id)!;
const validateDeploymentSchema = ajv.getSchema(`${schema.$id}#/$defs/deployment`)!;

function schemaErrors(validate: ValidateFunction, value: unknown): string[] {
  if (validate(value)) return [];
  return (validate.errors ?? []).map((e) => `${e.instancePath || '/'} ${e.message}`);
}

/** Schema errors plus checks JSON Schema cannot express. */
export function validateDeployment(deployment: Deployment): string[] {
  const errors = schemaErrors(validateDeploymentSchema, deployment);
  if (errors.length) return errors;

  const checks = ID_CHECKS[deployment.chain];
  const seen = new Set<string>();
  deployment.contracts.forEach((contract, i) => {
    const where = `/contracts/${i} (${contract.name})`;
    if (seen.has(contract.name)) errors.push(`${where} duplicate contract name`);
    seen.add(contract.name);
    const idError = checks.contract(contract.id);
    if (idError) errors.push(`${where} ${idError}`);
    const deployer = contract.deployment.deployer;
    if (deployer && checks.deployer) {
      const deployerError = checks.deployer(deployer);
      if (deployerError) errors.push(`${where} deployer: ${deployerError}`);
    }
    if (contract.abi && !fs.existsSync(path.join(REPO_ROOT, contract.abi.path))) {
      errors.push(`${where} abi.path ${contract.abi.path} does not exist`);
    }
    if (deployment.recordedBy === 'deploy-script' && !contract.sourceCommit && !contract.external) {
      errors.push(`${where} records written by a deploy script must carry sourceCommit`);
    }
    errors.push(...provenanceErrors(deployment, contract).map((e) => `${where} ${e}`));
  });

  const supported = EXPECTED_CONTRACTS[deployment.chain];
  const undeployed = new Set<string>();
  (deployment.undeployed ?? []).forEach((entry, i) => {
    const where = `/undeployed/${i} (${entry.name})`;
    if (undeployed.has(entry.name)) errors.push(`${where} duplicate undeployed name`);
    undeployed.add(entry.name);
    if (!supported.includes(entry.name)) {
      errors.push(
        `${where} is not a supported ${deployment.chain} contract (${supported.join(', ')})`,
      );
    }
    if (seen.has(entry.name)) {
      errors.push(`${where} is also recorded under contracts; it is either deployed or not`);
    }
  });
  return errors;
}

/**
 * What a record must say beyond the schema, for chains in EXPLICIT_COVERAGE_CHAINS:
 * every supported contract is recorded or explicitly marked undeployed, and an
 * EVM contract that is recorded carries its deployment transaction and ABI.
 * Run it after validateDeployment passes.
 */
export function coverageErrors(deployment: Deployment): string[] {
  if (!EXPLICIT_COVERAGE_CHAINS.includes(deployment.chain)) return [];
  const errors: string[] = [];
  const accounted = new Set([
    ...deployment.contracts.map((c) => c.name),
    ...(deployment.undeployed ?? []).map((u) => u.name),
  ]);
  for (const name of EXPECTED_CONTRACTS[deployment.chain]) {
    if (!accounted.has(name)) {
      errors.push(
        `/contracts ${name} has no record: record its deployment, or list it under "undeployed" with a reason`,
      );
    }
  }
  if (deployment.chain === 'evm') {
    deployment.contracts.forEach((contract, i) => {
      const where = `/contracts/${i} (${contract.name})`;
      if (!contract.deployment.txHash) {
        errors.push(`${where} deployment.txHash is required for a deployed EVM contract`);
      }
      if (!contract.abi) errors.push(`${where} abi is required for a deployed EVM contract`);
    });
  }
  return errors;
}

export function validateManifest(manifest: Manifest): string[] {
  const errors = schemaErrors(validateManifestSchema, manifest);
  if (errors.length) return errors;
  const keys = new Set<string>();
  for (const d of manifest.deployments) {
    const key = `${d.chain}/${d.network}`;
    if (keys.has(key)) errors.push(`duplicate deployment ${key}`);
    keys.add(key);
    const problems = validateDeployment(d);
    if (!problems.length) problems.push(...coverageErrors(d));
    errors.push(...problems.map((e) => `${key}${e}`));
  }
  return errors;
}

export interface LoadedRecord {
  file: string;
  deployment: Deployment;
}

/**
 * Parse one record stored under deployments/<chain>/ and list everything wrong
 * with it. `deployment` is returned whenever the file holds a JSON object, even
 * if `errors` is not empty.
 */
export function readRecord(
  chain: Chain,
  file: string,
): { deployment?: Deployment; errors: string[] } {
  const name = path.basename(file);
  let deployment: Deployment;
  try {
    deployment = JSON.parse(fs.readFileSync(file, 'utf8'));
  } catch (err) {
    return { errors: [`invalid JSON (${(err as Error).message})`] };
  }
  if (typeof deployment !== 'object' || deployment === null || Array.isArray(deployment)) {
    return { errors: ['not a JSON object'] };
  }
  const errors = validateDeployment(deployment);
  if (!errors.length) errors.push(...coverageErrors(deployment));
  if (deployment.chain !== chain)
    errors.push(`chain "${deployment.chain}" but stored under ${chain}/`);
  if (`${deployment.network}.json` !== name) {
    errors.push(`network "${deployment.network}" does not match file name ${name}`);
  }
  return { deployment, errors };
}

/** Read deployments/<chain>/<network>.json, failing on anything malformed or misplaced. */
export function loadRecords(dir = DEPLOYMENTS_DIR): { records: LoadedRecord[]; errors: string[] } {
  const records: LoadedRecord[] = [];
  const errors: string[] = [];
  for (const chain of CHAINS) {
    const chainDir = path.join(dir, chain);
    if (!fs.existsSync(chainDir)) continue;
    for (const name of fs.readdirSync(chainDir).sort()) {
      if (!name.endsWith('.json')) continue;
      const file = path.relative(REPO_ROOT, path.join(chainDir, name));
      const { deployment, errors: problems } = readRecord(chain, path.join(chainDir, name));
      if (problems.length) errors.push(...problems.map((p) => `${file}: ${p}`));
      else records.push({ file, deployment: deployment! });
    }
  }
  return { records, errors };
}

const chainOrder = (c: Chain) => CHAINS.indexOf(c);

export function buildManifest(deployments: Deployment[]): Manifest {
  const sorted = [...deployments].sort(
    (a, b) => chainOrder(a.chain) - chainOrder(b.chain) || a.network.localeCompare(b.network),
  );
  return { $schema: './schema/v1.json', schemaVersion: 1, deployments: sorted };
}

export const serialize = (value: unknown) => `${JSON.stringify(value, null, 2)}\n`;

export function readManifest(file = MANIFEST_PATH): Manifest {
  return JSON.parse(fs.readFileSync(file, 'utf8'));
}

export function findDeployment(manifest: Manifest, chain: Chain, network: string) {
  return manifest.deployments.find((d) => d.chain === chain && d.network === network);
}
