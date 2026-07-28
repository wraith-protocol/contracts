# Stellar Event Indexing Audit (Issue #61)

**Date:** 2026-06-27  
**Scope:** All Soroban contracts in `stellar/`  
**Background:** Soroban `getEvents` filters support 1–4 topic segments (hard limit). Unused topic slots waste filtering selectivity and force indexers to download more events than necessary.

## Status

**Overall verdict: MOSTLY OPTIMAL.** The two core contracts (`stealth-announcer`, `stealth-registry`) are correctly shaped. `wraith-names` is acceptable as-is. `stealth-splitter` has a confirmed indexing deficiency in its `ANNOUNCE` events. `stealth-batch-sender` requires a follow-up review.

---

## Findings Summary

| Contract | Event | Current Topics | Assessment | Recommendation |
|---|---|---|---|---|
| stealth-announcer | `announce` | `("announce", scheme_id, view_tag_bucket, metadata_kind)` | ✅ FINAL | None — v2 schema is authoritative |
| stealth-registry | `register` | `("register", registrant, scheme_id)` | ✅ OPTIMAL | None |
| stealth-registry | `remove` | `("remove", registrant, scheme_id)` | ✅ OPTIMAL | None |
| stealth-sender | — | No user-facing events | ✅ N/A | None |
| wraith-names | `register`, `update`, `release`, `extend` | `(event_type, name_hash)` | ✅ ACCEPTABLE | Optional: add owner as topic 3 (low priority) |
| stealth-splitter | `ANNOUNCE` | `("ANNOUNCE",)` | ⚠️ SUBOPTIMAL | Issue #62: adopt v2 topic layout or route through announcer |
| stealth-splitter | `BATCH` | `("BATCH",)` | ⚠️ SUBOPTIMAL | Covered by Issue #62 |
| stealth-splitter | `create`, `fund` | `(event_type, split_id)` | ✅ OK | None |
| stealth-batch-sender | per-transfer event | Unknown (line 83) | ⚠️ NEEDS REVIEW | Issue #63: audit event topic layout |
| stealth-batch-sender | batch summary event | Unknown (line 95) | ⚠️ NEEDS REVIEW | Issue #63: audit event topic layout |

---

## Detailed Findings

### stealth-announcer

**Event:** `announce`  
**Topics:** `("announce", scheme_id, view_tag_bucket, metadata_kind)`  
**Data:** `(stealth_address, ephemeral_pub_key, metadata)`

All 4 topic slots are used. `view_tag_bucket` is `metadata[0] as u32`, partitioning events into 256 buckets — a recipient only needs to scan ~1/256 of all announcements (~99.6% traffic reduction). `metadata_kind=1` signals that `metadata[0]` is the view tag byte. `STELLAR_V2_SCHEME_ID=2` is asserted at runtime, preventing accidental v1 data from appearing under this contract.

**No changes needed.**

## Migration: dual-emit and deprecation window

To preserve existing indexer integrations during rollout, the new v2 announcer deployment MUST emit both the v2 authoritative topic shape and a legacy v1-shaped event for a limited migration window. The repository's `stealth-announcer` implementation now emits both shapes (v2: `("announce", scheme_id, view_tag_bucket, metadata_kind)`; legacy: `("announce", scheme_id, stealth_address)` with data `(caller, ephemeral_pub_key, metadata)`).

- **Deprecation window:** Indexers should migrate to v2 and stop relying on the legacy three-topic shape within **3 months** of the v2 announcer launch. After that window the legacy shape may be removed from new deployments.

Indexers and SDKs should be updated to: (a) subscribe to v2 topics for primary scanning efficiency, and (b) continue reading the legacy shape during the migration window for backward compatibility.

---

### stealth-registry

**Events:** `register`, `remove`  
**Topics:** `("register"|"remove", registrant, scheme_id)`  
**Data:** `register` → `stealth_meta_address`; `remove` → `()`

The dominant read pattern is "fetch meta-address for address X under scheme Y." The existing `(registrant, scheme_id)` topic pair maps directly to that query. A 4th topic (e.g. a hash of the meta-address) would add no useful selectivity for any realistic indexer query.

**No changes needed.**

---

### stealth-sender

Delegates all announcement work to the `stealth-announcer` contract via `invoke_contract`. No user-facing events are emitted directly. Internal observability events from `wraith-metrics` are out of scope for this audit.

**Not applicable.**

---

### wraith-names

**Events:** `register`, `update`, `release`, `extend`  
**Topics:** `(event_type, name_hash)`  
**Data:** varies per event

`name_hash` is `SHA-256(name)`, so `(event_type, name_hash)` is a deterministic, exact-match lookup for "history of name X" — the overwhelmingly common query. This is already optimal for the primary use case.

A potential improvement is adding the owner address as topic 3, enabling queries like "all registrations by owner O." However, the owner address is not currently included in event data, so adding it would require a data-shape change. Given the low event volume of a name registry, the cost/complexity is not justified at this time.

**No changes required.** Owner-indexed queries are a low-priority optional follow-up.

---

### stealth-splitter

**Events of concern:** `ANNOUNCE`, `BATCH`

`ANNOUNCE` topics: `("ANNOUNCE",)` — only 1 topic, no filtering possible beyond event name.  
`BATCH` topics: `("BATCH",)` — same problem.

A recipient scanning for stealth payments routed through the splitter must download every `ANNOUNCE` event ever emitted by the contract. This is the exact problem the v2 announcer schema was designed to solve. The correct fix is one of:

- **(a) Route through announcer:** have `stealth-splitter` call `stealth-announcer` for each transfer instead of emitting its own `ANNOUNCE`. This fixes indexing and eliminates the architectural inconsistency (splitter announcements are not currently discoverable via the same `getEvents` filter as direct announcer events).
- **(b) Adopt v2 topic layout inline:** emit `("ANNOUNCE", scheme_id, view_tag_bucket, metadata_kind)` directly, mirroring the announcer schema without the cross-contract call overhead.

Option (a) is preferred for consistency. Tracked as **Issue #62**.

**`create` and `fund` events** (`(event_type, split_id)`) are fine — split management queries by `split_id` are the natural access pattern.

---

### stealth-batch-sender

The contract emits at least two events (per-transfer at line 83, batch summary at line 95 of `src/lib.rs`). The full topic layout was not inspected during this audit. If these events follow a pattern similar to `stealth-splitter`'s `ANNOUNCE` — emitting their own announcement-style events with fewer than 4 topics and no `view_tag_bucket` — the same indexing deficiency applies.

**Tracked as Issue #63** for a dedicated review.

---

## v2 Schema Sign-off

The `stealth-announcer` v2 event schema is **final and authoritative**:

```
topics: ("announce", scheme_id: u32, view_tag_bucket: u32, metadata_kind: u32)
data:   (stealth_address: Bytes, ephemeral_pub_key: Bytes, metadata: Bytes)
```

- `scheme_id = 2` (`STELLAR_V2_SCHEME_ID`) is the canonical Stellar stealth scheme identifier.
- `view_tag_bucket = metadata[0] as u32` when `metadata_kind = 1`.
- v1 events (scheme_id=1, 3-topic layout) remain readable from the old contract during any transition period (Path A migration).

SDK and off-chain indexers should treat this schema as stable. No further changes to `stealth-announcer` event structure are planned.

---

## Follow-up Issues

### Issue #62 — stealth-splitter: fix ANNOUNCE event topic layout

**Problem:** `stealth-splitter` emits `ANNOUNCE` with only 1 topic, making per-recipient scan filtering impossible. Recipients must download all splitter announcements.

**Options:**
1. Route splitter per-transfer announcements through `stealth-announcer` (preferred — restores unified indexing).
2. Adopt the v2 topic layout `("ANNOUNCE", scheme_id, view_tag_bucket, metadata_kind)` inline.

**Affects:** `ANNOUNCE` and `BATCH` events. `create`/`fund` events are unaffected.

---

### Issue #63 — stealth-batch-sender: audit event topic layout

**Problem:** The event topic layout in `stealth-batch-sender` was not fully reviewed in this audit. If per-transfer events lack `view_tag_bucket` and `scheme_id` topics, the same deficiency as Issue #62 applies.

**Action:** Read `stealth-batch-sender/src/lib.rs` lines 83 and 95 in full context, assess topic layout against the v2 schema, and apply the same fix as Issue #62 if needed.
