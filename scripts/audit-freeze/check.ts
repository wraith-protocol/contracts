#!/usr/bin/env -S npx tsx
/**
 * audit-freeze check
 *
 * Reads freeze_paths / freeze_until from audit-prep/ENGAGEMENT.md and fails
 * (exit 1) if the current pull request touches a frozen path without
 * carrying the "audit-approved" label.
 *
 * SECURITY NOTE: ENGAGEMENT.md is read from the pull request's BASE ref
 * (the target branch, e.g. `develop`), not its head ref (the PR's own
 * branch). This is deliberate: if we read the file from the head ref, a PR
 * could edit ENGAGEMENT.md to shorten or remove its own freeze window and
 * then sail through the same check. Reading from the base ref means the
 * freeze window is only ever whatever is already merged -- a PR cannot
 * change the rules it is itself being judged against. See decide.ts for the
 * second, independent layer of protection (ENGAGEMENT.md is always treated
 * as a frozen path in its own right while a freeze is active).
 *
 * This script makes plain REST calls to the GitHub API via fetch and has no
 * npm dependencies, so the workflow can run it with `npx tsx` right after
 * `pnpm install` at the repo root -- no separate install step for this
 * package is needed in CI.
 */

import { readFileSync } from 'node:fs';
import { ENGAGEMENT_DOC_PATH, decide } from './decide.js';
import { parseFrontMatter } from './parse.js';

interface CliOptions {
  repo: string;
  prNumber: number;
  dryRun: boolean;
}

function parseArgs(argv: string[]): CliOptions {
  const args = argv.slice(2);
  const dryRun = args.includes('--dry-run');

  const prFlagIndex = args.indexOf('--pr');
  const prFromFlag = prFlagIndex !== -1 ? Number(args[prFlagIndex + 1]) : null;

  const repoFlagIndex = args.indexOf('--repo');
  const repoFromFlag = repoFlagIndex !== -1 ? args[repoFlagIndex + 1] : null;

  const eventPrNumber = readPrNumberFromEvent();

  const prNumber = prFromFlag ?? eventPrNumber;
  if (!prNumber) {
    throw new Error(
      'Could not determine a PR number. Pass --pr <number>, or run inside a pull_request workflow event.',
    );
  }

  const repo = repoFromFlag ?? process.env.GITHUB_REPOSITORY;
  if (!repo) {
    throw new Error(
      'Could not determine the repo. Pass --repo owner/name, or set GITHUB_REPOSITORY.',
    );
  }

  return { repo, prNumber, dryRun };
}

function readPrNumberFromEvent(): number | null {
  const eventPath = process.env.GITHUB_EVENT_PATH;
  if (!eventPath) return null;
  try {
    const event = JSON.parse(readFileSync(eventPath, 'utf8'));
    return event.pull_request?.number ?? event.number ?? null;
  } catch {
    return null;
  }
}

const GITHUB_API_URL = process.env.GITHUB_API_URL ?? 'https://api.github.com';

async function githubApiFetch(path: string, token: string | undefined): Promise<Response> {
  const headers: Record<string, string> = {
    Accept: 'application/vnd.github+json',
    'X-GitHub-Api-Version': '2022-11-28',
  };
  if (token) headers.Authorization = `Bearer ${token}`;

  return fetch(`${GITHUB_API_URL}${path}`, { headers });
}

async function fetchPullRequest(repo: string, prNumber: number, token: string | undefined) {
  const res = await githubApiFetch(`/repos/${repo}/pulls/${prNumber}`, token);
  if (!res.ok) {
    throw new Error(`Failed to fetch PR #${prNumber}: ${res.status} ${res.statusText}`);
  }
  return res.json() as Promise<{
    base: { sha: string };
    labels: Array<{ name: string }>;
  }>;
}

async function fetchChangedFiles(
  repo: string,
  prNumber: number,
  token: string | undefined,
): Promise<string[]> {
  const files: string[] = [];
  let page = 1;
  for (;;) {
    const res = await githubApiFetch(
      `/repos/${repo}/pulls/${prNumber}/files?per_page=100&page=${page}`,
      token,
    );
    if (!res.ok) {
      throw new Error(
        `Failed to list changed files for PR #${prNumber}: ${res.status} ${res.statusText}`,
      );
    }
    const batch = (await res.json()) as Array<{ filename: string }>;
    files.push(...batch.map((f) => f.filename));
    if (batch.length < 100) break;
    page += 1;
  }
  return files;
}

async function fetchEngagementDocAtRef(
  repo: string,
  ref: string,
  token: string | undefined,
): Promise<string> {
  const res = await githubApiFetch(
    `/repos/${repo}/contents/${ENGAGEMENT_DOC_PATH}?ref=${ref}`,
    token,
  );
  if (res.status === 404) {
    // No ENGAGEMENT.md yet at the base ref -- treat as "no freeze".
    return '';
  }
  if (!res.ok) {
    throw new Error(
      `Failed to fetch ${ENGAGEMENT_DOC_PATH}@${ref}: ${res.status} ${res.statusText}`,
    );
  }
  const body = (await res.json()) as { content: string; encoding: string };
  if (body.encoding !== 'base64') {
    throw new Error(`Unexpected encoding "${body.encoding}" for ${ENGAGEMENT_DOC_PATH}`);
  }
  return Buffer.from(body.content, 'base64').toString('utf8');
}

async function main() {
  const { repo, prNumber, dryRun } = parseArgs(process.argv);
  const token = process.env.GITHUB_TOKEN;

  const pr = await fetchPullRequest(repo, prNumber, token);
  const baseSha = pr.base.sha;
  const hasApprovalLabel = pr.labels.some((label) => label.name === 'audit-approved');

  const [engagementDocContent, changedFiles] = await Promise.all([
    fetchEngagementDocAtRef(repo, baseSha, token),
    fetchChangedFiles(repo, prNumber, token),
  ]);

  const frontMatter = parseFrontMatter(engagementDocContent);

  const result = decide({
    frontMatter,
    changedFiles,
    hasApprovalLabel,
    now: new Date(),
  });

  console.log(`audit-freeze: ${result.reason}`);
  if (result.offendingFiles.length > 0) {
    console.log('Frozen path(s) touched:');
    for (const file of result.offendingFiles) {
      console.log(`  - ${file}`);
    }
  }

  if (result.blocked) {
    console.log(
      `\nTo proceed anyway (e.g. an audit-team-approved exception), add the "audit-approved" label to this PR and re-run.`,
    );
    if (dryRun) {
      console.log('\n[--dry-run] Not failing the process, but this PR WOULD be blocked.');
      return;
    }
    process.exitCode = 1;
    return;
  }

  console.log('audit-freeze: OK.');
}

main().catch((err) => {
  console.error(err);
  process.exitCode = 1;
});
