import { describe, expect, it } from 'vitest';
import { APPROVAL_LABEL, ENGAGEMENT_DOC_PATH, decide } from '../decide.js';

const NOW = new Date('2026-08-01T00:00:00Z');
const FUTURE = new Date('2026-12-31T00:00:00Z');
const PAST = new Date('2026-01-01T00:00:00Z');

describe('decide', () => {
  it('is not blocked when there is no active freeze', () => {
    const result = decide({
      frontMatter: { freezePaths: ['stellar/stealth-announcer/**'], freezeUntil: null },
      changedFiles: ['stellar/stealth-announcer/src/lib.rs'],
      hasApprovalLabel: false,
      now: NOW,
    });

    expect(result.blocked).toBe(false);
    expect(result.active).toBe(false);
  });

  it('is not blocked when the freeze window has already passed', () => {
    const result = decide({
      frontMatter: { freezePaths: ['stellar/stealth-announcer/**'], freezeUntil: PAST },
      changedFiles: ['stellar/stealth-announcer/src/lib.rs'],
      hasApprovalLabel: false,
      now: NOW,
    });

    expect(result.blocked).toBe(false);
    expect(result.active).toBe(false);
  });

  it('is not blocked when the freeze is active but no changed file is in scope', () => {
    const result = decide({
      frontMatter: { freezePaths: ['stellar/stealth-announcer/**'], freezeUntil: FUTURE },
      changedFiles: ['evm/contracts/WraithNames.sol', 'README.md'],
      hasApprovalLabel: false,
      now: NOW,
    });

    expect(result.blocked).toBe(false);
    expect(result.active).toBe(true);
    expect(result.offendingFiles).toEqual([]);
  });

  it('is blocked when the freeze is active and a changed file is in scope, without the label', () => {
    const result = decide({
      frontMatter: { freezePaths: ['stellar/stealth-announcer/**'], freezeUntil: FUTURE },
      changedFiles: ['stellar/stealth-announcer/src/lib.rs', 'README.md'],
      hasApprovalLabel: false,
      now: NOW,
    });

    expect(result.blocked).toBe(true);
    expect(result.offendingFiles).toEqual(['stellar/stealth-announcer/src/lib.rs']);
  });

  it(`is not blocked when the "${APPROVAL_LABEL}" label is present, even if in-scope files changed`, () => {
    const result = decide({
      frontMatter: { freezePaths: ['stellar/stealth-announcer/**'], freezeUntil: FUTURE },
      changedFiles: ['stellar/stealth-announcer/src/lib.rs'],
      hasApprovalLabel: true,
      now: NOW,
    });

    expect(result.blocked).toBe(false);
    expect(result.active).toBe(true);
    expect(result.offendingFiles).toEqual(['stellar/stealth-announcer/src/lib.rs']);
  });

  it(`always treats ${ENGAGEMENT_DOC_PATH} itself as frozen while a freeze is active, even if not in freeze_paths`, () => {
    const result = decide({
      frontMatter: { freezePaths: ['stellar/stealth-announcer/**'], freezeUntil: FUTURE },
      changedFiles: [ENGAGEMENT_DOC_PATH],
      hasApprovalLabel: false,
      now: NOW,
    });

    expect(result.blocked).toBe(true);
    expect(result.offendingFiles).toEqual([ENGAGEMENT_DOC_PATH]);
  });

  it(`allows an approved edit to ${ENGAGEMENT_DOC_PATH} itself, since the label override still applies`, () => {
    const result = decide({
      frontMatter: { freezePaths: ['stellar/stealth-announcer/**'], freezeUntil: FUTURE },
      changedFiles: [ENGAGEMENT_DOC_PATH],
      hasApprovalLabel: true,
      now: NOW,
    });

    expect(result.blocked).toBe(false);
  });
});
