# Announcer v2 Migration & Release Notes

Summary
- Introduces v2 `announce` event topics: `("announce", scheme_id, view_tag_bucket, metadata_kind)` with data `(stealth_address, ephemeral_pub_key, metadata)`.
- During migration v2 deployments MUST dual-emit the legacy v1-shaped event for 3 months.

Deprecation window
- Duration: 3 months from v2 announcer mainnet launch.
- Rationale: gives indexers + SDKs time to update filters and backfill any needed historical data.

Migration checklist for releases
1. Deploy new `stealth-announcer` contract that enforces `scheme_id = 2` and emits both v2 and legacy v1-shaped events.
2. Announce the migration and deprecation window to integrators and indexers (mailing list, Discord, repo issue).
3. SDKs: update `getEvents` subscriptions to prefer topics `("announce", 2, my_bucket, *)`. Continue reading legacy v1 during the window.
4. Indexers: add filters for Topic 2 buckets and migrate storage backends as needed. Validate that dual-emit events are both visible in streaming/backfill paths.
5. After 3 months, perform a coordinated removal of the legacy v1 emission from new deployments and update docs to mark v1 historical-only.

Notes
- The repository contains unit and integration test coverage to validate dual-emit behavior. Run `cargo test -p stealth-announcer` to verify locally.
- Do not remove historical v1 events from chain history; they remain readable indefinitely.
