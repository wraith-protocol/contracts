import { createServer, type Server } from 'node:http';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

const execFileAsync = promisify(execFile);

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const checkScript = path.join(__dirname, '..', 'check.ts');

let server: Server;
let port: number;

interface MockRoutes {
  pr: { base: { sha: string }; labels: Array<{ name: string }> };
  files: Array<{ filename: string }>;
  engagementMdAtBaseSha: string | null; // null => 404 (file doesn't exist yet)
}

function startMockGitHubApi(repo: string, prNumber: number, routes: MockRoutes): Promise<number> {
  return new Promise((resolve) => {
    server = createServer((req, res) => {
      const url = new URL(req.url ?? '/', 'http://localhost');
      const p = url.pathname;

      res.setHeader('Content-Type', 'application/json');

      if (p === `/repos/${repo}/pulls/${prNumber}`) {
        res.writeHead(200);
        res.end(JSON.stringify(routes.pr));
        return;
      }

      if (p === `/repos/${repo}/pulls/${prNumber}/files`) {
        const page = Number(url.searchParams.get('page') ?? '1');
        res.writeHead(200);
        res.end(JSON.stringify(page === 1 ? routes.files : []));
        return;
      }

      if (p === `/repos/${repo}/contents/audit-prep/ENGAGEMENT.md`) {
        if (routes.engagementMdAtBaseSha === null) {
          res.writeHead(404);
          res.end(JSON.stringify({ message: 'Not Found' }));
          return;
        }
        res.writeHead(200);
        res.end(
          JSON.stringify({
            content: Buffer.from(routes.engagementMdAtBaseSha).toString('base64'),
            encoding: 'base64',
          }),
        );
        return;
      }

      res.writeHead(404);
      res.end(JSON.stringify({ message: 'not found (test server)' }));
    });

    server.listen(0, () => {
      const address = server.address();
      resolve(typeof address === 'object' && address ? address.port : 0);
    });
  });
}

async function runCheck(
  env: Record<string, string>,
): Promise<{ status: number; stdout: string; stderr: string }> {
  try {
    const { stdout } = await execFileAsync(
      'npx',
      ['tsx', checkScript, '--repo', 'test/repo', '--pr', '42'],
      { env: { ...process.env, ...env }, encoding: 'utf8', timeout: 15000, shell: true },
    );
    return { status: 0, stdout, stderr: '' };
  } catch (err) {
    const e = err as { code: number | null; stdout: string; stderr: string; signal?: string };
    if (e.signal) {
      throw new Error(`check.ts subprocess was killed by signal ${e.signal} (likely timed out)`);
    }
    return { status: e.code ?? 1, stdout: e.stdout, stderr: e.stderr };
  }
}

afterEach(() => {
  server?.closeAllConnections?.();
  server?.close();
});

describe('check.ts end-to-end (against a local mock GitHub API)', () => {
  it('exits 1 and reports the offending file when a frozen path is touched without the label', async () => {
    port = await startMockGitHubApi('test/repo', 42, {
      pr: { base: { sha: 'base-sha' }, labels: [] },
      files: [{ filename: 'stellar/stealth-announcer/src/lib.rs' }, { filename: 'README.md' }],
      engagementMdAtBaseSha: `---
freeze_paths:
  - "stellar/stealth-announcer/**"
freeze_until: "2099-01-01T00:00:00Z"
---
`,
    });

    const result = await runCheck({ GITHUB_API_URL: `http://localhost:${port}` });

    expect(result.status).toBe(1);
    expect(result.stdout).toContain('stellar/stealth-announcer/src/lib.rs');
    expect(result.stdout).toContain('audit-approved');
  }, 15000);

  it('exits 0 when the PR carries the audit-approved label', async () => {
    port = await startMockGitHubApi('test/repo', 42, {
      pr: { base: { sha: 'base-sha' }, labels: [{ name: 'audit-approved' }] },
      files: [{ filename: 'stellar/stealth-announcer/src/lib.rs' }],
      engagementMdAtBaseSha: `---
freeze_paths:
  - "stellar/stealth-announcer/**"
freeze_until: "2099-01-01T00:00:00Z"
---
`,
    });

    const result = await runCheck({ GITHUB_API_URL: `http://localhost:${port}` });

    expect(result.status).toBe(0);
    expect(result.stdout).toContain('OK');
  }, 15000);

  it('exits 0 when there is no active freeze (freeze_until is TBD)', async () => {
    port = await startMockGitHubApi('test/repo', 42, {
      pr: { base: { sha: 'base-sha' }, labels: [] },
      files: [{ filename: 'stellar/stealth-announcer/src/lib.rs' }],
      engagementMdAtBaseSha: `---
freeze_paths:
  - "stellar/stealth-announcer/**"
freeze_until: "TBD"
---
`,
    });

    const result = await runCheck({ GITHUB_API_URL: `http://localhost:${port}` });

    expect(result.status).toBe(0);
  }, 15000);

  it('exits 0 when ENGAGEMENT.md does not exist yet at the base ref (404)', async () => {
    port = await startMockGitHubApi('test/repo', 42, {
      pr: { base: { sha: 'base-sha' }, labels: [] },
      files: [{ filename: 'stellar/stealth-announcer/src/lib.rs' }],
      engagementMdAtBaseSha: null,
    });

    const result = await runCheck({ GITHUB_API_URL: `http://localhost:${port}` });

    expect(result.status).toBe(0);
  }, 15000);

  it('blocks an edit to ENGAGEMENT.md itself while a freeze is active, even without it in freeze_paths', async () => {
    port = await startMockGitHubApi('test/repo', 42, {
      pr: { base: { sha: 'base-sha' }, labels: [] },
      files: [{ filename: 'audit-prep/ENGAGEMENT.md' }],
      engagementMdAtBaseSha: `---
freeze_paths:
  - "stellar/stealth-announcer/**"
freeze_until: "2099-01-01T00:00:00Z"
---
`,
    });

    const result = await runCheck({ GITHUB_API_URL: `http://localhost:${port}` });

    expect(result.status).toBe(1);
    expect(result.stdout).toContain('audit-prep/ENGAGEMENT.md');
  }, 15000);
});
