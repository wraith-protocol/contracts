Stellar Event Indexing Audit (Issue #61)
Date: 2026-06-27
Last updated: 2026-08-25
Scope: All Soroban contracts in stellar/
Background: Soroban getEvents filters support 1–4 topic segments (hard limit). Unused topic slots waste filtering selectivity and force indexers to download more events than necessary.

Status
Overall verdict: OPTIMAL. The two core contracts (stealth-announcer, stealth-registry) are correctly shaped. wraith-names is acceptable as-is. stealth-splitter already routes per-transfer announcements through the announcer (Issue #62 was a stale audit-doc finding). stealth-batch-sender now does the same (Issue #63 resolved), so all three announcement sources share the v2 4-topic layout.

Findings Summary
Contract	Event	Current Topics	Assessment	Recommendation
stealth-announcer	announce	("announce", scheme_id, view_tag_bucket, metadata_kind)	✅ FINAL	None — v2 schema is authoritative
stealth-registry	register	("register", registrant, scheme_id)	✅ OPTIMAL	None
stealth-registry	remove	("remove", registrant, scheme_id)	✅ OPTIMAL	None
stealth-sender	—	No user-facing events (delegates to announcer)	✅ N/A	None
wraith-names	register, update, release, extend	(event_type, name_hash)	✅ ACCEPTABLE	Optional: add owner as topic 3 (low priority)
stealth-splitter	per-transfer announce	routed through announcer → v2 4-topic layout	✅ RESOLVED	Issue #62 closed — audit-doc correction; fund_split already calls announcer_client::announce
stealth-splitter	create, fund	(event_type, split_id)	✅ OK	None
stealth-batch-sender	per-transfer announce	routed through announcer → v2 4-topic layout	✅ RESOLVED	Issue #63 closed — see merging PR
stealth-batch-sender	batch summary	("BATCH",)	✅ OK	Summary event; not an announcement. Filter by announcer for scan traffic.
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
Per-transfer announcements: routed through stealth-announcer.

The original audit flagged ANNOUNCE / BATCH as single-topic events. That verdict is stale. fund_split (see stealth-splitter/src/lib.rs) already imports announcer_client::announce and invokes the announcer contract for every beneficiary transfer. Splitter output therefore appears on the announcer with the full 4-topic v2 layout ("announce", scheme_id, view_tag_bucket, metadata_kind). Recipients can apply the same topic-3 view-tag filter they use for direct announcer events.

The splitter does not emit its own ANNOUNCE or BATCH events. Management events create and fund ((event_type, split_id)) are unchanged and remain the correct shape for split-management queries.

Issue #62 closed as an audit-doc correction. No splitter code change was required.

stealth-batch-sender
Per-transfer announcements: routed through stealth-announcer (same announcer_client::announce pattern as splitter).

The original audit could not inspect the topic layout and opened Issue #63. The contract previously emitted a single-topic ("ANNOUNCE",) event inline, which broke server-side topic-3 view-tag filtering and forced indexers to full-scan batch-sender output.

That emission is gone. Each transfer now calls the announcer, so batch-sender output uses the identical v2 4-topic layout (including view_tag_bucket = metadata[0] as u32). The remaining ("BATCH",) event is a batch-level summary (from, count, asset) — not an announcement — and does not need view-tag selectivity.

Issue #63 resolved. Indexers watch the announcer; there is no batch-sender-specific announce path. Covered by the integration test in stellar/integration-tests/tests/topic_filter.rs.

v2 Schema Sign-off
The stealth-announcer v2 event schema is final and authoritative:

text

topics: ("announce", scheme_id: u32, view_tag_bucket: u32, metadata_kind: u32)
data:   (stealth_address: Bytes, ephemeral_pub_key: Bytes, metadata: Bytes)
scheme_id = 2 (STELLAR_V2_SCHEME_ID) is the canonical Stellar stealth scheme identifier.
view_tag_bucket = metadata[0] as u32 when metadata_kind = 1.
v1 events (scheme_id=1, 3-topic layout) remain readable from the old contract during any transition period (Path A migration).
SDK and off-chain indexers should treat this schema as stable. No further changes to stealth-announcer event structure are planned.

All three announcement sources (direct announcer, splitter, batch-sender) produce this layout. The reference indexer processor uses a single code path for all three — see stellar/examples/indexer/src/processor.ts.

Follow-up Issues
Issue #62 — stealth-splitter: fix ANNOUNCE event topic layout
Status: CLOSED (audit-doc correction).

Original problem: stealth-splitter was recorded as emitting ANNOUNCE with only 1 topic.

Resolution: No code change. fund_split already routes per-transfer announcements through announcer_client::announce, so splitter output has the v2 4-topic layout. The ANNOUNCE / BATCH rows in the original findings table were stale.

Issue #63 — stealth-batch-sender: audit event topic layout
Status: RESOLVED.

Original problem: The event topic layout in stealth-batch-sender was not fully reviewed. Per-transfer events used a single-topic ("ANNOUNCE",) publish, so topic-3 view-tag filtering could not be applied.

Resolution: Batch-sender now routes each transfer through the announcer contract (announcer_client::announce), matching splitter. Unit + snapshot tests assert the 4-topic layout. The integration test topic_filter.rs verifies a topic-3 filter returns only matching view-tag entries across announcer, splitter, and batch-sender. Merging PR: this change.