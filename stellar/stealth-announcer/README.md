# Stealth Announcer (Stellar)

The Soroban stealth announcer is a stateless, permissionless event emitter for
Stellar v2 stealth-payment announcements. It accepts scheme ID 2 and publishes
an indexed view-tag bucket so wallets can pre-filter recipient scans.

Security assumptions, STRIDE coverage for `announce`, audit references, and
open risks are documented in the unified [Stellar threat model](../THREAT_MODEL.md).

The contract-specific audit is
[`audits/2026-05-gpt-5-3-codex.md`](./audits/2026-05-gpt-5-3-codex.md).
