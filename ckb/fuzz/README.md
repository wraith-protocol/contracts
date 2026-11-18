# CKB Malformed-Witness & Type-Script Fuzz Suite (`wraith-ckb-fuzz`)

Deterministic fuzzing harness and retained seed/regression corpus for Wraith's CKB scripts (`wraith-stealth-lock` and `wraith-names-type`, Issue #281).

## Fuzz Targets

| Target | Attack Surface Covered |
|--------|------------------------|
| `lock_witness_and_args` | Molecule `WitnessArgs` wire header/offset/length parsing, trailing excess bytes, `u32::MAX` offset boundaries, and 53-byte `wraith-stealth-lock` args (`ephemeral_pubkey[33] \|\| pubkey_hash[20]`). |
| `names_type_script` | `wraith-names-type` 32-byte `blake2b(name)` args, 66-byte output cell data (`spending_pk[33] \|\| viewing_pk[33]`), singleton cell uniqueness (`<= 1` matching input and `<= 1` matching output), and zero-match lock-script misuse. |
| `ckb_auth_inputs` | `ckb-auth` IPC C-string argv (`algorithm_id`, `signature`, `message`, `pubkey_hash`), rejecting unsupported algorithm IDs (`0x01..=0xFF`), interior NUL bytes, odd/non-hex encodings, recovery IDs `>= 4`, and zero `r`/`s` scalars. |
| `cell_deps_and_announcement` | `CellDep` resolution (`CKB_AUTH_CODE_HASH`, `Data2` vs `Type`, unresolved `DepGroup`, wrong dep/type hashes, duplicate out-points) and canonical 61-byte + metadata stealth announcement args codec. |
| `ckb_tx_layout` | End-to-end multi-cell transaction invariants: `u64::checked_add` capacity overflow guards, minimum 61-CKB occupied capacity, `output_index` bounds, duplicated outputs, lock-group witness resolution, and cross-script dispatch. |

## Minimized Regressions Found & Retained

Every regression discovered while building the corpus is minimized into `corpus/<target>/regression_*` and verified inside the real RISC-V CKB VM in `ckb/integration-tests/tests/fuzz_corpus_regressions.rs`:

1. **Duplicated `.wraith` Name Cells (`wraith-names-type`)**: `has_cell_with_type_hash` previously collapsed cell counts to booleans `(has_input, has_output)`, allowing a single transaction to mint multiple identical `.wraith` name outputs (`0 -> 2`, `1 -> 2`) or coalesce duplicate inputs (`2 -> 1`). Fixed via `count_cells_with_type_hash` returning `Error::DuplicateNameCell` (`7`).
2. **Unchecked `wraith-names-type` Script Args Length**: Type script args shorter or longer than 32 bytes were previously accepted without validation. Fixed with `NAME_ARGS_LEN = 32` (`Error::InvalidArgsLength = 6`).
3. **VM Trap (`panic!`) When `wraith-names-type` Is Invoked Without Matching Type Cells**: Attaching `wraith-names-type` as a lock script triggered `(false, false) => panic!(...)` (an ungraceful VM abort) and allocated full cell data across unrelated outputs via `QueryIter::new(load_cell_data, Source::Output)`. Fixed to scan lightweight 32-byte `load_cell_type_hash` digests first and return `Error::NoMatchingCells` (`8`).
4. **Panic Arms in `From<SysError>` & `.unwrap()` on `CString::new`**: Replaced `panic!` arms in `From<SysError>` and `.unwrap()` calls in `wraith-stealth-lock` and `wraith-names-type` with deterministic exit codes (`Error::UnexpectedSysError` / `Error::Encoding`).

## Running Locally

```bash
# Deterministic corpus replay + bounded PR sample on stable Rust:
cd ckb/fuzz
WRAITH_CKB_FUZZ_CASES=4096 cargo test --release

# Real RISC-V CKB VM execution of all minimized regressions:
cd ckb
make test-ci

# Coverage-guided libFuzzer run (requires nightly toolchain + cargo-fuzz 0.12.0):
cd ckb/fuzz
cargo +nightly fuzz run lock_witness_and_args corpus/lock_witness_and_args -- -seed=281 -runs=10000
```
