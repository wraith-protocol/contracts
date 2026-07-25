# Internal Audit Index

This index gives external auditors the starting map for prior internal security work. The issue numbers below are the internal audit work items referenced by issue #62.

| Issue | Area | Report | Status | Key items for external auditor |
| --- | --- | --- | --- | --- |
| #34 | `stealth-announcer` | `stellar/stealth-announcer/audits/2026-05-gpt-5-3-codex.md` | Completed, historical v1 findings plus v2 redesign notes | Caller attribution ambiguity fixed by v2 event shape; metadata size and ephemeral key validation remain operational/client-hardening topics. |
| #35 | `stealth-sender` atomicity/security | `stellar/stealth-sender/audits/2026-05-security-audit.md` and `stellar/stealth-sender/AUDIT_SUMMARY.md` | Completed with recommendations | No critical/high findings in core atomic send review; medium operational risk around one-shot `init`; token validation documented as caller/policy responsibility. |
| #38 | `wraith-names` | `stellar/wraith-names/audits/2026-05-author.md` | Completed with one medium, two low, three informational findings | Update event semantics, replay-key TTL, on-behalf message binding, no commit-reveal, and address-based ownership should be rechecked against current code. |
| #40 | `stealth-registry` | `stellar/stealth-registry/audits/2026-06-thebabalola.md` | Completed with implemented fixes | Persistent storage migration, no squatting via `require_auth`, storage-key collision analysis, scheme-ID forward compatibility, and `remove_keys`. |
| #43 | SAC compatibility for `stealth-sender` | `stellar/audits/2026-06-sac-compatibility.md` | Draft/internal, high priority before mainnet | Clawback and revocable assets can break unlinkability/liveness; auth-required assets are incompatible with stealth; allowlist/policy enforcement is recommended. |

## Cross-cutting readiness inputs

| Document | Relevance |
| --- | --- |
| `stellar/MAINNET_READINESS.md` | Mainnet go/no-go checklist, including audit, fuzzing, Wasm size, SAC compatibility, reproducible builds, governance, and incident response. |
| `stellar/GOVERNANCE.md` | Planned frozen versus upgradeable contracts, multisig/timelock policy, pause trade-off, and upgrade auth test rationale. |
| `stellar/UPGRADE_AUTH_TESTS.md` | Upgrade authorization test suite rationale and status. |
| `stellar/build/THREAT_MODEL.md` | Existing threat model for reproducible build attestations. |

## Open items to highlight for external audit

- Confirm whether `wraith-names` update events now use a distinct event symbol or whether indexers must treat register/update as the same class.
- Confirm replay protection for on-behalf names operations remains valid across TTL/rent expiration scenarios.
- Confirm asset policy enforcement blocks or warns against SAC configurations identified in #43.
- Confirm `stealth-sender` initialization is covered by deployment tooling and cannot be forgotten in production.
- Confirm frozen contracts cannot be upgraded or administratively paused.
