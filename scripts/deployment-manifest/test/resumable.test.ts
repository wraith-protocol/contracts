import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { DeploymentJournalManager } from '../src/journal.js';
import { sha256Hex } from '../src/hash.js';

describe('DeploymentJournalManager', () => {
  let tmpDir: string;
  let journalFile: string;
  let fakeWasm1: string;
  let fakeWasm2: string;

  beforeEach(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'journal-test-'));
    journalFile = path.join(tmpDir, 'deployment-journal.json');
    fakeWasm1 = path.join(tmpDir, 'stealth_announcer.wasm');
    fakeWasm2 = path.join(tmpDir, 'stealth_registry.wasm');
    fs.writeFileSync(fakeWasm1, 'fake-announcer-wasm-binary-content');
    fs.writeFileSync(fakeWasm2, 'fake-registry-wasm-binary-content');
  });

  afterEach(() => {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  });

  it('initializes a fresh journal with in_progress status', () => {
    const manager = new DeploymentJournalManager(journalFile);
    expect(manager.exists()).toBe(false);

    const journal = manager.initialize(
      'stellar-testnet',
      'Test SDF Network ; September 2015',
      '0123456789abcdef0123456789abcdef01234567',
      'GBBD47IF6LWK7P7MDEV',
    );

    expect(manager.exists()).toBe(true);
    expect(journal.status).toBe('in_progress');
    expect(journal.network).toBe('stellar-testnet');
    expect(journal.deployer).toBe('GBBD47IF6LWK7P7MDEV');
    expect(Object.keys(journal.contracts)).toHaveLength(0);
  });

  it('records deployment steps incrementally', () => {
    const manager = new DeploymentJournalManager(journalFile);
    manager.initialize(
      'stellar-testnet',
      'Test SDF Network ; September 2015',
      '0123456789abcdef0123456789abcdef01234567',
      'GBBD47IF6LWK7P7MDEV',
    );

    const wasmHash1 = sha256Hex(fs.readFileSync(fakeWasm1));
    manager.recordContractStep(
      'stealth-announcer',
      'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABBB4',
      fakeWasm1,
      100,
      true,
    );

    const j1 = manager.read();
    expect(j1.contracts['stealth-announcer']).toBeDefined();
    expect(j1.contracts['stealth-announcer'].contractId).toBe(
      'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABBB4',
    );
    expect(j1.contracts['stealth-announcer'].wasmHash).toBe(wasmHash1);
    expect(j1.contracts['stealth-announcer'].verified).toBe(true);

    const plan = manager.plan(['stealth-announcer', 'stealth-registry']);
    expect(plan).toEqual([
      {
        contract: 'stealth-announcer',
        action: 'reuse',
        reason: 'Contract already deployed and verified at CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABBB4',
        existingId: 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABBB4',
      },
      {
        contract: 'stealth-registry',
        action: 'deploy',
        reason: 'Contract not yet deployed in this journal session',
      },
    ]);
  });

  it('rejects resume when source commit or network mismatches (fail-closed)', async () => {
    const manager = new DeploymentJournalManager(journalFile);
    manager.initialize(
      'stellar-testnet',
      'Test SDF Network ; September 2015',
      '0123456789abcdef0123456789abcdef01234567',
      'GBBD47IF6LWK7P7MDEV',
    );

    manager.recordContractStep(
      'stealth-announcer',
      'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABBB4',
      fakeWasm1,
      100,
      true,
    );

    const mismatchCommit = await manager.validateResume(
      'stellar-testnet',
      'Test SDF Network ; September 2015',
      'ffffffffffffffffffffffffffffffffffffffff',
      'GBBD47IF6LWK7P7MDEV',
      { 'stealth-announcer': fakeWasm1 },
    );
    expect(mismatchCommit.valid).toBe(false);
    expect(mismatchCommit.mismatches.some((m) => m.includes('Source commit mismatch'))).toBe(true);

    const mismatchNetwork = await manager.validateResume(
      'stellar-futurenet',
      'Test SDF Future Network ; October 2022',
      '0123456789abcdef0123456789abcdef01234567',
      'GBBD47IF6LWK7P7MDEV',
      { 'stealth-announcer': fakeWasm1 },
    );
    expect(mismatchNetwork.valid).toBe(false);
    expect(mismatchNetwork.mismatches.some((m) => m.includes('Network mismatch'))).toBe(true);

    const mismatchDeployer = await manager.validateResume(
      'stellar-testnet',
      'Test SDF Network ; September 2015',
      '0123456789abcdef0123456789abcdef01234567',
      'GDIFFERENTDEPLOYER',
      { 'stealth-announcer': fakeWasm1 },
    );
    expect(mismatchDeployer.valid).toBe(false);
    expect(mismatchDeployer.mismatches.some((m) => m.includes('Deployer authority mismatch'))).toBe(true);
  });

  it('rejects resume when artifact hash has drifted (bytecode mismatch)', async () => {
    const manager = new DeploymentJournalManager(journalFile);
    manager.initialize(
      'stellar-testnet',
      'Test SDF Network ; September 2015',
      '0123456789abcdef0123456789abcdef01234567',
      'GBBD47IF6LWK7P7MDEV',
    );

    manager.recordContractStep(
      'stealth-announcer',
      'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABBB4',
      fakeWasm1,
      100,
      true,
    );

    fs.writeFileSync(fakeWasm1, 'modified-tampered-wasm-binary');

    const mismatchWasm = await manager.validateResume(
      'stellar-testnet',
      'Test SDF Network ; September 2015',
      '0123456789abcdef0123456789abcdef01234567',
      'GBBD47IF6LWK7P7MDEV',
      { 'stealth-announcer': fakeWasm1 },
    );
    expect(mismatchWasm.valid).toBe(false);
    expect(mismatchWasm.mismatches.some((m) => m.includes('Artifact hash mismatch'))).toBe(true);
  });

  it('marks deployment completed cleanly upon success', () => {
    const manager = new DeploymentJournalManager(journalFile);
    manager.initialize(
      'stellar-testnet',
      'Test SDF Network ; September 2015',
      '0123456789abcdef0123456789abcdef01234567',
      'GBBD47IF6LWK7P7MDEV',
    );

    manager.recordContractStep('stealth-announcer', 'CID1', fakeWasm1, 100, true);
    manager.recordContractStep('stealth-registry', 'CID2', fakeWasm2, 101, true);
    manager.markComplete();

    const j = manager.read();
    expect(j.status).toBe('completed');
    expect(j.completedAt).toBeDefined();
  });
});
