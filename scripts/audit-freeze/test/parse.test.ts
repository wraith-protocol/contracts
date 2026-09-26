import { describe, expect, it } from 'vitest';
import { parseFrontMatter } from '../parse.js';

describe('parseFrontMatter', () => {
  it('parses freeze_paths and a valid ISO freeze_until', () => {
    const content = `---
freeze_paths:
  - "stellar/stealth-announcer/**"
  - "stellar/stealth-registry/**"
freeze_until: "2026-09-30T00:00:00Z"
---

# Audit Engagement
`;

    const result = parseFrontMatter(content);

    expect(result.freezePaths).toEqual([
      'stellar/stealth-announcer/**',
      'stellar/stealth-registry/**',
    ]);
    expect(result.freezeUntil).toEqual(new Date('2026-09-30T00:00:00Z'));
  });

  it('treats a literal "TBD" freeze_until as no active freeze', () => {
    const content = `---
freeze_paths:
  - "stellar/**"
freeze_until: "TBD"
---
`;

    const result = parseFrontMatter(content);

    expect(result.freezeUntil).toBeNull();
  });

  it('treats a missing freeze_until as no active freeze', () => {
    const content = `---
freeze_paths:
  - "stellar/**"
---
`;

    const result = parseFrontMatter(content);

    expect(result.freezeUntil).toBeNull();
  });

  it('treats an empty freeze_paths list as no frozen paths', () => {
    const content = `---
freeze_paths:
freeze_until: "TBD"
---
`;

    const result = parseFrontMatter(content);

    expect(result.freezePaths).toEqual([]);
  });

  it('returns empty result for content with no front matter at all', () => {
    const content = `# Just a regular markdown file\n\nNo front matter here.\n`;

    const result = parseFrontMatter(content);

    expect(result).toEqual({ freezePaths: [], freezeUntil: null });
  });

  it('returns empty result for empty file content (e.g. file does not exist yet)', () => {
    const result = parseFrontMatter('');

    expect(result).toEqual({ freezePaths: [], freezeUntil: null });
  });

  it('handles unquoted freeze_until', () => {
    const content = `---
freeze_paths:
  - "stellar/**"
freeze_until: 2026-09-30T00:00:00Z
---
`;

    const result = parseFrontMatter(content);

    expect(result.freezeUntil).toEqual(new Date('2026-09-30T00:00:00Z'));
  });

  it('treats an unparseable freeze_until as no active freeze rather than throwing', () => {
    const content = `---
freeze_paths:
  - "stellar/**"
freeze_until: "not-a-real-date"
---
`;

    const result = parseFrontMatter(content);

    expect(result.freezeUntil).toBeNull();
  });
});
