# stealth-sender

Atomic asset transfer + stealth announcement for the Wraith protocol on Stellar/Soroban.

- `send` — transfer tokens to a single stealth address and emit an announcement via the configured `StealthAnnouncer` contract.
- `batch_send` — same as `send`, but for parallel vectors of stealth addresses, ephemeral public keys, metadata, and amounts. All four vectors must be the same length or the call is rejected with `SenderError::LengthMismatch`.

## Testing

```sh
cargo test
```

## Fuzzing

`batch_send` accepts variable-length batch payloads, which makes it the richest
attack surface in this contract. `fuzz/` contains two `cargo-fuzz` targets:

| Target | What it covers |
|---|---|
| `batch_decode` | Builds arbitrary-shaped (and often length-mismatched) batches, round-trips each container (`Vec<Address>`, `Vec<BytesN<32>>`, `Vec<Bytes>`, `Vec<i128>`) through the same XDR encoding a contract invocation argument goes through, and asserts the decoded value never drifts from the original and never panics. It does not call the contract — pure (de)serialization. |
| `batch_execute` | Deploys a real `StealthAnnouncer` and SAC token, then calls `batch_send` with arbitrary batches (including mismatched lengths, duplicate recipients, and negative/zero/large amounts). Asserts: the call never panics; on success, lengths matched and sender/recipient balances and the announcement-event count moved by exactly the batch amounts (no overwrites when a recipient repeats); on failure, balances and event count are byte-for-byte unchanged (no partial-batch drift). |

### Running locally

Requires the nightly toolchain and `cargo-fuzz`:

```sh
cargo install cargo-fuzz
cargo +nightly fuzz run batch_decode
cargo +nightly fuzz run batch_execute
```

Add `-- -max_total_time=<seconds>` to bound a run.

### Reproducing a crash

A crashing input is written to `fuzz/artifacts/<target>/crash-<hash>`. Reproduce it with:

```sh
cargo +nightly fuzz run <target> fuzz/artifacts/<target>/crash-<hash>
```

Once triaged, copy the crashing file into `fuzz/corpus/<target>/` and commit it so the
regression is fuzzed on every future run.

### CI

A scheduled job (`stellar-fuzz` in `.github/workflows/ci.yml`) runs both targets nightly,
each bounded to 15 minutes (30 minutes total), and uploads any crash artifacts for triage.
It can also be triggered manually via `workflow_dispatch`.

### Corpus

`fuzz/corpus/` holds a seed corpus (minimized with `cargo fuzz cmin`) for both targets,
committed so CI and local runs start from good coverage instead of from scratch.
