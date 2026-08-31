# Wraith Stellar Soroban Resource Budget Report

Measured on 2026-06-01 with `soroban-sdk = 22.0.0` resolved to `22.0.11`.
The reusable harness is in `stellar/bench/` and can be re-run with:

```sh
cargo bench -p wraith-stellar-bench --bench gas
# or: cargo run -p wraith-stellar-bench
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

<!-- BENCH:CURRENT:START -->
These are the harness results auto-updated from `develop` (measured 2026-08-31, commit `0cd83911c569`).

| Contract | Function | Parameters | Instructions | Mem bytes | Read entries | Write entries | Read bytes | Write bytes | Event bytes |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| stealth-announcer | announce | metadata_len=1 | 15096 | 1610 | 1 | 0 | 104 | 0 | 196 |
| stealth-announcer | announce | metadata_len=32 | 15096 | 1610 | 1 | 0 | 104 | 0 | 224 |
| stealth-announcer | announce | metadata_len=256 | 15096 | 1610 | 1 | 0 | 104 | 0 | 448 |
| stealth-announcer | announce | metadata_len=1024 | 15096 | 1610 | 1 | 0 | 104 | 0 | 1216 |
| stealth-announcer | announce | metadata_len=4096 | 15096 | 1610 | 1 | 0 | 104 | 0 | 4288 |
| stealth-registry | register_keys | first_time | 52852 | 7481 | 3 | 2 | 156 | 280 | 372 |
| stealth-registry | register_keys | replacement | 50934 | 6801 | 3 | 2 | 364 | 280 | 372 |
| stealth-sender | send | asset=xlm | 223328 | 32667 | 6 | 3 | 1232 | 520 | 964 |
| stealth-sender | send | asset=issued | 223280 | 32667 | 6 | 3 | 1232 | 520 | 964 |
| stealth-sender | batch_send | batch_size=1 | 232340 | 33332 | 6 | 3 | 1232 | 520 | 1216 |
| stealth-sender | batch_send | batch_size=5 | 853185 | 125400 | 6 | 7 | 1232 | 1416 | 3056 |
| stealth-sender | batch_send | batch_size=10 | 1674743 | 250790 | 6 | 12 | 1232 | 2536 | 5356 |
| stealth-sender | batch_send | batch_size=25 | 4352799 | 695660 | 6 | 27 | 1232 | 5896 | 12256 |
| stealth-sender | withdraw_many | entries=1 | 166455 | 24918 | 4 | 3 | 888 | 520 | 616 |
| stealth-sender | withdraw_many | entries=10 | 1478512 | 218040 | 4 | 12 | 888 | 2536 | 4756 |
| stealth-sender | withdraw_many | entries=30 | 4826570 | 760300 | 4 | 32 | 888 | 7016 | 13956 |
| wraith-names | register | name_len=3 | 88106 | 10944 | 2 | 3 | 104 | 568 | 348 |
| wraith-names | register | name_len=32 | 88654 | 10914 | 2 | 3 | 104 | 596 | 376 |
| wraith-names | resolve | hit | 39126 | 4010 | 2 | 0 | 440 | 0 | 144 |
| wraith-names | resolve | miss | 27723 | 2655 | 2 | 0 | 104 | 0 | 0 |
| wraith-names | name_of | hit | 49222 | 4865 | 3 | 0 | 604 | 0 | 0 |
| wraith-names | name_of | miss | 25374 | 2127 | 2 | 0 | 104 | 0 | 0 |
| wraith-names | bulk_register | count=5 | 537147 | 62539 | 2 | 11 | 104 | 2552 | 2060 |
| wraith-names | bulk_register | count=10 | 1181877 | 155599 | 2 | 21 | 104 | 5032 | 4000 |
| wraith-names | bulk_register | count=20 | 2811135 | 442669 | 2 | 41 | 104 | 9992 | 7880 |
| wraith-names | bulk_renew | count=5 | 351158 | 25489 | 11 | 0 | 2584 | 0 | 1032 |
| wraith-names | bulk_renew | count=10 | 718431 | 51559 | 21 | 0 | 5064 | 0 | 1812 |
| wraith-names | bulk_renew | count=20 | 1511175 | 111799 | 41 | 0 | 10024 | 0 | 3372 |
| wraith-names | extend_name_ttl | existing | 61349 | 5506 | 4 | 0 | 656 | 0 | 264 |
| stealth-splitter | create_split | beneficiaries=5 | 121823 | 14396 | 1 | 2 | 180 | 1300 | 544 |
| stealth-splitter | create_split | beneficiaries=15 | 230548 | 38036 | 1 | 2 | 180 | 2700 | 544 |
| stealth-splitter | create_split | beneficiaries=25 | 343273 | 77676 | 1 | 2 | 180 | 4100 | 544 |
| stealth-splitter | fund_split | beneficiaries=5 | 946972 | 146219 | 4 | 8 | 2116 | 2644 | 2848 |
| stealth-splitter | fund_split | beneficiaries=15 | 2755375 | 439179 | 4 | 18 | 3516 | 6284 | 7448 |
| stealth-splitter | fund_split | beneficiaries=25 | 4730305 | 777939 | 4 | 28 | 4916 | 9924 | 12048 |
| stealth-vault | deposit | asset=xlm | 262123 | 39207 | 6 | 4 | 1228 | 924 | 1084 |
| stealth-vault | deposit | asset=issued | 261883 | 39207 | 6 | 4 | 1228 | 924 | 1084 |
| stealth-vault | claim | unlocked | 201891 | 32853 | 4 | 4 | 1476 | 520 | 628 |
| stealth-vault | refund | depositor | 197513 | 30286 | 3 | 4 | 1608 | 520 | 628 |
| stealth-vault | refund_permissionless | keeper | 210287 | 32111 | 3 | 4 | 1608 | 520 | 628 |
| governance | propose | happy_path | 116975 | 18098 | 2 | 2 | 464 | 964 | 320 |
| governance | vote | happy_path | 236214 | 43742 | 4 | 3 | 1636 | 1164 | 364 |
| governance | execute | happy_path | 164625 | 29805 | 1 | 2 | 1048 | 1040 | 256 |
<!-- BENCH:CURRENT:END -->

## Gas Regression Gate

CI compares PR bench results against the weekly-rotated baseline in
`stellar/bench/baseline.json` (also published as the `stellar-bench-baseline`
workflow artifact). A PR fails when any per-op `instructions` exceeds
baseline + 5%. Re-run locally:

```sh
cargo bench -p wraith-stellar-bench --bench gas -- --format json --out /tmp/bench.json
python3 bench/compare.py bench/baseline.json /tmp/bench.json
```

## Batch vs Individual Crossover

`stealth-batch-sender` and `stealth-sender` overlap for multi-recipient pays.
Partners keep asking: at what batch size does the dedicated batch contract become
cheaper than N individual `stealth-sender::send` calls?

Re-run with:

```sh
cargo bench -p wraith-stellar-bench-crossover --bench crossover
# or
cargo run -p wraith-stellar-bench-crossover
```

Chart data is checked into [`stellar/bench/data/`](bench/data/).

### Methodology

- Entry counts: 1, 2, 5, 10, 15, 20
- **Individual**: N isolated `stealth-sender::send` transactions (instructions
  summed; Soroban `resources()` only reports the last top-level invoke)
- **Batch**: one `stealth-batch-sender::batch_send` with N transfers
- Metrics: Soroban instruction count (fee-relevant "gas") and wall-clock ns,
  reported total and per-entry
- Setup (register / init / mint) is excluded from the metered window

### Results

<!-- CROSSOVER_TABLE_START -->
| N | Individual instr | Batch instr | Indiv /entry | Batch /entry | Individual ns | Batch ns | Indiv ns/entry | Batch ns/entry | Winner |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| 1 | 216310 | 182837 | 216310 | 182837 | 782800 | 575600 | 782800 | 575600 | batch |
| 2 | 432620 | 320986 | 216310 | 160493 | 1002900 | 412000 | 501450 | 206000 | batch |
| 5 | 1081550 | 743151 | 216310 | 148630 | 1992000 | 1003000 | 398400 | 200600 | batch |
| 10 | 2163100 | 1496019 | 216310 | 149601 | 4135300 | 2783400 | 413530 | 278340 | batch |
| 15 | 3244650 | 2292557 | 216310 | 152837 | 7869400 | 6136100 | 524626 | 409073 | batch |
| 20 | 4326200 | 3103832 | 216310 | 155191 | 12951500 | 6341200 | 647575 | 317060 | batch |
<!-- CROSSOVER_TABLE_END -->

### Crossover Point

<!-- CROSSOVER_SUMMARY_START -->
Instruction crossover: `stealth-batch-sender` becomes cheaper at **N = 1**.

Batch stays cheaper across the full measured range. Per-entry batch cost falls
from ~183k instructions at N=1 toward ~149–155k as N grows, while individual
`stealth-sender::send` stays flat at 216,310 instructions per entry (it always
pays for a cross-contract `announce` invoke). Prefer `stealth-batch-sender`
whenever more than zero recipients share a payment.
<!-- CROSSOVER_SUMMARY_END -->

### Chart

<!-- CROSSOVER_CHART_START -->
```mermaid
xychart-beta
    title "Instructions: individual send vs batch send"
    x-axis [1, 2, 5, 10, 15, 20]
    y-axis "Instructions"
    line "individual (N x send)" [216310, 432620, 1081550, 2163100, 3244650, 4326200]
    line "batch (batch_send)" [182837, 320986, 743151, 1496019, 2292557, 3103832]
```
<!-- CROSSOVER_CHART_END -->

Raw CSV: [`bench/data/crossover.csv`](bench/data/crossover.csv).
Mermaid source: [`bench/data/crossover-chart.md`](bench/data/crossover-chart.md).

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

## Concrete Diff Suggestions

1. Batch send: add a dedicated announcer batch API and invoke it once after all
   transfers. This requires extending `stealth-announcer`, so it is a protocol
   change and was not landed here.
2. Metadata: enforce a product-level metadata size cap, or store only compact
   view-tag payloads in the event. This is semantics-affecting for callers that
   rely on arbitrary metadata and should be decided at the protocol layer.
3. Reverse lookup: store the name string directly in `DataKey::Reverse`. This is
   the optimization implemented here.

