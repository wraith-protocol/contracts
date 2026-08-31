---
freeze_paths:
  - 'stellar/stealth-announcer/**'
  - 'stellar/stealth-registry/**'
  - 'stellar/stealth-sender/**'
  - 'stellar/wraith-names/**'
freeze_until: 'TBD'
---

# Audit Engagement

> **Template.** This document has the front-matter shape and section
> structure `audit-freeze.yml` expects, with the actual engagement details
> left as clearly-marked `[TBD]` placeholders below. The freeze is inactive
> (`freeze_until: "TBD"`) until those placeholders -- and the front matter
> above -- are filled in with real values from a signed SOW.
>
> `freeze_paths` above lists the four core in-scope crates from
> [`audit-prep/README.md`](./README.md) as a starting point; adjust it if
> the signed SOW's scope differs. See [How the freeze works](#how-the-freeze-works)
> below for exactly how these two fields are interpreted.

## Engagement Summary

| Field                 | Value |
| --------------------- | ----- |
| **Audit Firm**        | [TBD] |
| **SOW Signed**        | [TBD] |
| **Kickoff Date**      | [TBD] |
| **Expected Delivery** | [TBD] |

## Scope

### In-Scope Crates

- [TBD] -- e.g. `stellar/stealth-announcer`
- [TBD] -- e.g. `stellar/stealth-registry`
- [TBD] -- e.g. `stellar/stealth-sender`
- [TBD] -- e.g. `stellar/wraith-names`

### Out-of-Scope Crates

- [TBD] -- e.g. `stellar/stealth-splitter` (optional, per audit-prep/README.md)
- [TBD] -- e.g. `evm/`, `solana/`, `ckb/` (separate audits planned)

## Delivery Milestones

| Milestone                        | Target Date | Status |
| -------------------------------- | ----------- | ------ |
| [TBD] -- e.g. Kickoff            | [TBD]       | [TBD]  |
| [TBD] -- e.g. Initial findings   | [TBD]       | [TBD]  |
| [TBD] -- e.g. Final report       | [TBD]       | [TBD]  |
| [TBD] -- e.g. Remediation review | [TBD]       | [TBD]  |

## Escalation Contacts

| Role              | Name  | Contact |
| ----------------- | ----- | ------- |
| Audit Coordinator | [TBD] | [TBD]   |
| Technical Contact | [TBD] | [TBD]   |
| Audit Firm Lead   | [TBD] | [TBD]   |

## Disclosure Policy

[TBD] -- e.g. coordinated disclosure terms, embargo period, public
disclosure timeline once remediation is verified.

## How the Freeze Works

While `freeze_until` (above, in the front matter) is a real timestamp in the
future, [`.github/workflows/audit-freeze.yml`](../.github/workflows/audit-freeze.yml)
blocks any pull request that touches a path matching `freeze_paths` unless
the PR carries the `audit-approved` label. Once `freeze_until` passes, or is
reset to `"TBD"`, the gate is inactive again.

This file (`audit-prep/ENGAGEMENT.md`) is always treated as a frozen path in
its own right whenever a freeze is active, regardless of whether it's
explicitly listed in `freeze_paths` -- so a PR can't shorten or remove its
own freeze window to slip changes past the gate. The gate also always reads
this file's content from the pull request's base ref, never its head ref,
as a second, independent layer of the same protection. See
[`scripts/audit-freeze/check.ts`](../scripts/audit-freeze/check.ts) and
[`scripts/audit-freeze/decide.ts`](../scripts/audit-freeze/decide.ts) for
the implementation.
