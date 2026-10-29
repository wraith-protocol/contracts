import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { keccak256Hex } from '../src/hash.js';
import { validateDeployment, type Deployment } from '../src/manifest.js';
import {
  SourceSession,
  candidateCommits,
  dirtyFiles,
  resolveSourceCommit,
  type Builder,
} from '../src/provenance.js';

const git = (cwd: string, ...args: string[]) =>
  execFileSync('git', args, { cwd, encoding: 'utf8' }).trim();

const bytesOf = (text: string) => new TextEncoder().encode(text);
const hashOf = (text: string) => keccak256Hex(bytesOf(text));

/** "Builds" by reading artifact.txt from the checkout. */
const fileBuilder: Builder = {
  prepare() {},
  collect: (dir) => ({ bytes: fs.readFileSync(path.join(dir, 'artifact.txt')) }),
};

let repo: string;
let first: string;
let second: string;

beforeEach(() => {
  repo = fs.mkdtempSync(path.join(os.tmpdir(), 'wraith-prov-'));
  git(repo, 'init', '-q');
  git(repo, 'config', 'user.email', 't@example.com');
  git(repo, 'config', 'user.name', 't');
  fs.mkdirSync(path.join(repo, 'evm', 'contracts'), { recursive: true });
  const commit = (text: string) => {
    fs.writeFileSync(path.join(repo, 'artifact.txt'), text);
    fs.writeFileSync(path.join(repo, 'evm', 'contracts', 'A.sol'), text);
    git(repo, 'add', '-A');
    git(repo, 'commit', '-qm', text);
    return git(repo, 'rev-parse', 'HEAD');
  };
  first = commit('one');
  second = commit('two');
});

afterEach(() => fs.rmSync(repo, { recursive: true, force: true }));

const contract = (commit: string | null, text: string) => ({
  name: 'A',
  id: '0x8AE65c05E7eb48B9bA652781Bc0a3DBA09A484F3',
  sourceCommit: commit,
  artifactHash: { algorithm: 'keccak256' as const, value: hashOf(text) },
});

describe('resolveSourceCommit', () => {
  it('resolves HEAD and accepts a full hash', () => {
    expect(resolveSourceCommit('HEAD', repo)).toBe(second);
    expect(resolveSourceCommit(first.toUpperCase(), repo)).toBe(first);
  });

  it.each(['', 'main', 'abc1234', 'z'.repeat(40), '0'.repeat(40)])('rejects %j', (bad) => {
    expect(() => resolveSourceCommit(bad, repo)).toThrow(/--source-commit/);
  });

  it('rejects a well-formed hash that is not in the repository', () => {
    expect(() => resolveSourceCommit('b'.repeat(40), repo)).toThrow(/not a commit/);
  });
});

describe('dirtyFiles', () => {
  it('reports uncommitted changes to tracked files only', () => {
    expect(dirtyFiles(repo)).toEqual([]);
    fs.writeFileSync(path.join(repo, 'untracked.txt'), 'x');
    expect(dirtyFiles(repo)).toEqual([]);
    fs.writeFileSync(path.join(repo, 'artifact.txt'), 'changed');
    expect(dirtyFiles(repo).join()).toMatch(/artifact\.txt/);
  });
});

describe('SourceSession.check', () => {
  const session = () => new SourceSession({ repoRoot: repo, builders: { evm: fileBuilder } });

  it('accepts a commit whose build reproduces the artifact hash', async () => {
    const s = session();
    try {
      expect(await s.check('evm', contract(first, 'one'))).toBeNull();
      expect(await s.check('evm', contract(second, 'two'))).toBeNull();
    } finally {
      s.dispose();
    }
  });

  it('rejects a commit whose build differs', async () => {
    const s = session();
    try {
      expect(await s.check('evm', contract(first, 'two'))).toMatch(/gives keccak256 0x/);
    } finally {
      s.dispose();
    }
  });

  it('rejects null, malformed and unknown commits', async () => {
    const s = session();
    try {
      expect(await s.check('evm', contract(null, 'one'))).toMatch(/sourceCommit/);
      expect(await s.check('evm', contract('abc', 'one'))).toMatch(/sourceCommit/);
      expect(await s.check('evm', contract('b'.repeat(40), 'one'))).toMatch(/could not build/);
    } finally {
      s.dispose();
    }
  });

  it('cannot verify a chain with no reproducible build', async () => {
    const s = session();
    try {
      expect(await s.check('solana', contract(first, 'one'))).toMatch(/no reproducible build/);
    } finally {
      s.dispose();
    }
  });

  it('leaves no worktrees behind', async () => {
    const s = session();
    await s.check('evm', contract(first, 'one'));
    s.dispose();
    expect(git(repo, 'worktree', 'list').split('\n')).toHaveLength(1);
  });

  it('compares around immutables against the deployed code', async () => {
    const compiled = new Uint8Array([1, 2, 0, 0, 5]);
    const withImmutables: Builder = {
      prepare() {},
      collect: () => ({ bytes: compiled, immutableRanges: [{ start: 2, length: 2 }] }),
    };
    const deployed = (bytes: number[]) => async () => `0x${Buffer.from(bytes).toString('hex')}`;
    const c = (bytes: number[]) => ({
      ...contract(first, ''),
      artifactHash: {
        algorithm: 'keccak256' as const,
        value: keccak256Hex(Uint8Array.from(bytes)),
      },
    });
    const make = (bytes: number[]) =>
      new SourceSession({
        repoRoot: repo,
        builders: { evm: withImmutables },
        fetchCode: deployed(bytes),
      });

    const ok = make([1, 2, 9, 9, 5]);
    expect(await ok.check('evm', c([1, 2, 9, 9, 5]), 'https://rpc')).toBeNull();
    ok.dispose();

    const bad = make([1, 3, 9, 9, 5]);
    expect(await bad.check('evm', c([1, 3, 9, 9, 5]), 'https://rpc')).toMatch(/byte 1/);
    bad.dispose();

    const noRpc = make([1, 2, 9, 9, 5]);
    expect(await noRpc.check('evm', c([1, 2, 9, 9, 5]))).toMatch(/no rpcUrl/);
    noRpc.dispose();
  });
});

describe('candidateCommits', () => {
  it('lists commits that touched the chain sources, newest first', () => {
    expect(candidateCommits('evm', null, 10, repo)).toEqual([second, first]);
    expect(candidateCommits('evm', null, 1, repo)).toEqual([second]);
  });
});

describe('provenance validation', () => {
  const base = (): Deployment => ({
    chain: 'evm',
    network: 'test-net',
    networkId: '1',
    recordedBy: 'chain-query',
    contracts: [
      {
        ...contract(null, 'one'),
        version: null,
        provenance: { status: 'unverifiable', reason: 'historical' },
        deployment: { height: 1 },
      },
    ],
  });

  it('accepts an unverifiable historical record and a source-verified one', () => {
    expect(validateDeployment(base())).toEqual([]);
    const d = base();
    d.contracts[0].sourceCommit = 'a'.repeat(40);
    d.contracts[0].provenance = { status: 'source-verified' };
    expect(validateDeployment(d)).toEqual([]);
  });

  it('requires provenance on first-party contracts', () => {
    const d = base();
    delete d.contracts[0].provenance;
    expect(validateDeployment(d).join()).toMatch(/provenance is required/);
  });

  it('rejects contradictions and malformed commits', () => {
    const a = base();
    a.contracts[0].provenance = { status: 'source-verified' };
    expect(validateDeployment(a).join()).toMatch(/requires a sourceCommit/);

    const b = base();
    b.contracts[0].sourceCommit = 'a'.repeat(40);
    expect(validateDeployment(b).join()).toMatch(/contradicts/);

    const c = base();
    c.contracts[0].provenance = { status: 'unverifiable' };
    expect(validateDeployment(c).join()).toMatch(/requires a reason/);

    for (const bad of ['abc', 'A'.repeat(40), '0'.repeat(40)]) {
      const d = base();
      d.contracts[0].sourceCommit = bad;
      d.contracts[0].provenance = { status: 'source-verified' };
      expect(validateDeployment(d).length).toBeGreaterThan(0);
    }
  });

  it('never lets a deploy script write an unverifiable or null-commit record', () => {
    const d = { ...base(), recordedBy: 'deploy-script' as const };
    expect(validateDeployment(d).join()).toMatch(/must carry sourceCommit/);
    expect(validateDeployment(d).join()).toMatch(/must be source-verified/);
  });

  it('exempts external contracts', () => {
    const d = base();
    d.contracts[0].external = true;
    delete d.contracts[0].provenance;
    expect(validateDeployment(d)).toEqual([]);
  });
});
