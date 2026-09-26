# Deployment Manifest

## Current status

`stellar/contract-ids.json` currently contains empty values for the four core Stellar contracts:

| Contract | Mainnet contract ID | Status |
| --- | --- | --- |
| `stealth-announcer` | TBD | Not deployed/final ID not recorded. |
| `stealth-registry` | TBD | Not deployed/final ID not recorded. |
| `stealth-sender` | TBD | Not deployed/final ID not recorded. |
| `wraith-names` | TBD | Not deployed/final ID not recorded. |

`stellar/verification/status.json` is currently `pending`, with no completed checks recorded.

## Expected deployment order

1. Deploy `stealth-announcer`.
2. Deploy `stealth-registry`.
3. Deploy `stealth-sender` and initialize it with the announcer contract address and any required policy/admin configuration.
4. Deploy `wraith-names`.
5. Configure governance and asset policy controls.
6. Update `stellar/contract-ids.json`.
7. Run reproducible build verification against final IDs.

## Governance manifest

| Contract | Governance expectation | Audit note |
| --- | --- | --- |
| `stealth-announcer` | Frozen/no upgrade path. | Verify no admin role, no pause, no upgrade interface. |
| `stealth-registry` | Frozen/no upgrade path. | Verify user mappings cannot be changed by maintainers. |
| `stealth-sender` | 3-of-5 multisig, 7-day timelock for upgrades, pause mechanism for emergencies. | Verify pause and upgrade powers are scoped and observable. |
| `wraith-names` | 3-of-5 multisig, 7-day timelock, eventually renounceable. | Verify state migration and replay storage safety under upgrades. |

Planned guardians from `stellar/GOVERNANCE.md`:

- `@truthixify`
- `@thebabalola`
- `@bbkenny`
- `@richiey1`
- `@drips-wave`

## Mainnet readiness dependencies

From `stellar/MAINNET_READINESS.md`, the audit-relevant blockers are:

- All four contract audits completed with zero unresolved Critical or High findings.
- Property-based fuzz tests running nightly with no failures for 30 days.
- Wasm size budget under threshold for every contract.
- SAC compatibility matrix signed off.
- Reproducible build attestation verified.
- Deployment script tested on Futurenet and dry-run for Mainnet.
- Upgrade governance finalized and signers configured.
- Multisig hardware setup verified by an independent third party.
- Incident response runbook and on-call rotation active.

## Final deployment sign-off template

| Item | Value |
| --- | --- |
| Audited commit | TBD |
| Deployed commit | TBD |
| Attestation hash/file | TBD |
| Verification workflow run | TBD |
| Mainnet contract IDs recorded | Pending |
| Security council configured | Pending |
| Timelock configured | Pending |
| Pause owner configured | Pending |
| External audit report link | Pending |
