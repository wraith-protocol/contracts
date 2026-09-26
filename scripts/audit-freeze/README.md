# audit-freeze

CI gate that blocks pull requests from touching audit-frozen paths without
the `audit-approved` label, once a real audit engagement is signed.

## How it works

[`audit-prep/ENGAGEMENT.md`](../../audit-prep/ENGAGEMENT.md) carries two
front-matter fields:

```yaml
---
freeze_paths:
  - 'stellar/stealth-announcer/**'
freeze_until: '2026-09-30T00:00:00Z'
---
```

[`.github/workflows/audit-freeze.yml`](../../.github/workflows/audit-freeze.yml)
runs `check.ts` on every pull request. If `freeze_until` is a real timestamp
in the future, and the PR touches a path matching `freeze_paths`, the check
fails (exit 1) unless the PR carries the `audit-approved` label.

If `freeze_until` is missing, in the past, or the literal placeholder
`"TBD"` (its default value in the template), the gate is inactive and every
PR passes.

## Security design: why a PR can't shorten its own freeze

A freeze window that's defined by a file in the repo has an obvious hole: a
PR could edit `ENGAGEMENT.md` to shorten or remove `freeze_until`, then have
that same PR's check read its own edited version and pass. This gate closes
that hole with two independent layers:

1. **The workflow checks out the PR's base ref, not the default PR merge
   ref** (`ref: ${{ github.event.pull_request.base.sha }}` in
   `audit-freeze.yml`). This means the copy of `check.ts` (and everything
   else in `scripts/audit-freeze/`) that actually executes is always the
   already-merged, trusted version -- a PR cannot modify the gate's own
   logic to weaken or disable it. `check.ts` then separately fetches
   `ENGAGEMENT.md`'s content from that same base SHA via the GitHub
   Contents API (`fetchEngagementDocAtRef` in `check.ts`), rather than
   reading whatever is checked out on disk -- so even if the checkout step
   were ever changed to use the head ref instead, the freeze parameters
   themselves would still come from the base.

2. **`ENGAGEMENT.md` is always treated as a frozen path in its own right**
   whenever a freeze is active, regardless of whether it's explicitly
   listed in `freeze_paths` (see `ENGAGEMENT_DOC_PATH` in `decide.ts`, and
   the "always treats it as frozen" test in `test/decide.test.ts`). This
   holds even if the base-ref-reading approach above were ever to regress.

Both were straightforward to add, so both are in: the base-ref read is the
primary mechanism (it's what actually prevents the freeze _parameters_ from
being attacker-controlled), and treating `ENGAGEMENT.md` as self-frozen is a
cheap second layer that still requires the `audit-approved` label for any
edit to it while a freeze is active, including legitimate ones (e.g.
updating milestones mid-engagement).

## Testing

```bash
cd scripts/audit-freeze
npm install
npm test
```

- `test/glob.test.ts`, `test/parse.test.ts`, `test/decide.test.ts` -- unit
  tests for the pure logic (glob matching, front-matter parsing, the
  pass/fail decision), no network required.
- `test/check.e2e.test.ts` -- runs the actual `check.ts` CLI as a
  subprocess against a local mock GitHub API server, covering: a blocked PR,
  an approved-label override, no active freeze, a missing `ENGAGEMENT.md` at
  the base ref (404), and the ENGAGEMENT.md self-protection case.

### Manual dry-run against a real PR

```bash
GITHUB_TOKEN=<token> npx tsx check.ts --repo wraith-protocol/contracts --pr <number> --dry-run
```

Reports what the gate _would_ decide for an existing PR without exiting
non-zero -- useful for sanity-checking `freeze_paths` changes before they go
live, or for reproducing a CI failure locally.
