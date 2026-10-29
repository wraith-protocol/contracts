import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { ckbHashHex, fromHex, keccak256Hex, sha256Hex } from './hash.js';
import { COMMIT_PATTERN, commitError, type Chain, type Contract } from './manifest.js';
import { REPO_ROOT } from './paths.js';
import { jsonRpc } from './record/rpc.js';

/* -------------------------------------------------------------------------- */
/* Git helpers                                                                 */
/* -------------------------------------------------------------------------- */

function git(args: string[], cwd: string): string {
  return execFileSync('git', args, {
    cwd,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  }).trim();
}

/**
 * Turn `--source-commit` into a full 40-hex commit that exists in this repository.
 * Only `HEAD` or a full hash is accepted: short hashes, branch names and tags are
 * ambiguous or movable, so they are not provenance.
 */
export function resolveSourceCommit(input: string, repoRoot = REPO_ROOT): string {
  let commit = input;
  if (input === 'HEAD') {
    commit = git(['rev-parse', 'HEAD'], repoRoot);
  } else {
    commit = input.toLowerCase();
    const problem = commitError(commit);
    if (problem) throw new Error(`--source-commit ${problem} (or pass HEAD)`);
  }
  try {
    git(['cat-file', '-e', `${commit}^{commit}`], repoRoot);
  } catch {
    throw new Error(
      `--source-commit ${commit} is not a commit in this repository (shallow clone? try \`git fetch --unshallow\`)`,
    );
  }
  return commit;
}

/** Tracked files with uncommitted changes. A dirty tree means HEAD is not what was built. */
export function dirtyFiles(repoRoot = REPO_ROOT): string[] {
  const out = git(['status', '--porcelain', '--untracked-files=no'], repoRoot);
  return out ? out.split('\n') : [];
}

/** Commits (newest first) that touched the chain's sources, optionally no later than `before`. */
export function candidateCommits(
  chain: Chain,
  before: string | null | undefined,
  max: number,
  repoRoot = REPO_ROOT,
): string[] {
  const args = ['log', '--format=%H', `--max-count=${max}`];
  if (before) args.push(`--before=${before}`);
  args.push('--', ...SOURCE_PATHS[chain]);
  const out = git(args, repoRoot);
  return out ? out.split('\n').filter((c) => COMMIT_PATTERN.test(c)) : [];
}

const SOURCE_PATHS: Record<Chain, string[]> = {
  evm: ['evm/contracts', 'evm/hardhat.config.ts', 'evm/package.json', 'evm/package-lock.json'],
  stellar: ['stellar'],
  solana: ['solana'],
  ckb: ['ckb'],
};

/* -------------------------------------------------------------------------- */
/* Builders                                                                    */
/* -------------------------------------------------------------------------- */

export interface Built {
  /** The artifact exactly as it would be hashed for the manifest. */
  bytes: Uint8Array;
  /** Byte ranges the compiler leaves empty and the deployment fills in (Solidity immutables). */
  immutableRanges?: Array<{ start: number; length: number }>;
}

export interface Builder {
  /** Build everything once for a checked-out commit (dir is a clean checkout). */
  prepare(dir: string): void | Promise<void>;
  /** Pick one contract's artifact out of the build. */
  collect(dir: string, name: string): Built | Promise<Built>;
}

const SAFE_NAME = /^[A-Za-z0-9_.-]+$/;

function run(cmd: string, args: string[], cwd: string): void {
  // shell:true only on Windows, where npm/npx are .cmd shims. Arguments are fixed strings.
  execFileSync(cmd, args, {
    cwd,
    stdio: ['ignore', 'inherit', 'inherit'],
    shell: process.platform === 'win32',
  });
}

function findFile(root: string, fileName: string): string[] {
  const found: string[] = [];
  const walk = (dir: string) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (entry.name === fileName) found.push(full);
    }
  };
  if (fs.existsSync(root)) walk(root);
  return found;
}

/** Hardhat: runtime bytecode (`deployedBytecode`) as keccak256'd by the recorder (EXTCODEHASH). */
export const evmBuilder: Builder = {
  prepare(dir) {
    const evm = path.join(dir, 'evm');
    run('npm', ['ci', '--ignore-scripts'], evm);
    run('npx', ['hardhat', 'compile'], evm);
  },
  collect(dir, name) {
    if (!SAFE_NAME.test(name)) throw new Error(`unsafe contract name "${name}"`);
    const artifacts = path.join(dir, 'evm', 'artifacts', 'contracts');
    const matches = findFile(artifacts, `${name}.json`);
    if (matches.length !== 1) {
      throw new Error(`expected one Hardhat artifact for ${name}, found ${matches.length}`);
    }
    const artifact = JSON.parse(fs.readFileSync(matches[0], 'utf8')) as {
      sourceName: string;
      contractName: string;
      deployedBytecode: string;
    };
    const bytes = fromHex(artifact.deployedBytecode);

    // Immutables are zero in the compiled code and filled in at deployment, so the
    // compiler output is the only place that says where they are.
    const dbg = JSON.parse(fs.readFileSync(matches[0].replace(/\.json$/, '.dbg.json'), 'utf8')) as {
      buildInfo: string;
    };
    const buildInfo = JSON.parse(
      fs.readFileSync(path.resolve(path.dirname(matches[0]), dbg.buildInfo), 'utf8'),
    ) as {
      output: {
        contracts: Record<
          string,
          Record<
            string,
            {
              evm: {
                deployedBytecode: {
                  immutableReferences?: Record<string, Array<{ start: number; length: number }>>;
                };
              };
            }
          >
        >;
      };
    };
    const refs =
      buildInfo.output.contracts[artifact.sourceName]?.[artifact.contractName]?.evm.deployedBytecode
        .immutableReferences ?? {};
    const immutableRanges = Object.values(refs)
      .flat()
      .map(({ start, length }) => ({ start, length }));
    return { bytes, immutableRanges };
  },
};

/** Soroban: the optimized wasm, built exactly as stellar/deploy.sh builds it. */
export const stellarBuilder: Builder = {
  prepare(dir) {
    // The whole workspace, as deploy.sh does: building one crate alone can unify
    // dependency features differently and produce a different wasm.
    run(
      'cargo',
      ['build', '--target', 'wasm32-unknown-unknown', '--release', '--locked'],
      path.join(dir, 'stellar'),
    );
  },
  collect(dir, name) {
    if (!SAFE_NAME.test(name)) throw new Error(`unsafe contract name "${name}"`);
    const stellar = path.join(dir, 'stellar');
    const file = `${name.replace(/-/g, '_')}.wasm`;
    const raw = path.join(stellar, 'target', 'wasm32-unknown-unknown', 'release', file);
    const optimizedDir = path.join(stellar, 'target', 'optimized');
    fs.mkdirSync(optimizedDir, { recursive: true });
    const out = path.join(optimizedDir, file);
    run('stellar', ['contract', 'optimize', '--wasm', raw, '--wasm-out', out], stellar);
    return { bytes: fs.readFileSync(out) };
  },
};

/** Solana and CKB have no deterministic in-tree build here yet, so nothing can be source-verified. */
export const BUILDERS: Partial<Record<Chain, Builder>> = {
  evm: evmBuilder,
  stellar: stellarBuilder,
};

/* -------------------------------------------------------------------------- */
/* Reproduction                                                                */
/* -------------------------------------------------------------------------- */

export interface SessionDeps {
  repoRoot?: string;
  builders?: Partial<Record<Chain, Builder>>;
  /** Runtime code at an address, for contracts with immutables. */
  fetchCode?: (rpcUrl: string, address: string) => Promise<string>;
}

const defaultFetchCode = (rpcUrl: string, address: string) =>
  jsonRpc<string>(rpcUrl, 'eth_getCode', [address, 'latest']);

/**
 * Checks commits out into throwaway worktrees and builds each (chain, commit)
 * once, however many contracts are checked against it. Call dispose() when done.
 */
export class SourceSession {
  private repoRoot: string;
  private builders: Partial<Record<Chain, Builder>>;
  private fetchCode: (rpcUrl: string, address: string) => Promise<string>;
  private trees = new Map<string, string>();
  private prepared = new Map<string, Promise<void>>();

  constructor(deps: SessionDeps = {}) {
    this.repoRoot = deps.repoRoot ?? REPO_ROOT;
    this.builders = deps.builders ?? BUILDERS;
    this.fetchCode = deps.fetchCode ?? defaultFetchCode;
  }

  private checkout(commit: string): string {
    const existing = this.trees.get(commit);
    if (existing) return existing;
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'wraith-src-'));
    git(['worktree', 'add', '--detach', '--force', dir, commit], this.repoRoot);
    this.trees.set(commit, dir);
    return dir;
  }

  /**
   * null when building `contract.sourceCommit` reproduces `contract.artifactHash`,
   * otherwise a sentence saying why not.
   */
  async check(
    chain: Chain,
    contract: Pick<Contract, 'name' | 'id' | 'sourceCommit' | 'artifactHash'>,
    rpcUrl?: string,
  ): Promise<string | null> {
    const { sourceCommit, artifactHash } = contract;
    const problem = commitError(sourceCommit);
    if (problem) return `sourceCommit ${problem}`;
    const builder = this.builders[chain];
    if (!builder)
      return `no reproducible build exists for ${chain}, so its source cannot be verified`;

    let built: Built;
    try {
      const dir = this.checkout(sourceCommit!);
      const key = `${chain}:${sourceCommit}`;
      if (!this.prepared.has(key)) this.prepared.set(key, Promise.resolve(builder.prepare(dir)));
      await this.prepared.get(key);
      built = await builder.collect(dir, contract.name);
    } catch (err) {
      return `could not build ${sourceCommit} (${(err as Error).message.split('\n')[0]})`;
    }

    const hashOf = (bytes: Uint8Array) =>
      artifactHash.algorithm === 'keccak256'
        ? keccak256Hex(bytes)
        : artifactHash.algorithm === 'sha256'
          ? sha256Hex(bytes)
          : ckbHashHex(bytes);

    if (!built.immutableRanges?.length) {
      const actual = hashOf(built.bytes);
      return actual === artifactHash.value
        ? null
        : `building ${sourceCommit} gives ${artifactHash.algorithm} ${actual}, recorded ${artifactHash.value}`;
    }

    // The compiled code has holes where immutables go. It matches the deployment if
    // it is identical everywhere else and the deployed code is what was recorded.
    if (!rpcUrl) {
      return 'has immutables, so the deployed code is needed to compare against (no rpcUrl recorded)';
    }
    let deployed: Uint8Array;
    try {
      deployed = fromHex(await this.fetchCode(rpcUrl, contract.id));
    } catch (err) {
      return `could not read deployed code (${(err as Error).message})`;
    }
    if (hashOf(deployed) !== artifactHash.value) {
      return `deployed code hashes to ${hashOf(deployed)}, recorded ${artifactHash.value}`;
    }
    if (deployed.length !== built.bytes.length) {
      return `building ${sourceCommit} gives ${built.bytes.length} bytes of code, deployed ${deployed.length}`;
    }
    const masked = new Uint8Array(deployed.length);
    for (const { start, length } of built.immutableRanges) masked.fill(1, start, start + length);
    for (let i = 0; i < deployed.length; i++) {
      if (!masked[i] && deployed[i] !== built.bytes[i]) {
        return `building ${sourceCommit} differs from the deployed code at byte ${i} (outside immutables)`;
      }
    }
    return null;
  }

  dispose(): void {
    for (const dir of this.trees.values()) {
      try {
        git(['worktree', 'remove', '--force', dir], this.repoRoot);
      } catch {
        fs.rmSync(dir, { recursive: true, force: true });
      }
    }
    try {
      git(['worktree', 'prune'], this.repoRoot);
    } catch {
      /* best effort */
    }
    this.trees.clear();
    this.prepared.clear();
  }
}
