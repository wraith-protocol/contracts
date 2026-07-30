# STRIDE Threat Model

## System overview

The Wraith Stellar contracts support stealth payments on Stellar:

- `stealth-announcer` emits public announcement events.
- `stealth-registry` stores user stealth meta-addresses by registrant and scheme ID.
- `stealth-sender` transfers tokens to stealth addresses and announces in one transaction.
- `wraith-names` maps `.wraith` names to 64-byte Stellar meta-addresses and supports sponsored/on-behalf operations.

Primary users are senders, recipients, relayers/sponsors, indexers/scanners, maintainers, and asset issuers.

## Assets to protect

| Asset | Protection goal |
| --- | --- |
| User funds | No contract path should move or trap funds unexpectedly. |
| Recipient unlinkability | Announcements and asset behavior should not allow practical sender-recipient linkage beyond known protocol leakage. |
| Registry integrity | Only the registrant can create, update, or remove their meta-address mapping. |
| Name ownership | Only the owner can update or release a registered name. |
| Announcement integrity | Indexers can distinguish supported event schemas and reject malformed/unsupported flows. |
| Upgrade authority | Only approved governance can upgrade or pause upgradeable contracts; frozen contracts remain immutable. |
| Build provenance | Deployed Wasm matches the audited source commit. |

## Trust boundaries

| Boundary | Notes |
| --- | --- |
| User wallet to contract | Auth must rely on Soroban `Address` auth or explicit verified signatures. |
| Contract to asset contract | `stealth-sender` calls arbitrary token contracts unless policy-gated; asset issuer behavior is a major trust boundary. |
| Contract to announcer | Sender depends on announcer liveness and event schema stability. |
| Contract to indexer/scanner | Indexers must parse event versions correctly and reject unsupported asset flows. |
| Maintainer governance to users | Upgrade and pause power can protect users during emergencies but can also censor or alter behavior. |
| Source repository to deployed Wasm | Reproducible builds and attestation bridge this boundary. |

## STRIDE analysis

### Spoofing

| Threat | Impact | Existing controls | Audit focus |
| --- | --- | --- | --- |
| Attacker registers or updates another user's registry entry. | Victim's meta-address is replaced or squatted. | `registrant.require_auth()` in registry. | Confirm all write/remove paths require the intended registrant. |
| Attacker claims ownership of another `.wraith` name. | Name resolution is hijacked. | Owner auth and ownership checks in names. | Verify register/update/release and on-behalf signature paths. |
| Relayer submits forged on-behalf names operation. | Unauthorized name registration/update/release. | Ed25519 signature verification, expiry, replay tracking. | Check domain separation, signed fields, owner binding, expiry, replay keys, and TTL. |
| Malicious contract emits announcements that look like canonical sender output. | Indexers may misclassify provenance. | Permissionless announcer is intentional; scheme/event schema versioning. | Confirm event schema and docs make provenance assumptions clear. |

### Tampering

| Threat | Impact | Existing controls | Audit focus |
| --- | --- | --- | --- |
| Registry storage-key collision. | One user's mapping overwrites another's. | Soroban XDR encoding of typed storage keys; internal audit tests. | Review `DataKey` structure and collision tests. |
| Name reverse lookup not cleaned on update/release. | Stale or poisoned reverse resolution. | Reverse delete/write logic and tests. | Confirm atomicity and all error paths. |
| Unsupported token modifies transfer semantics. | Recipient receives less than announced or loses access post-transfer. | Asset policy support and SAC compatibility tests. | Validate policy enforcement and unsupported asset handling. |
| Upgraded contract changes storage layout unsafely. | State corruption or loss. | Governance plan and upgrade auth tests. | Review storage layout, migration constraints, and timelock controls. |

### Repudiation

| Threat | Impact | Existing controls | Audit focus |
| --- | --- | --- | --- |
| Maintainer denies an upgrade/pause action. | Governance accountability failure. | Planned multisig, timelock, and documented governance. | Confirm events/logs for governance actions where implemented. |
| Relayer denies submitting a sponsored operation. | Operational dispute. | On-chain transaction record and signed payload. | Ensure signed payloads are reproducible and auditable. |
| Sender denies a stealth send. | Transaction provenance is visible at account level. | Stellar ledger records caller authorization. | Confirm sender contract does not hide the authorizing account from ledger-level audit. |

### Information disclosure

| Threat | Impact | Existing controls | Audit focus |
| --- | --- | --- | --- |
| Event fields leak recipient identity. | Privacy break. | Stealth address and ephemeral key design; v2 event bucket design. | Validate event schema minimizes avoidable metadata and documents required leakage. |
| Asset issuer freezes/claws back a stealth balance after seeing announcement. | Recipient unlinkability and liveness break. | SAC compatibility review recommends allowlist. | Confirm current policy blocks or clearly rejects risky assets. |
| Names reverse lookup links a meta-address to a human-readable name. | User opt-in identity disclosure. | Names are user-registered public mappings. | Ensure docs and APIs make this public nature explicit. |
| Registry exposes public meta-addresses. | Scanner metadata is public. | Registry stores public keys only. | Confirm no secret material is stored. |

### Denial of service

| Threat | Impact | Existing controls | Audit focus |
| --- | --- | --- | --- |
| Oversized metadata inflates event/indexer load. | Higher fees and scanner workload. | Soroban resource pricing; some tests. | Review metadata limits or documented client rejection rules. |
| Batch send length mismatch or failure mid-batch. | Partial payments or failed UX. | Transaction atomicity and length checks. | Verify batch operations revert atomically. |
| Storage TTL/rent expiration archives state. | Registry/name/replay data unavailable or replayable. | Persistent storage for registry; TTL docs and tests for names. | Confirm storage class choices and extension methods. |
| Pause misuse halts sender. | Censorship/liveness failure. | Governance plan limits pause to `stealth-sender`. | Verify pause authority, scope, and unpause path. |

### Elevation of privilege

| Threat | Impact | Existing controls | Audit focus |
| --- | --- | --- | --- |
| Non-admin upgrades `stealth-sender` or `wraith-names`. | Malicious code deployment. | Upgrade auth tests and governance plan. | Confirm upgrade interface, admin checks, timelock, and renounce behavior. |
| Frozen contracts secretly retain admin authority. | Registry/announcer trust model invalid. | Upgrade auth tests assert no admin role/interface. | Verify no hidden upgrade, pause, or admin storage paths. |
| On-behalf replay bypasses ownership. | Relayer reuses old signatures. | Replay key storage and expiry. | Review replay key derivation and storage TTL. |
| Asset policy admin allows unsafe tokens. | Privacy/liveness break via token issuer powers. | Governance review and policy docs. | Confirm policy admin model and emergency removal path. |

## Highest-risk review areas

1. Asset compatibility and policy enforcement for `stealth-sender`.
2. On-behalf authorization and replay protection in `wraith-names`.
3. Storage TTL/rent strategy for long-lived mappings and replay state.
4. Upgrade governance implementation versus `stellar/GOVERNANCE.md`.
5. Event schema stability for announcer/sender consumers.

## Maintainer review status

Maintainer review is required before sending this pack externally.

| Reviewer | Role | Status | Date |
| --- | --- | --- | --- |
| TBD | Maintainer | Pending | TBD |
| TBD | Security reviewer | Pending | TBD |
