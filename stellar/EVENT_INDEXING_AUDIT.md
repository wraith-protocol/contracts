Stellar Event Indexing Audit (Issue #61)
Date: 2026-06-27
Scope: All Soroban contracts in stellar/
Background: Soroban getEvents filters support 1–4 topic segments (hard limit). Unused topic slots waste filtering selectivity and force indexers to download more events than necessary.

Status
Overall verdict: OPTIMAL. The two core contracts (stealth-announcer, stealth-registry) are correctly shaped. wraith-names is acceptable as-is. stealth-splitter already routes per-transfer announcements through the announcer (Issue #62 was a stale audit-doc verdict). stealth-batch-sender now does the same (Issue #63 resolved).

Findings Summary
Contract	Event	Current Topics	Assessment	Recommendation
stealth-announcer	announce	("announce", scheme_id, view_tag_bucket, metadata_kind)	✅ FINAL	None — v2 schema is authoritative
stealth-registry	register	("register", registrant, scheme_id)	✅ OPTIMAL	None
stealth-registry	remove	("remove", registrant, scheme_id)	✅ OPTIMAL	None
stealth-sender	—	No user-facing events	✅ N/A	None
wraith-names	register, update, release, extend	(event_type, name_hash)	✅ ACCEPTABLE	Optional: add owner as topic 3 (low priority)
stealth-splitter	per-transfer announcement	("announce", scheme_id, view_tag_bucket, metadata_kind) via announcer	✅ OPTIMAL	None — already routed through announcer (Issue #62 was a stale audit verdict)
stealth-splitter	create, fund	(event_type, split_id)	✅ OK	None
stealth-batch-sender	per-transfer announcement	("announce", scheme_id, view_tag_bucket, metadata_kind) via announcer	✅ OPTIMAL	None — Issue #63 resolved
stealth-batch-sender	batch summary event	("BATCH",)	✅ OK	Batch-level observability; not used for recipient scan filtering
Detailed Findings
stealth-announcer
Event: announce
Topics: ("announce", scheme_id, view_tag_bucket, metadata_kind)
Data: (stealth_address, ephemeral_pub_key, metadata)

All 4 topic slots are used. view_tag_bucket is metadata[0] as u32, partitioning events into 256 buckets — a recipient only needs to scan ~1/256 of all announcements (~99.6% traffic reduction). metadata_kind=1 signals that metadata[0] is the view tag byte. STELLAR_V2_SCHEME_ID=2 is asserted at runtime, preventing accidental v1 data from appearing under this contract.

No changes needed.

stealth-registry
Events: register, remove
Topics: ("register"|"remove", registrant, scheme_id)
Data: register → stealth_meta_address; remove → ()

The dominant read pattern is "fetch meta-address for address X under scheme Y." The existing (registrant, scheme_id) topic pair maps directly to that query. A 4th topic (e.g. a hash of the meta-address) would add no useful selectivity for any realistic indexer query.

No changes needed.

stealth-sender
Delegates all announcement work to the stealth-announcer contract via invoke_contract. No user-facing events are emitted directly. Internal observability events from wraith-metrics are out of scope for this audit.

Not applicable.

wraith-names
Events: register, update, release, extend
Topics: (event_type, name_hash)
Data: varies per event

name_hash is SHA-256(name), so (event_type, name_hash) is a deterministic, exact-match lookup for "history of name X" — the overwhelmingly common query. This is already optimal for the primary use case.

A potential improvement is adding the owner address as topic 3, enabling queries like "all registrations by owner O." However, the owner address is not currently included in event data, so adding it would require a data-shape change. Given the low event volume of a name registry, the cost/complexity is not justified at this time.

No changes required. Owner-indexed queries are a low-priority optional follow-up.

stealth-splitter
Per-transfer announcements: routed through stealth-announcer via announcer_client::announce in fund_split (stealth-splitter/src/lib.rs). The resulting events are emitted by the announcer contract with the v2 topic layout ("announce", scheme_id, view_tag_bucket, metadata_kind). Recipients can apply the same topic-3 (view_tag_bucket) getEvents filter used for direct announcer traffic.

The original audit recorded splitter ANNOUNCE / BATCH as single-topic events. That verdict is stale: fund_split no longer publishes ("ANNOUNCE",) itself. Issue #62 is therefore an audit-doc correction, not a code change.

create and fund events ((event_type, split_id)) are fine — split management queries by split_id are the natural access pattern.

stealth-batch-sender
Per-transfer announcements are routed through stealth-announcer via announcer_client::announce in batch_send (stealth-batch-sender/src/lib.rs), matching the splitter pattern. Each transfer therefore appears under the announcer with the v2 4-topic layout, including view_tag_bucket = metadata[0] as u32. Indexers no longer need a batch-sender-specific decoder for a single-topic ("ANNOUNCE",) event.

The batch summary event ("BATCH",) with (from, count, asset) is operational observability and is not used for recipient scan filtering.

Issue #63 is resolved.

v2 Schema Sign-off
The stealth-announcer v2 event schema is final and authoritative:

text

topics: ("announce", scheme_id: u32, view_tag_bucket: u32, metadata_kind: u32)
data:   (stealth_address: Bytes, ephemeral_pub_key: Bytes, metadata: Bytes)
scheme_id = 2 (STELLAR_V2_SCHEME_ID) is the canonical Stellar stealth scheme identifier.
view_tag_bucket = metadata[0] as u32 when metadata_kind = 1.
v1 events (scheme_id=1, 3-topic layout) remain readable from the old contract during any transition period (Path A migration).
SDK and off-chain indexers should treat this schema as stable. No further changes to stealth-announcer event structure are planned.

The reference indexer (stellar/examples/indexer/src/processor.ts) decodes this layout through a single code path shared by announcer, splitter, and batch-sender.

Follow-up Issues
Issue #62 — stealth-splitter: fix ANNOUNCE event topic layout
Status: CLOSED (audit-doc correction). No splitter code change.

Original problem: the audit recorded stealth-splitter as emitting ANNOUNCE with only 1 topic.

Correction: fund_split already routes per-transfer announcements through stealth-announcer via announcer_client::announce. Splitter output therefore already uses the full 4-topic v2 layout ("announce", scheme_id, view_tag_bucket, metadata_kind). Recipients can filter on topic-3 (view_tag_bucket) without a full scan of splitter traffic.

Issue #63 — stealth-batch-sender: audit event topic layout
Status: RESOLVED. Closed by the merging PR that lands this change: stpatrickghost/contracts.

Original problem: stealth-batch-sender emitted per-transfer ("ANNOUNCE",) with a single topic, so server-side topic-3 view-tag filtering could not be applied to batch-sender traffic.

Fix: batch_send now invokes the announcer contract per transfer (same announcer_client::announce pattern as the splitter). Events use ("announce", scheme_id, view_tag_bucket, metadata_kind) with view_tag_bucket = metadata[0] as u32. Covered by unit + snapshot tests and by stellar/integration-tests/tests/topic_filter.rs.