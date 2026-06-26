/**
 * setup-multisig.test.ts
 *
 * Unit tests for the multisig setup script logic.
 *
 * For futurenet integration tests, set FUTURENET_ACCOUNT and FUTURENET_IDENTITY
 * env vars before running, or they will be skipped.
 *
 * Run: npx vitest run scripts/setup-multisig.test.ts
 */

import { describe, it, expect } from 'vitest';
import { execSync } from 'child_process';
import * as path from 'path';

const SCRIPT = path.resolve(__dirname, 'setup-multisig.sh');

// Valid G-address fixtures (56-char, base32 alphabet A-Z2-7)
const VALID_SIGNERS = [
  'GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',
  'GCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC',
  'GDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD',
  'GEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE',
  'GFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF',
];
const VALID_ACCOUNT = 'GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA';

function runScript(args: string): { stdout: string; stderr: string; code: number } {
  try {
    const stdout = execSync(`bash "${SCRIPT}" ${args}`, {
      encoding: 'utf-8',
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    return { stdout, stderr: '', code: 0 };
  } catch (e: any) {
    return { stdout: e.stdout ?? '', stderr: e.stderr ?? '', code: e.status ?? 1 };
  }
}

describe('setup-multisig.sh argument validation', () => {
  it('exits non-zero with no arguments', () => {
    const r = runScript('');
    expect(r.code).not.toBe(0);
    expect(r.stderr).toMatch(/--network is required/);
  });

  it('exits non-zero when --account is missing', () => {
    const r = runScript(`--network futurenet --signers "${VALID_SIGNERS[0]}" --threshold 1`);
    expect(r.code).not.toBe(0);
    expect(r.stderr).toMatch(/--account is required/);
  });

  it('exits non-zero when --signers is missing', () => {
    const r = runScript(`--network futurenet --account ${VALID_ACCOUNT} --threshold 1`);
    expect(r.code).not.toBe(0);
    expect(r.stderr).toMatch(/--signers is required/);
  });

  it('exits non-zero when --threshold is missing', () => {
    const r = runScript(
      `--network futurenet --account ${VALID_ACCOUNT} --signers "${VALID_SIGNERS[0]}"`,
    );
    expect(r.code).not.toBe(0);
    expect(r.stderr).toMatch(/--threshold is required/);
  });

  it('rejects unknown network', () => {
    const r = runScript(
      `--network devnet --account ${VALID_ACCOUNT} --signers "${VALID_SIGNERS[0]}" --threshold 1`,
    );
    expect(r.code).not.toBe(0);
    expect(r.stderr).toMatch(/Unknown network/);
  });

  it('rejects invalid signer address', () => {
    const r = runScript(
      `--network futurenet --account ${VALID_ACCOUNT} --signers "INVALID" --threshold 1 --dry-run`,
    );
    expect(r.code).not.toBe(0);
    expect(r.stderr).toMatch(/Invalid signer address/);
  });

  it('rejects invalid account address', () => {
    const r = runScript(
      `--network futurenet --account NOTVALID --signers "${VALID_SIGNERS[0]}" --threshold 1 --dry-run`,
    );
    expect(r.code).not.toBe(0);
    expect(r.stderr).toMatch(/Invalid --account address/);
  });

  it('rejects threshold greater than number of signers', () => {
    const r = runScript(
      `--network futurenet --account ${VALID_ACCOUNT} --signers "${VALID_SIGNERS[0]},${VALID_SIGNERS[1]}" --threshold 5 --dry-run`,
    );
    expect(r.code).not.toBe(0);
    expect(r.stderr).toMatch(/Threshold.*cannot exceed/);
  });

  it('rejects threshold of 0', () => {
    const r = runScript(
      `--network futurenet --account ${VALID_ACCOUNT} --signers "${VALID_SIGNERS[0]}" --threshold 0 --dry-run`,
    );
    expect(r.code).not.toBe(0);
    expect(r.stderr).toMatch(/must be a positive integer/);
  });

  it('rejects non-integer threshold', () => {
    const r = runScript(
      `--network futurenet --account ${VALID_ACCOUNT} --signers "${VALID_SIGNERS[0]}" --threshold 1.5 --dry-run`,
    );
    expect(r.code).not.toBe(0);
    expect(r.stderr).toMatch(/must be a positive integer/);
  });
});

describe('setup-multisig.sh dry-run output', () => {
  const signerList = VALID_SIGNERS.slice(0, 5).join(',');

  it('exits 0 in dry-run mode', () => {
    const r = runScript(
      `--network futurenet --account ${VALID_ACCOUNT} --signers "${signerList}" --threshold 3 --dry-run`,
    );
    expect(r.code).toBe(0);
  });

  it('prints plan with correct threshold', () => {
    const r = runScript(
      `--network futurenet --account ${VALID_ACCOUNT} --signers "${signerList}" --threshold 3 --dry-run`,
    );
    expect(r.stdout).toMatch(/Threshold:\s+3-of-5/);
  });

  it('lists all signers in plan output', () => {
    const r = runScript(
      `--network futurenet --account ${VALID_ACCOUNT} --signers "${signerList}" --threshold 3 --dry-run`,
    );
    for (const s of VALID_SIGNERS.slice(0, 5)) {
      expect(r.stdout).toContain(s);
    }
  });

  it('shows master weight 0 in plan', () => {
    const r = runScript(
      `--network futurenet --account ${VALID_ACCOUNT} --signers "${signerList}" --threshold 3 --dry-run`,
    );
    expect(r.stdout).toMatch(/Master weight:\s+0/);
  });

  it('shows DRY-RUN notice and no submission', () => {
    const r = runScript(
      `--network futurenet --account ${VALID_ACCOUNT} --signers "${signerList}" --threshold 3 --dry-run`,
    );
    expect(r.stdout).toMatch(/DRY-RUN.*No transactions submitted/);
  });

  it('handles single signer threshold 1-of-1', () => {
    const r = runScript(
      `--network futurenet --account ${VALID_ACCOUNT} --signers "${VALID_SIGNERS[0]}" --threshold 1 --dry-run`,
    );
    expect(r.code).toBe(0);
    expect(r.stdout).toMatch(/Threshold:\s+1-of-1/);
  });

  it('accepts all three supported networks', () => {
    for (const net of ['testnet', 'futurenet', 'mainnet']) {
      const r = runScript(
        `--network ${net} --account ${VALID_ACCOUNT} --signers "${signerList}" --threshold 3 --dry-run`,
      );
      expect(r.code).toBe(0);
    }
  });

  it('strips whitespace from signer addresses', () => {
    const spacedSigners = VALID_SIGNERS.slice(0, 2).join(', ');
    const r = runScript(
      `--network futurenet --account ${VALID_ACCOUNT} --signers "${spacedSigners}" --threshold 1 --dry-run`,
    );
    expect(r.code).toBe(0);
  });

  it('writes to log file', () => {
    const logFile = `/tmp/multisig-test-${Date.now()}.log`;
    const r = runScript(
      `--network futurenet --account ${VALID_ACCOUNT} --signers "${signerList}" --threshold 3 --dry-run --log-file "${logFile}"`,
    );
    expect(r.code).toBe(0);
    const { readFileSync, existsSync } = require('fs');
    expect(existsSync(logFile)).toBe(true);
    const logContent = readFileSync(logFile, 'utf-8');
    expect(logContent).toMatch(/Multisig Setup Plan/);
    // cleanup
    require('fs').unlinkSync(logFile);
  });
});

describe('setup-multisig.sh futurenet integration', () => {
  const FUTURENET_ACCOUNT = process.env['FUTURENET_ACCOUNT'];
  const itIf = (cond: boolean) => (cond ? it : it.skip);

  itIf(!!FUTURENET_ACCOUNT)('dry-run against real futurenet account address', () => {
    const r = runScript(
      `--network futurenet --account ${FUTURENET_ACCOUNT} --signers "${VALID_SIGNERS.slice(0, 3).join(',')}" --threshold 2 --dry-run`,
    );
    expect(r.code).toBe(0);
    expect(r.stdout).toMatch(/Threshold:\s+2-of-3/);
  });
});
