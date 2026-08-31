import { describe, expect, it } from 'vitest';
import { matchesAnyGlob, matchesGlob } from '../glob.js';

describe('matchesGlob', () => {
  it('matches an exact path with no wildcards', () => {
    expect(matchesGlob('audit-prep/ENGAGEMENT.md', 'audit-prep/ENGAGEMENT.md')).toBe(true);
    expect(matchesGlob('audit-prep/OTHER.md', 'audit-prep/ENGAGEMENT.md')).toBe(false);
  });

  it('matches ** across directory boundaries', () => {
    expect(
      matchesGlob('stellar/stealth-announcer/src/lib.rs', 'stellar/stealth-announcer/**'),
    ).toBe(true);
    expect(
      matchesGlob(
        'stellar/stealth-announcer/src/nested/deep/file.rs',
        'stellar/stealth-announcer/**',
      ),
    ).toBe(true);
    expect(matchesGlob('stellar/stealth-registry/src/lib.rs', 'stellar/stealth-announcer/**')).toBe(
      false,
    );
  });

  it('matches * within a single path segment only', () => {
    expect(matchesGlob('stellar/wraith-names/Cargo.toml', 'stellar/*/Cargo.toml')).toBe(true);
    expect(matchesGlob('stellar/wraith-names/src/lib.rs', 'stellar/*/Cargo.toml')).toBe(false);
  });

  it('does not treat regex-special characters in the path as regex', () => {
    expect(matchesGlob('stellar/stealth-sender/src/lib.rs', 'stellar/stealth-sender/**')).toBe(
      true,
    );
    // A literal '.' in a pattern should only match a literal '.', not any character.
    expect(matchesGlob('stellar/stealth-senderXsrcXlibXrs', 'stellar/stealth-sender/**')).toBe(
      false,
    );
  });
});

describe('matchesAnyGlob', () => {
  it('returns true if any pattern matches', () => {
    const patterns = ['stellar/stealth-announcer/**', 'stellar/stealth-registry/**'];
    expect(matchesAnyGlob('stellar/stealth-registry/src/lib.rs', patterns)).toBe(true);
    expect(matchesAnyGlob('evm/contracts/WraithNames.sol', patterns)).toBe(false);
  });

  it('returns false for an empty pattern list', () => {
    expect(matchesAnyGlob('stellar/stealth-announcer/src/lib.rs', [])).toBe(false);
  });
});
