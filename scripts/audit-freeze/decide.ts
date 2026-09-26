import { matchesAnyGlob } from './glob.js';
import type { EngagementFrontMatter } from './parse.js';

export const ENGAGEMENT_DOC_PATH = 'audit-prep/ENGAGEMENT.md';
export const APPROVAL_LABEL = 'audit-approved';

export interface DecideInput {
  frontMatter: EngagementFrontMatter;
  changedFiles: readonly string[];
  hasApprovalLabel: boolean;
  now: Date;
}

export interface DecideResult {
  blocked: boolean;
  active: boolean;
  offendingFiles: string[];
  reason: string;
}

/**
 * Decides whether a PR should be blocked by the audit freeze.
 *
 * Defense in depth: whenever a freeze is active, `audit-prep/ENGAGEMENT.md`
 * itself is always treated as a frozen path -- in addition to whatever
 * `freeze_paths` lists -- so a PR can't loosen or shorten its own freeze
 * window to sneak changes past the gate. (The primary safeguard is that the
 * caller reads this file's content from the PR's base ref, not its head
 * ref -- see check.ts / audit-freeze.yml -- but this is a second,
 * independent layer that holds even if that ever regresses.)
 */
export function decide(input: DecideInput): DecideResult {
  const { frontMatter, changedFiles, hasApprovalLabel, now } = input;

  const active = frontMatter.freezeUntil !== null && now < frontMatter.freezeUntil;

  if (!active) {
    return { blocked: false, active: false, offendingFiles: [], reason: 'No active audit freeze.' };
  }

  const frozenPaths = [...frontMatter.freezePaths, ENGAGEMENT_DOC_PATH];
  const offendingFiles = changedFiles.filter((file) => matchesAnyGlob(file, frozenPaths));

  if (offendingFiles.length === 0) {
    return {
      blocked: false,
      active: true,
      offendingFiles: [],
      reason: 'Audit freeze is active, but this PR does not touch any frozen path.',
    };
  }

  if (hasApprovalLabel) {
    return {
      blocked: false,
      active: true,
      offendingFiles,
      reason: `Audit freeze is active and this PR touches frozen paths, but the "${APPROVAL_LABEL}" label is present.`,
    };
  }

  return {
    blocked: true,
    active: true,
    offendingFiles,
    reason: `Audit freeze is active until ${frontMatter.freezeUntil?.toISOString()}. This PR touches frozen path(s) and does not carry the "${APPROVAL_LABEL}" label.`,
  };
}
