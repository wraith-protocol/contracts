# Soroban Event Topic Design for `stealth-announcer`

## Status

Recommendation: do this, via a new `scheme_id = 2` announcer deployment.

This issue is worth fixing. Soroban RPC can filter on indexed event topics at the `getEvents` layer, and the current Stellar announcer schema leaves too much of the scan workload to clients.

## Problem

Today the contract emits:

- Topics: `("announce", scheme_id, stealth_address)`
- Data: `(announcer_contract_address, ephemeral_pub_key, metadata)`

That lets indexers filter by event type and `scheme_id`, but not by recipient-related scan hints. A recipient, SDK, or watcher still has to download every matching announcement and inspect `metadata` client-side.

For low throughput this is fine. At sustained mainnet volume it becomes expensive because:

- `getEvents` response bytes scale with the number of returned events.
- RPC providers commonly price by request volume and/or egress.
- Every recipient repeats the same broad fetch even though almost all announcements are irrelevant to them.

## Relevant Soroban RPC Constraints

Stellar RPC `getEvents` filters can match one to four topic segments per filter, and an event topic list itself is limited to four items. This is the hard design budget for indexed fields.

That means we cannot index every useful field. We have to spend topic slots on the fields that most improve server-side selectivity.

## Proposed Topic Schema

Use all four indexed topic slots:

- Topic 0: `"announce"`
- Topic 1: `scheme_id`
- Topic 2: `view_tag_bucket`
- Topic 3: `metadata_kind`

Move everything else into event data:

- Data: `(stealth_address, ephemeral_pub_key, metadata)`

Example:

```text
topics = ("announce", 2, 173, "default")
data   = (stealth_address, ephemeral_pub_key, metadata)
```

## Field Meanings

### Topic 0: `"announce"`

Event-type discriminator. This stays first so SDKs and infra can filter for a single contract event family.

### Topic 1: `scheme_id`

Lets clients query only the stealth scheme they understand. This is a small but still useful filter, especially during migration windows where v1 and v2 may coexist.

### Topic 2: `view_tag_bucket`

This is the main optimization.

Use one byte derived from the recipient scan hint, with 256 possible buckets. The simplest version is:

- If `metadata[0]` is the view tag byte, then `view_tag_bucket = metadata[0]`.

If the metadata format later changes, the bucket should still remain a stable first-class indexed value so indexers do not need to decode arbitrary metadata blobs just to build filters.

### Topic 3: `metadata_kind`

Reserve the last slot for coarse discovery classes such as:

- `"default"`
- `"invoice"`
- `"subscription"`

This is optional at launch, but it is worth reserving now because Soroban only gives us four indexed topic slots. If we spend the last slot on something less selective today, we will likely regret it later.

## Why `stealth_address` Should Leave Topics

The current schema indexes `stealth_address`, but that field is not a useful scan key:

- Recipients do not know the stealth address until after they derive and validate it.
- Filtering by `stealth_address` does not help broad wallet scanning.
- Keeping it indexed would crowd out `view_tag_bucket`, which is far more selective.

It belongs in event data, not in the indexed topic budget.

## Expected Client Query Shape

For a wallet scanning a v2 announcer:

```text
topics = [
  "announce",
  2,
  my_view_tag_bucket,
  "*"
]
```

For a watcher scanning a specific class:

```text
topics = [
  "announce",
  2,
  my_view_tag_bucket,
  "invoice"
]
```

This preserves exact matching on event type and scheme while letting the RPC server discard roughly 255 of every 256 unrelated announcements before they cross the network.

## Back-of-the-Envelope Savings Model

### Assumptions

- Mainnet announcement rate: `50 announcements / second`
- Daily announcements: `50 * 86,400 = 4,320,000`
- Active scanners: `10,000 users`
- Average returned announcement payload over RPC: `~500 bytes`

The 500-byte assumption is intentionally conservative and close to the repo's current serialized test event footprint. Real RPC responses may be somewhat larger because they also include ledger, cursor, tx hash, and envelope fields. The ratio below is the important part.

### Current v1 shape

Each user effectively downloads all announcements for the scheme they watch.

- Per user per day: `4,320,000 * 500 B = 2.16 GB/day`
- Across 10k users: `21.6 TB/day`

### Proposed v2 shape with `view_tag_bucket`

Each user queries only one of 256 buckets.

- Expected events per user per day: `4,320,000 / 256 = 16,875`
- Per user per day: `16,875 * 500 B = 8.44 MB/day`
- Across 10k users: `84.4 GB/day`

### Net effect

- Traffic reduction: `256x`
- Response-byte reduction: about `99.61%`
- Aggregate daily savings at 10k users: about `21.5 TB/day`

Even if the true RPC payload is 2x larger, the percentage savings stay the same and the absolute savings become even more compelling.

## Scheme Filter Savings Alone

Indexing `scheme_id` is still worth keeping, but by itself it is not enough.

- If almost all traffic uses one scheme, scheme filtering saves almost nothing.
- If traffic splits evenly across two schemes, it saves about `2x`.
- The major gain only appears when we index a recipient-side partition key such as `view_tag_bucket`.

## Backward Compatibility

This is scheme-breaking because clients interpret event layout by scheme.

Two paths were considered:

- A: Bump `scheme_id` from `1` to `2`, deploy a new announcer contract, and have SDKs read both during a transition window.
- B: Upgrade the existing contract in place and update SDKs lock-step.

## Recommendation

Choose path A.

Why:

- It avoids ambiguity about how to decode old versus new announcements.
- It avoids coupling rollout timing to governance and contract upgrade coordination.
- It lets indexers and wallets dual-read `scheme_id = 1` and `scheme_id = 2` safely during migration.
- It creates a clean operational cutover: old events remain v1 forever, new events are explicitly v2 forever.

Path B is operationally riskier because historical and new events would come from the same contract address with different semantics across time, which makes replay, backfill, and indexer behavior easier to get wrong.

## Privacy Trade-Off

Adding `view_tag_bucket` to a public indexed topic leaks one byte of public correlation per payment.

Concretely:

- Observers can group announcements into 256 recipient-like buckets.
- They still cannot directly recover the recipient or spending keys from that bucket.
- They can, however, tell that two announcements landed in the same coarse bucket.

This is a real privacy cost and should be documented explicitly in the SDK and contract docs.

The trade-off is still favorable in practice because:

- Wallets already need a scan hint to avoid downloading the full stream.
- A 256-bucket partition leaks far less than naive full-stream client-side scanning at scale, where infra providers or intermediaries may observe every fetch anyway.
- Without server-side partitioning, scan costs may become high enough that wallets centralize around trusted indexers, which is also a privacy loss.

This should not be described as "free" privacy-wise. It is a deliberate performance-for-correlation trade.

## Recommendation Summary

Do this.

Specifically:

- Introduce a v2 Stellar announcer event schema using indexed topics `("announce", scheme_id, view_tag_bucket, metadata_kind)`.
- Set `scheme_id = 2` for the new Stellar scheme.
- Keep v1 readable during transition.
- Update SDK fetch logic to query by bucket at the RPC layer.

## Follow-Up Work

The implementation should be split into contract and SDK work:

- Contract: add the v2 topic layout and a stable definition of `view_tag_bucket` and `metadata_kind`.
- SDK: query `getEvents` with topic filters on `(announce, 2, bucket, *)`, then perform the normal cryptographic validation client-side.

Filed follow-up issues:

- `#24` for the Stellar contract-side v2 event schema implementation
- `#25` for the SDK/RPC fetch-path update

## Source Notes

This recommendation relies on Stellar RPC's documented `getEvents` filtering model:

- filters must match both contract id and topic
- each topic filter supports one to four segment matchers
- event topics themselves are limited to four items

That four-slot limit is the reason the design prioritizes `scheme_id` and `view_tag_bucket` over `stealth_address`.
