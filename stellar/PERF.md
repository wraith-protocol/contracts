# Wraith Stellar Soroban Resource Budget Report

Measured on 2026-06-01 with `soroban-sdk = 22.0.0` resolved to `22.0.11`.
The reusable harness is in `stellar/bench/` and can be re-run with:

```sh
cargo run -p wraith-stellar-bench
```

## How to Read the Units

Soroban metering separates execution and ledger access. `instructions` are modeled
CPU instructions, `mem_bytes` are modeled memory bytes, `read_entries` and
`write_entries` count ledger entries touched, `read_bytes` and `write_bytes` are
the serialized ledger bytes read or written, and `event_bytes` is the serialized
contract event payload. Fees are computed from these dimensions using network
configuration, so an optimization can matter even when it only saves ledger bytes
or event bytes. The SDK's `Env::cost_estimate().resources()` reports resources
for the last top-level invocation in the test environment; production simulation
with Soroban RPC should still be used before setting transaction fees.

References:
- https://developers.stellar.org/docs/learn/encyclopedia/network-configuration/resource-and-fee-metering
- https://docs.rs/soroban-sdk/latest/soroban_sdk/struct.Env.html

## Baseline

These baseline numbers were captured before the optimization below.

| Contract | Function | Parameters | Instructions | Mem bytes | Read entries | Write entries | Read bytes | Write bytes | Event bytes |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| stealth-announcer | announce | metadata_len=0 | 15458 | 1666 | 1 | 0 | 104 | 0 | 216 |
| stealth-announcer | announce | metadata_len=32 | 15458 | 1666 | 1 | 0 | 104 | 0 | 248 |
| stealth-announcer | announce | metadata_len=256 | 15458 | 1666 | 1 | 0 | 104 | 0 | 472 |
| stealth-announcer | announce | metadata_len=1024 | 15458 | 1666 | 1 | 0 | 104 | 0 | 1240 |
| stealth-announcer | announce | metadata_len=4096 | 15458 | 1666 | 1 | 0 | 104 | 0 | 4312 |
| stealth-registry | register_keys | first_time | 33345 | 4461 | 1 | 2 | 104 | 332 | 188 |
| stealth-registry | register_keys | replacement | 44880 | 6553 | 1 | 2 | 260 | 332 | 188 |
| stealth-sender | send | asset=xlm | 182403 | 28137 | 5 | 3 | 1068 | 520 | 484 |
| stealth-sender | send | asset=issued | 182355 | 28137 | 5 | 3 | 1068 | 520 | 484 |
| stealth-sender | batch_send | batch_size=1 | 184674 | 28137 | 5 | 3 | 1068 | 520 | 484 |
| stealth-sender | batch_send | batch_size=5 | 807519 | 120229 | 5 | 7 | 1068 | 1416 | 2420 |
| stealth-sender | batch_send | batch_size=10 | 1633634 | 245649 | 5 | 12 | 1068 | 2536 | 4840 |
| stealth-sender | batch_send | batch_size=25 | 4322337 | 690609 | 5 | 27 | 1068 | 5896 | 12100 |
| stealth-sender | sponsored_announce | batch_size=1 | 233608 | 36052 | 7 | 4 | 1164 | 592 | 916 |
| stealth-sender | sponsored_announce | batch_size=5 | 1098506 | 193054 | 11 | 16 | 2060 | 2672 | 2852 |
| stealth-sender | sponsored_announce | batch_size=10 | 2445729 | 491659 | 16 | 31 | 3180 | 5272 | 5272 |
| stealth-sender | sponsored_announce | batch_size=20 | 6018914 | 1430044 | 26 | 61 | 5420 | 10472 | 10112 |
| wraith-names | register | name_len=3 | 59800 | 6269 | 1 | 2 | 104 | 544 | 204 |
| wraith-names | register | name_len=32 | 61413 | 6327 | 1 | 2 | 104 | 572 | 232 |
| wraith-names | resolve | hit | 46120 | 5537 | 1 | 0 | 476 | 0 | 0 |
| wraith-names | resolve | miss | 19766 | 1600 | 1 | 0 | 104 | 0 | 0 |
| wraith-names | name_of | hit | 50728 | 5554 | 1 | 0 | 476 | 0 | 0 |
| wraith-names | name_of | miss | 21581 | 1513 | 1 | 0 | 104 | 0 | 0 |

## Optimization Landed

`wraith-names::name_of()` previously stored `Reverse(meta_hash) -> name_hash`,
then loaded `Name(name_hash)` to return the human-readable name. The reverse map
now stores `Reverse(meta_hash) -> name`, removing the second lookup for reverse
resolution. This does not change public semantics for new deployments: register,
update, release, resolve, and name_of return the same values and enforce the same
ownership checks.

| Case | Instructions Before | Instructions After | Delta | Mem Before | Mem After | Delta | Read Bytes Before | Read Bytes After | Delta |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| wraith-names name_of hit | 50728 | 47042 | -3686 (-7.3%) | 5554 | 5383 | -171 (-3.1%) | 476 | 452 | -24 (-5.0%) |
| wraith-names register len=3 | 59800 | 59792 | -8 | 6269 | 6240 | -29 | 104 | 104 | 0 |
| wraith-names resolve hit | 46120 | 46096 | -24 | 5537 | 5456 | -81 | 476 | 452 | -24 |

## Current Numbers

These are the post-optimization harness results.

| Contract | Function | Parameters | Instructions | Mem bytes | Read entries | Write entries | Read bytes | Write bytes | Event bytes |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| stealth-announcer | announce | metadata_len=0 | 15458 | 1666 | 1 | 0 | 104 | 0 | 216 |
| stealth-announcer | announce | metadata_len=32 | 15458 | 1666 | 1 | 0 | 104 | 0 | 248 |
| stealth-announcer | announce | metadata_len=256 | 15458 | 1666 | 1 | 0 | 104 | 0 | 472 |
| stealth-announcer | announce | metadata_len=1024 | 15458 | 1666 | 1 | 0 | 104 | 0 | 1240 |
| stealth-announcer | announce | metadata_len=4096 | 15458 | 1666 | 1 | 0 | 104 | 0 | 4312 |
| stealth-registry | register_keys | first_time | 33345 | 4461 | 1 | 2 | 104 | 332 | 188 |
| stealth-registry | register_keys | replacement | 44880 | 6553 | 1 | 2 | 260 | 332 | 188 |
| stealth-sender | send | asset=xlm | 182403 | 28137 | 5 | 3 | 1068 | 520 | 484 |
| stealth-sender | send | asset=issued | 182355 | 28137 | 5 | 3 | 1068 | 520 | 484 |
| stealth-sender | batch_send | batch_size=1 | 184674 | 28137 | 5 | 3 | 1068 | 520 | 484 |
| stealth-sender | batch_send | batch_size=5 | 807519 | 120229 | 5 | 7 | 1068 | 1416 | 2420 |
| stealth-sender | batch_send | batch_size=10 | 1633634 | 245649 | 5 | 12 | 1068 | 2536 | 4840 |
| stealth-sender | batch_send | batch_size=25 | 4322337 | 690609 | 5 | 27 | 1068 | 5896 | 12100 |
| wraith-names | register | name_len=3 | 59792 | 6240 | 1 | 2 | 104 | 516 | 204 |
| wraith-names | register | name_len=32 | 61413 | 6327 | 1 | 2 | 104 | 572 | 232 |
| wraith-names | resolve | hit | 46096 | 5456 | 1 | 0 | 452 | 0 | 0 |
| wraith-names | resolve | miss | 19766 | 1600 | 1 | 0 | 104 | 0 | 0 |
| wraith-names | name_of | hit | 47042 | 5383 | 1 | 0 | 452 | 0 | 0 |
| wraith-names | name_of | miss | 21581 | 1513 | 1 | 0 | 104 | 0 | 0 |

## Top Optimization Opportunities

1. Reduce per-recipient `batch_send` overhead. Batch size 25 costs 4,322,337
   instructions and 690,609 memory bytes; the slope is dominated by repeated
   token transfers and cross-contract announcement calls. Expected savings:
   high for every multi-recipient payment.
2. Cap or compress announcement metadata. CPU is flat for `announce`, but event
   bytes scale directly from 216 bytes at empty metadata to 4,312 bytes at 4 KiB.
   Expected savings: high for users who attach large metadata.
3. Avoid the extra reverse lookup in `wraith-names::name_of`. Implemented in
   this PR. Expected savings: medium for reverse lookup-heavy clients.
4. Split name entry storage if resolve dominates. `resolve` loads the whole
   `NameEntry`, including owner and name, to return only the meta-address.
   Expected savings: medium, but it changes storage layout more broadly.
5. Avoid duplicate metadata/key vector traversal in `batch_send`. Current code
   validates lengths once and then does four indexed reads per recipient.
   Expected savings: low to medium; correctness and readability should be
   preserved.

## Sponsored Announcement Cost

The sponsored rows were measured with distinct authenticated senders per
entry. Average resources per entry are calculated from each row:

| Batch size | Instructions/entry | Mem bytes/entry | Event bytes/entry |
|---:|---:|---:|---:|
| 1 | 233608 | 36052 | 916 |
| 5 | 219701 | 38611 | 570 |
| 10 | 244573 | 49166 | 527 |
| 20 | 300946 | 71502 | 506 |

## Concrete Diff Suggestions

1. Batch send: add a dedicated announcer batch API and invoke it once after all
   transfers. This requires extending `stealth-announcer`, so it is a protocol
   change and was not landed here.
2. Metadata: enforce a product-level metadata size cap, or store only compact
   view-tag payloads in the event. This is semantics-affecting for callers that
   rely on arbitrary metadata and should be decided at the protocol layer.
3. Reverse lookup: store the name string directly in `DataKey::Reverse`. This is
   the optimization implemented here.

