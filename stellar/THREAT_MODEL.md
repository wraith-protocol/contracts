# Stellar Contracts STRIDE Threat Model

This document is the unified threat model for the four core Wraith Protocol
Soroban contracts. It describes the deployed contract boundary represented by
the current source, not proposed pause or upgrade infrastructure. Each public
contract entry point appears in a table below and is mapped to at least one
STRIDE category.

## Scope and assumptions

| Contract | Assets and trust boundary | Public entry points |
| --- | --- | --- |
| [`stealth-announcer`](./stealth-announcer/) | Integrity and availability of the public announcement event stream | `announce` |
| [`stealth-registry`](./stealth-registry/) | `(registrant, scheme_id)` to public 64-byte meta-address mappings | `register_keys`, `remove_keys`, `stealth_meta_address_of` |
| [`stealth-sender`](./stealth-sender/) | Sender authorization, token transfers, fee routing, and transfer/announcement atomicity | `init`, `send`, `batch_send` |
| [`wraith-names`](./wraith-names/) | Name ownership, forward/reverse mappings, delegated signatures, and storage lifetime | `register`, `register_on_behalf`, `update`, `update_on_behalf`, `release`, `release_on_behalf`, `resolve`, `name_of`, `extend_name_ttl` |

Soroban authentication, transaction atomicity, ledger resource limits, and
contract-address isolation are trusted platform controls. Token contracts, the
configured announcer and asset-policy contracts, deployment tooling, RPC
providers, indexers, wallets, and user key custody are outside each contract's
trust boundary. Meta-addresses, names, announcements, metadata, and events are
public; confidentiality is not a property these contracts provide.

STRIDE abbreviations used in the coverage tables are **S**poofing,
**T**ampering, **R**epudiation, **I**nformation disclosure, **D**enial of
service, and **E**levation of privilege.

## Spoofing

| Contract and surface | Threat | Mitigation |
| --- | --- | --- |
| `stealth-announcer.announce` | A caller can publish an announcement that looks relevant to a recipient. | The event makes no caller-authenticity claim. V2 removes the misleading v1 caller field, fixes `scheme_id` to 2, and commits to a metadata kind. Wallets must cryptographically validate announcements. The v2 change resolves the caller-attribution ambiguity described in [WA-ANN-01](./stealth-announcer/audits/2026-05-gpt-5-3-codex.md#wa-ann-01--event-caller-payload-is-always-the-announcer-contract-not-the-invoker). |
| `stealth-registry.register_keys`, `remove_keys` | An attacker registers, replaces, or removes another address's keys. | `registrant.require_auth()` gates both writes. The registry audit confirmed this prevents replacement squatting and relies on Stellar transaction sequence checks for signed-transaction replay ([sections 2.3 and 2.6](./stealth-registry/audits/2026-06-thebabalola.md#23-replacement-squatting-high)). |
| `stealth-sender.send`, `batch_send` | A caller spends tokens while impersonating `sender`. | Both functions call `sender.require_auth()` before cross-contract calls. The sender audit validated the authorization context ([finding 6](./stealth-sender/audits/2026-05-security-audit.md#6-auth-caching--require_auth_for_args)). |
| `wraith-names.register`, `update`, `release` | A caller manages a name as another owner. | Soroban `owner.require_auth()` is required; internal ownership/parent-manager checks protect existing names and subdomains. |
| `wraith-names.*_on_behalf` | A relayer forges or reuses delegated authority. | The contract derives the Ed25519 key from the account address, verifies a domain- and operation-specific digest, enforces ledger expiry, and rejects a persistent replay hash before mutation. |

## Tampering

| Contract and surface | Threat | Mitigation |
| --- | --- | --- |
| `stealth-announcer.announce` | Inputs are altered or ambiguously decoded by consumers. | Soroban serializes typed arguments; `BytesN<32>` fixes key length; v2 accepts only scheme 2, requires a first metadata byte, and emits a stable topic/data schema. Events are ledger records, not mutable contract storage. |
| `stealth-registry.register_keys`, `remove_keys`, `stealth_meta_address_of` | Keys collide, malformed mappings are stored, or lookup observes a different slot. | Meta-addresses must be exactly 64 bytes. Typed `DataKey::MetaAddress(Address, u32)` serialization prevents packing collisions, as assessed in [audit section 2.2](./stealth-registry/audits/2026-06-thebabalola.md#22-storage-key-collision-risk-low). Writes and reads use the same key and persistent storage. |
| `stealth-sender.init` | Configuration is replaced after deployment. | The presence of `DataKey::Announcer` makes initialization one-shot; fee basis points are capped at 50 and a nonzero fee requires a recipient. |
| `stealth-sender.send`, `batch_send` | Transfer and announcement diverge, batch entries become misaligned, or a disallowed asset is used. | A configured asset policy is checked; batch vector lengths must match; transfers and announcer calls execute atomically. The audit confirmed [batch atomicity](./stealth-sender/audits/2026-05-security-audit.md#3-batch-send-atomicity) and [announcer coupling](./stealth-sender/audits/2026-05-security-audit.md#4-announcer-call-coupling). |
| `wraith-names` mutation functions | Names, reverse mappings, or ownership are changed inconsistently. | Names and 64-byte meta-addresses are validated; full SHA-256 keys are used; existing ownership or parent-manager authority is checked; forward and reverse changes share one atomic invocation; register/update/release use distinct events. The audit assessed hash use, ownership, and reverse cleanup as correct. |
| `wraith-names.resolve`, `name_of`, `extend_name_ttl` | A read or TTL extension targets an absent or mismatched record. | Missing records return `NameNotFound`; reverse lookup resolves through the corresponding name entry; TTL extension derives both keys from the stored entry and rejects a non-future ledger. |

## Repudiation

| Contract and surface | Threat | Mitigation |
| --- | --- | --- |
| `stealth-announcer.announce` | A party denies originating an announcement, or an indexer attributes it to a user. | Events prove only that the announcer contract emitted supplied data. V2 intentionally makes no origin claim; callers and indexers must not treat an announcement as evidence of payer authorization. |
| `stealth-registry.register_keys`, `remove_keys` | A registrant denies a mapping change. | Soroban auth is recorded in the transaction authorization tree; `register` and `remove` events identify registrant and scheme. |
| `stealth-registry.stealth_meta_address_of` | A consumer disputes a lookup result. | Results are deterministically derived from ledger state; historical proof depends on ledger/RPC retention, not a view-call event. |
| `stealth-sender.init` | Deployment configuration is disputed. | Configuration is stored on ledger, but `init` emits no dedicated configuration event. Deployment records must capture the initialization transaction. |
| `stealth-sender.send`, `batch_send` | A sender denies authorizing payment or event data. | `require_auth`, token transfer records, announcer events, and metric events provide ledger evidence. Atomicity prevents a successful transfer without its announcement. |
| `wraith-names` mutation functions, including `*_on_behalf` and `extend_name_ttl` | An owner, relayer, or TTL sponsor denies a lifecycle action. | Direct calls have Soroban auth; delegated calls have verifiable Ed25519 signatures and replay hashes; register/update/release/extend emit distinct lifecycle events. |
| `wraith-names.resolve`, `name_of` | A resolver result is disputed. | Deterministic ledger state can be queried, but view calls intentionally emit no event. |

## Information disclosure

| Contract and surface | Threat | Mitigation |
| --- | --- | --- |
| `stealth-announcer.announce` | Stealth address, ephemeral key, view-tag bucket, and metadata reveal linkable information. | Disclosure is required for scanning. V2 keeps the stealth address out of indexed topics and uses a one-byte bucket, but all event data remains public. Clients should place no secret or identifying plaintext in metadata. |
| `stealth-registry` all entry points | Registration links a Stellar address and scheme to public spending/viewing keys; lookups and events expose changes. | The contract stores public keys only and no secret material, as noted in [audit section 2.5](./stealth-registry/audits/2026-06-thebabalola.md#25-state-exposure--privileged-side-channels-low). `remove_keys` deletes live state but cannot erase ledger history. |
| `stealth-sender.init`, `send`, `batch_send` | Configuration, token, amounts, sender, destinations, fees, and announcements are observable and correlate activity. | No on-chain confidentiality is claimed. Fresh stealth addresses limit recipient reuse; applications must avoid identifying metadata and understand that atomic calls remain publicly correlated. |
| `wraith-names` all entry points | Names deliberately link human-readable identifiers to meta-addresses and owners; signatures and recovery/storage activity add timing signals. | Only public mapping material is stored. SHA-256 storage keys reduce plain-text key exposure but events and return values disclose names; clients must treat registration as public and voluntary. |

## Denial of service

| Contract and surface | Threat | Mitigation |
| --- | --- | --- |
| `stealth-announcer.announce` | Permissionless spam, oversized metadata, malformed keys, or an empty view tag consumes indexer/ledger resources. | Network fees and Soroban limits price spam; an empty metadata value and non-v2 scheme fail. Indexers can filter by scheme and view-tag bucket. Metadata remains uncapped and curve validity is not checked ([WA-ANN-02 and WA-ANN-03](./stealth-announcer/audits/2026-05-gpt-5-3-codex.md#wa-ann-02--unbounded-metadata-can-inflate-event-payloads-and-indexer-workload)). |
| `stealth-registry.register_keys`, `remove_keys`, `stealth_meta_address_of` | Storage exhaustion or archival makes registration/lookup unavailable. | Per-user persistent entries avoid instance-footprint exhaustion; active writes and successful reads extend entry and instance TTL. Invalid lengths fail before storage. This follows the storage fix in [audit section 2.1](./stealth-registry/audits/2026-06-thebabalola.md#21-storage-type-efficiency--rent-strategy-medium). |
| `stealth-sender.init` | An arbitrary first caller permanently installs malicious or invalid dependencies, or initialization is omitted. | One-shot initialization prevents later replacement, but there is no initializer authentication. Safe deployment requires atomic or otherwise controlled initialization and verified dependency addresses; the audit calls out omitted initialization as an operational risk ([finding 8](./stealth-sender/audits/2026-05-security-audit.md#8-init--upgrade-story)). |
| `stealth-sender.send`, `batch_send` | A failing token, policy, announcer, insufficient balance, excessive batch, or expired instance state blocks payment. | Failures revert all effects; policy can restrict tokens; length checks reject malformed batches; use extends instance TTL. Callers pay resource fees and should bound/preflight batches. |
| `wraith-names.register*` | Name front-running/squatting prevents the intended owner from registering. | Existing names cannot be overwritten and subdomains require parent authority. There is no commit-reveal protection; the names audit records the residual ordering risk ([name squatting/front-running](./wraith-names/audits/2026-05-author.md#name-squatting--front-running)). |
| `wraith-names` reads/writes and `extend_name_ttl` | Persistent entries or replay markers expire; arbitrary huge extension requests consume resources or fail. | Normal successful access extends relevant TTLs and the permissionless endpoint lets sponsors extend a name and reverse key. Ledger bounds/resource fees constrain requests; operational keepers remain necessary. |

## Elevation of privilege

| Contract and surface | Threat | Mitigation |
| --- | --- | --- |
| `stealth-announcer.announce` | A caller gains an implied trusted-announcer role. | There is deliberately no privileged role and the event asserts no caller identity. Consumers must apply cryptographic validation and their own trust policy. |
| `stealth-registry` all entry points | A user or administrator modifies another user's slot or reads privileged data. | Mutations require the exact registrant's auth; reads are intentionally public; the current contract exposes no admin or upgrade entry point. |
| `stealth-sender.init` | The initialization caller gains control over routing, asset policy, or fees. | Configuration becomes immutable after the first call and fees are capped, but deployment-time capture remains a residual risk. |
| `stealth-sender.send`, `batch_send` | A dependency or caller bypasses sender authority or redirects more than configured. | Sender auth precedes calls; policy and fee configuration are read from immutable initialized state; Soroban atomicity rolls back dependency failure. |
| `wraith-names.register`, `update`, `release` | A non-owner acquires management rights; a subdomain owner bypasses its parent. | Direct auth plus stored-owner/parent-manager checks enforce authority. |
| `wraith-names.*_on_behalf` | A signature is replayed in another deployment/chain or after replay state expires. | Operation domain, expiry, owner-derived signature verification, and persistent replay keys limit use. The signed message does not bind the network or contract address, so cross-deployment replay remains possible where the same inputs and key are valid. |
| `wraith-names.resolve`, `name_of`, `extend_name_ttl` | Public callers obtain mutation authority through a view or maintenance function. | Reads cannot alter mappings; TTL extension can only prolong existing records and cannot change owner, name, or meta-address. |

## Entry-point coverage matrix

Every public `#[contractimpl]` entry point is mapped below. A check mark means
the function has a surface or control discussed in that category; it does not
mean that every listed threat is fully eliminated.

| Contract | Function | S | T | R | I | D | E |
| --- | --- | :---: | :---: | :---: | :---: | :---: | :---: |
| stealth-announcer | `announce` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| stealth-registry | `register_keys` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| stealth-registry | `remove_keys` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| stealth-registry | `stealth_meta_address_of` |  | ✓ | ✓ | ✓ | ✓ | ✓ |
| stealth-sender | `init` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| stealth-sender | `send` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| stealth-sender | `batch_send` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| wraith-names | `register` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| wraith-names | `register_on_behalf` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| wraith-names | `update` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| wraith-names | `update_on_behalf` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| wraith-names | `release` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| wraith-names | `release_on_behalf` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| wraith-names | `resolve` |  | ✓ | ✓ | ✓ | ✓ | ✓ |
| wraith-names | `name_of` |  | ✓ | ✓ | ✓ | ✓ | ✓ |
| wraith-names | `extend_name_ttl` |  | ✓ | ✓ | ✓ | ✓ | ✓ |

`view_tag_bucket` and the sender's announcer/asset-policy wrappers are internal
implementation helpers rather than callable contract entry points; their input
validation and dependency-call risks are covered under `announce`, `send`, and
`batch_send`.

## Residual risks

These risks are not claimed as mitigated by the current contracts:

| ID | Contract(s) | STRIDE | Residual risk and required operational response |
| --- | --- | --- | --- |
| RR-01 | stealth-announcer | S, R, D | Announcements are permissionless and unattributed. Spam is only economically bounded; consumers must never infer payer authorization from an event. |
| RR-02 | stealth-announcer, stealth-sender | T, D | Metadata has no maximum and a 32-byte ephemeral key is not guaranteed curve-valid. Wallets/indexers must cap payload processing and reject invalid keys. |
| RR-03 | all | I | Ledger state, calldata, events, amounts, timing, and historical data are public. Removal cannot provide erasure, and stealth addresses do not hide the sender-to-one-time-address transfer. |
| RR-04 | stealth-registry, stealth-sender, wraith-names | D | Contract-instance and persistent-entry TTL/rent can cause archival or unavailability. Active calls extend some TTLs, but operators and users must monitor and restore/extend state. |
| RR-05 | stealth-sender | S, T, D, E | `init` is unauthenticated and first-caller-wins. A deployment that is not initialized in a controlled transaction can be captured; a wrong announcer/policy/fee recipient is then immutable. |
| RR-06 | stealth-sender | T, D | Correctness and availability depend on configured and caller-selected external contracts. Malicious/nonstandard tokens, a failing policy, or a failing announcer revert sends; token issuer freeze/clawback/fee behavior remains outside this contract. |
| RR-07 | stealth-sender | D | Batch work is linear and has no explicit item cap. Oversized batches fail resource limits and still cost submission fees; clients must bound and simulate batches. |
| RR-08 | wraith-names | S, D | Top-level registration has no commit-reveal, so transaction ordering can enable name front-running. Released names may be immediately re-registered. |
| RR-09 | wraith-names | S, E | Delegated signatures bind a Wraith domain, operation, name, meta-address and expiry, but not network passphrase or contract ID. Reuse across compatible deployments is possible; use short expiries and deployment-specific signing policy. |
| RR-10 | wraith-names | D | Losing the owner key loses direct name control; storage expiry can remove a mapping or replay marker. Ledger history and already observed meta-addresses remain public. |
| RR-11 | all | R, D | RPC/indexer omission, reorganization handling, and historical retention are off-chain responsibilities. Consumers should use trusted/independent providers and confirmation/finality policies. |

## Review maintenance

Update this model whenever a contract entry point, event schema, authorization
rule, storage key/TTL policy, dependency, or upgrade/pause capability changes.
The coverage matrix should be compared against every `pub fn` inside each
contract's `#[contractimpl]` block during review.
