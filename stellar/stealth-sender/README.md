# stealth-sender

Soroban contract for atomic asset transfer + stealth announcement on Stellar.

`init` stores the announcer address; `send` transfers tokens to a single
stealth address and emits one announcement; `batch_send` does the same for a
variable-length batch of parallel vectors (`stealth_addresses`,
`ephemeral_pub_keys`, `metadatas`, `amounts`), rejecting mismatched lengths.

```bash
cargo test          # unit tests (from stellar/)
```

## Fuzzing

The batch path accepts variable-length, caller-controlled payloads and is the
richest attack surface in this contract. A [`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz)
harness under [`fuzz/`](./fuzz) drives it with arbitrary input.

Because Soroban contracts run in Wasm against host objects rather than raw byte
buffers, `fuzz/src/lib.rs` reimplements the two byte-facing behaviours as a
self-contained pure-Rust model that mirrors the contract semantics.

### Targets

| Target | What it checks |
|---|---|
| `batch_decode` | Round-trips an arbitrary batch payload through the wire codec: decoding never panics or over-reads, and `decode → encode → decode` is a stable fixed point. |
| `batch_execute` | Runs arbitrary parallel batch vectors through the `batch_send` model and asserts its invariants: no event drift (one announcement per entry, in order), no index drift, and no silent over-write of accumulated recipient balances. |

The committed corpus lives under [`fuzz/corpus/`](./fuzz/corpus) (one directory
per target) and seeds each run with valid and boundary payloads plus
coverage-expanding inputs.

### Running locally

Requires a nightly toolchain and `cargo-fuzz`:

```bash
rustup toolchain install nightly
cargo install cargo-fuzz --locked

cd stellar/stealth-sender/fuzz
cargo +nightly fuzz run batch_decode      # or batch_execute
```

The model (without the fuzzer) can be exercised on stable:

```bash
cargo test --manifest-path stellar/stealth-sender/fuzz/Cargo.toml --lib
```

### CI

The `fuzz` job in [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml)
runs on a nightly schedule (03:00 UTC) and on manual `workflow_dispatch`, on the
Rust nightly toolchain, with a 30-minute budget split evenly across the two
targets (`-max_total_time=900` each). It is not run on every PR — a half-hour
fuzz run is too heavy for that. Crash inputs are uploaded as build artifacts on
failure.

### Reproducing a crash

When a run finds a crash, `cargo-fuzz` writes the offending input to
`fuzz/artifacts/<target>/crash-<hash>`. Replay it deterministically with:

```bash
cargo +nightly fuzz run <target> fuzz/artifacts/<target>/crash-<hash>
```

Commit the crash input into `fuzz/corpus/<target>/` so the case becomes a
permanent regression seed:

```bash
cp fuzz/artifacts/<target>/crash-<hash> fuzz/corpus/<target>/
```
