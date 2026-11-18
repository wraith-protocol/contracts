//! Malformed-witness, type-script, cell-dep, duplicated-output, and integer-boundary
//! fuzz corpus & minimized regression tests for CKB scripts (Issue #281).
//!
//! Executes the compiled RISC-V `wraith-stealth-lock` and `wraith-names-type`
//! binaries inside `ckb-testtool`'s in-process CKB VM to guarantee every
//! malformed transaction is rejected cleanly without panic or out-of-bounds access.

use ckb_testtool::ckb_types::bytes::Bytes;
use ckb_testtool::ckb_types::core::{DepType, ScriptHashType, TransactionBuilder};
use ckb_testtool::ckb_types::packed::{
    Byte32, CellDepBuilder, CellInput, CellOutput, Script, WitnessArgs,
};
use ckb_testtool::ckb_types::prelude::*;
use wraith_stealth_lock_integration::{
    exit_code, names_exit_code, require_executable_vm, script_exit_code, LockHarness,
    NamesTypeHarness, ARGS_LEN, NAMES_ARGS_LEN, NAMES_DATA_LEN, SIGNATURE_LEN,
};

const CELL_CAPACITY: u64 = 500_000_000_000;
const MIN_CAPACITY: u64 = 61_000_000_000;
const MAX_CYCLES: u64 = 70_000_000;

/// Assert that a verification failure came from a clean script exit code rather
/// than a CKB VM panic, trap (`ebreak` / `InvalidInstruction`), or OOB memory fault.
fn assert_clean_script_rejection(err_str: &str, code: Option<i8>) {
    assert!(
        !err_str.contains("InvalidInstruction")
            && !err_str.contains("MemOutOfBound")
            && !err_str.contains("MemWriteOnExecutablePage"),
        "script panicked or faulted in CKB VM instead of returning an error code: {err_str}"
    );
    assert!(
        code.is_some(),
        "expected a deterministic script exit code, got VM/consensus error: {err_str}"
    );
}

// ── 1. Truncation at every field boundary ─────────────────────────────────────

#[test]
fn corpus_truncation_at_every_molecule_witness_and_lock_boundary() {
    require_executable_vm!();

    // Molecule WitnessArgs boundaries:
    //   0..4 bytes   = truncated total_size u32 header
    //   4..16 bytes  = truncated 3-field offset table (lock, input_type, output_type)
    //   16..20 bytes = truncated BytesOpt inner u32 length prefix
    let truncated_witnesses: Vec<Vec<u8>> = vec![
        vec![],
        vec![0x01],
        vec![0x04, 0x00, 0x00],
        vec![0x10, 0x00, 0x00, 0x00, 0x10, 0x00],
        vec![0x14, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x14, 0x00, 0x00, 0x00],
        // Declared total_size = 85 bytes, actual slice = 18 bytes
        vec![
            85, 0, 0, 0, 16, 0, 0, 0, 85, 0, 0, 0, 85, 0, 0, 0, 65, 0,
        ],
    ];

    for (idx, raw) in truncated_witnesses.iter().enumerate() {
        let mut harness = LockHarness::new();
        let cell = harness.create_locked_cell(CELL_CAPACITY);
        let tx = harness.build_unlock_with_raw_witnesses(
            cell,
            std::slice::from_ref(raw),
            CELL_CAPACITY,
        );
        let failure = harness
            .verify(&tx, MAX_CYCLES, &format!("witness-trunc-{idx}"))
            .expect_err("truncated WitnessArgs must be rejected");
        let code = script_exit_code(&failure.error);
        assert_clean_script_rejection(&failure.error.to_string(), code);
        assert!(
            matches!(code, Some(exit_code::ENCODING | exit_code::SIGNATURE_LENGTH)),
            "unexpected exit code {code:?} on truncated witness #{idx}"
        );
    }

    // Lock signature length boundaries: 0, 1, 32 (r-only), 33, 64 (missing recid)
    for sig_len in [0usize, 1, 32, 33, SIGNATURE_LEN - 1] {
        let mut harness = LockHarness::new();
        let cell = harness.create_locked_cell(CELL_CAPACITY);
        let tx = harness.build_unlock(cell, &vec![0x42u8; sig_len], CELL_CAPACITY, false);
        let failure = harness
            .verify(&tx, MAX_CYCLES, &format!("sig-trunc-{sig_len}"))
            .expect_err("truncated signature must be rejected");
        assert_eq!(script_exit_code(&failure.error), Some(exit_code::SIGNATURE_LENGTH));
    }

    // Stealth lock args field boundaries: 0, 1, 20, 32, 33 (ephemeral only), 52
    for args_len in [0usize, 1, 20, 32, 33, ARGS_LEN - 1] {
        let mut harness = LockHarness::new();
        let truncated_args = harness.args[..args_len].to_vec();
        let script = harness
            .context
            .build_script_with_hash_type(
                &harness.lock_out_point,
                harness.hash_type,
                Bytes::from(truncated_args.clone()),
            )
            .expect("lock deployed")
            .as_builder()
            .args(truncated_args.pack())
            .build();
        let cell = harness.create_locked_cell_with_script(CELL_CAPACITY, script);
        let tx = harness.build_unlock(
            cell,
            &LockHarness::well_formed_signature(),
            CELL_CAPACITY,
            false,
        );
        let failure = harness
            .verify(&tx, MAX_CYCLES, &format!("lock-args-trunc-{args_len}"))
            .expect_err("truncated lock args must be rejected");
        assert_eq!(script_exit_code(&failure.error), Some(exit_code::ARGS_LENGTH));
    }
}

#[test]
fn corpus_truncation_at_names_type_args_and_cell_data_boundaries() {
    require_executable_vm!();

    // Names type args boundaries: 0, 1, 16, 31 bytes (expected 32)
    for args_len in [0usize, 1, 16, NAMES_ARGS_LEN - 1] {
        let mut harness = NamesTypeHarness::new();
        let tx = harness.build_name_tx(
            &vec![0xABu8; args_len],
            &[],
            &[NamesTypeHarness::valid_name_data()],
            0,
            CELL_CAPACITY,
        );
        let failure = harness
            .verify(&tx, MAX_CYCLES, &format!("names-args-trunc-{args_len}"))
            .expect_err("truncated names type args must be rejected");
        assert_eq!(
            script_exit_code(&failure.error),
            Some(names_exit_code::INVALID_ARGS_LENGTH)
        );
    }

    // Names cell data boundaries: 0, 1, 32, 33 (spending key only), 65 bytes (expected 66)
    for data_len in [0usize, 1, 32, 33, NAMES_DATA_LEN - 1] {
        let mut harness = NamesTypeHarness::new();
        let args = harness.default_name_args.clone();
        let tx = harness.build_name_tx(
            &args,
            &[],
            &[vec![0x02u8; data_len]],
            0,
            CELL_CAPACITY,
        );
        let failure = harness
            .verify(&tx, MAX_CYCLES, &format!("names-data-trunc-{data_len}"))
            .expect_err("truncated names cell data must be rejected");
        assert_eq!(
            script_exit_code(&failure.error),
            Some(names_exit_code::INVALID_DATA_LENGTH)
        );
    }
}

// ── 2. Excess bytes ───────────────────────────────────────────────────────────

#[test]
fn corpus_excess_bytes_rejected_across_witness_lock_and_names_type() {
    require_executable_vm!();

    // Valid WitnessArgs followed by trailing unreferenced bytes (violates Molecule total_size)
    let valid_witness = WitnessArgs::new_builder()
        .lock(Some(Bytes::from(LockHarness::well_formed_signature())).pack())
        .build();
    let mut trailing = valid_witness.as_bytes().to_vec();
    trailing.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);

    let mut harness = LockHarness::new();
    let cell = harness.create_locked_cell(CELL_CAPACITY);
    let tx = harness.build_unlock_with_raw_witnesses(cell, &[trailing], CELL_CAPACITY);
    let failure = harness
        .verify(&tx, MAX_CYCLES, "witness-trailing-excess")
        .expect_err("WitnessArgs with trailing excess bytes must be rejected");
    assert_eq!(script_exit_code(&failure.error), Some(exit_code::ENCODING));

    // Overlong signatures: 66, 130, 512 bytes
    for sig_len in [SIGNATURE_LEN + 1, SIGNATURE_LEN * 2, 512] {
        let mut lock_h = LockHarness::new();
        let locked = lock_h.create_locked_cell(CELL_CAPACITY);
        let tx = lock_h.build_unlock(locked, &vec![0x55u8; sig_len], CELL_CAPACITY, false);
        let err = lock_h
            .verify(&tx, MAX_CYCLES, &format!("sig-excess-{sig_len}"))
            .expect_err("excess signature bytes must be rejected");
        assert_eq!(script_exit_code(&err.error), Some(exit_code::SIGNATURE_LENGTH));
    }

    // Overlong stealth lock args: 54, 85, 256 bytes
    for args_len in [ARGS_LEN + 1, 85, 256] {
        let mut lock_h = LockHarness::new();
        let long_args = vec![0x77u8; args_len];
        let script = lock_h
            .context
            .build_script_with_hash_type(
                &lock_h.lock_out_point,
                lock_h.hash_type,
                Bytes::from(long_args.clone()),
            )
            .expect("lock deployed")
            .as_builder()
            .args(long_args.pack())
            .build();
        let locked = lock_h.create_locked_cell_with_script(CELL_CAPACITY, script);
        let tx = lock_h.build_unlock(
            locked,
            &LockHarness::well_formed_signature(),
            CELL_CAPACITY,
            false,
        );
        let err = lock_h
            .verify(&tx, MAX_CYCLES, &format!("lock-args-excess-{args_len}"))
            .expect_err("excess lock args must be rejected");
        assert_eq!(script_exit_code(&err.error), Some(exit_code::ARGS_LENGTH));
    }

    // Overlong names type args (33, 64) and cell data (67, 132)
    for args_len in [NAMES_ARGS_LEN + 1, 64] {
        let mut names_h = NamesTypeHarness::new();
        let tx = names_h.build_name_tx(
            &vec![0xCDu8; args_len],
            &[],
            &[NamesTypeHarness::valid_name_data()],
            0,
            CELL_CAPACITY,
        );
        let err = names_h
            .verify(&tx, MAX_CYCLES, &format!("names-args-excess-{args_len}"))
            .expect_err("excess names type args must be rejected");
        assert_eq!(
            script_exit_code(&err.error),
            Some(names_exit_code::INVALID_ARGS_LENGTH)
        );
    }

    for data_len in [NAMES_DATA_LEN + 1, 132] {
        let mut names_h = NamesTypeHarness::new();
        let args = names_h.default_name_args.clone();
        let tx = names_h.build_name_tx(&args, &[], &[vec![0x02u8; data_len]], 0, CELL_CAPACITY);
        let err = names_h
            .verify(&tx, MAX_CYCLES, &format!("names-data-excess-{data_len}"))
            .expect_err("excess names cell data must be rejected");
        assert_eq!(
            script_exit_code(&err.error),
            Some(names_exit_code::INVALID_DATA_LENGTH)
        );
    }
}

// ── 3. Duplicated outputs & singleton cell regressions (Minimized) ────────────

#[test]
fn regression_names_type_rejects_duplicated_outputs_and_inputs_without_panic() {
    require_executable_vm!();

    // Valid baseline: Create (0 -> 1), Update (1 -> 1), Destroy (1 -> 0), and
    // Create alongside 2 unrelated plain change outputs all succeed.
    {
        let mut h = NamesTypeHarness::new();
        let args = h.default_name_args.clone();
        let valid = NamesTypeHarness::valid_name_data();

        let create_tx = h.build_name_tx(&args, &[], std::slice::from_ref(&valid), 2, CELL_CAPACITY);
        h.verify(&create_tx, MAX_CYCLES, "names-valid-create")
            .expect("single name output with plain change outputs must succeed");

        let update_tx = h.build_name_tx(
            &args,
            std::slice::from_ref(&valid),
            std::slice::from_ref(&valid),
            1,
            CELL_CAPACITY,
        );
        h.verify(&update_tx, MAX_CYCLES, "names-valid-update")
            .expect("1-to-1 name update must succeed");

        let destroy_tx = h.build_name_tx(&args, &[valid], &[], 1, CELL_CAPACITY);
        h.verify(&destroy_tx, MAX_CYCLES, "names-valid-destroy")
            .expect("1-to-0 name release must succeed");
    }

    // Minimized regression A: Create minting 2 identical `.wraith` name outputs (0 -> 2)
    {
        let mut h = NamesTypeHarness::new();
        let args = h.default_name_args.clone();
        let valid = NamesTypeHarness::valid_name_data();
        let dup_create = h.build_name_tx(
            &args,
            &[],
            &[valid.clone(), valid],
            0,
            CELL_CAPACITY,
        );
        let failure = h
            .verify(&dup_create, MAX_CYCLES, "regression-dup-create-outputs")
            .expect_err("duplicated name outputs on Create must be rejected");
        assert_eq!(
            script_exit_code(&failure.error),
            Some(names_exit_code::DUPLICATE_NAME_CELL)
        );
    }

    // Minimized regression B: Update splitting 1 name input into 2 identical name outputs (1 -> 2)
    {
        let mut h = NamesTypeHarness::new();
        let args = h.default_name_args.clone();
        let valid = NamesTypeHarness::valid_name_data();
        let dup_update = h.build_name_tx(
            &args,
            &[valid.clone()],
            &[valid.clone(), valid],
            0,
            CELL_CAPACITY,
        );
        let failure = h
            .verify(&dup_update, MAX_CYCLES, "regression-dup-update-outputs")
            .expect_err("duplicated name outputs on Update must be rejected");
        assert_eq!(
            script_exit_code(&failure.error),
            Some(names_exit_code::DUPLICATE_NAME_CELL)
        );
    }

    // Minimized regression C: Coalescing 2 identical name inputs (2 -> 1)
    {
        let mut h = NamesTypeHarness::new();
        let args = h.default_name_args.clone();
        let valid = NamesTypeHarness::valid_name_data();
        let dup_inputs = h.build_name_tx(
            &args,
            &[valid.clone(), valid.clone()],
            &[valid],
            0,
            CELL_CAPACITY,
        );
        let failure = h
            .verify(&dup_inputs, MAX_CYCLES, "regression-dup-inputs")
            .expect_err("duplicate name inputs must be rejected");
        assert_eq!(
            script_exit_code(&failure.error),
            Some(names_exit_code::DUPLICATE_NAME_CELL)
        );
    }
}

// ── 4. Wrong dep / type hashes & NoMatchingCells lock-invocation guard ────────

#[test]
fn corpus_wrong_dep_and_type_hashes_and_non_matching_invocation_rejected() {
    require_executable_vm!();

    // Case A: Invoking `wraith-names-type` as a cell's LOCK script (so 0 input/output
    // cells carry its type_hash). Previously panicked with `(false, false) => panic!(...)`;
    // now cleanly returns `NO_MATCHING_CELLS` (8) without VM trap!
    {
        let mut h = NamesTypeHarness::new();
        let args = h.default_name_args.clone();
        let misused_as_lock = h.names_type_script(&args);
        let cell_out = CellOutput::new_builder()
            .capacity(CELL_CAPACITY)
            .lock(misused_as_lock)
            .build();
        let op = h.context.create_cell(cell_out, Bytes::new());
        let out = CellOutput::new_builder()
            .capacity(CELL_CAPACITY)
            .lock(h.owner_lock())
            .build();
        let tx = TransactionBuilder::default()
            .input(CellInput::new_builder().previous_output(op).build())
            .output(out)
            .outputs_data(vec![Bytes::new()].pack())
            .witness(Bytes::new())
            .build();
        let tx = h.context.complete_tx(tx);
        let failure = h
            .verify(&tx, MAX_CYCLES, "regression-names-invoked-as-lock")
            .expect_err("names-type invoked as lock with 0 matching type cells must fail cleanly");
        let code = script_exit_code(&failure.error);
        assert_clean_script_rejection(&failure.error.to_string(), code);
        assert_eq!(code, Some(names_exit_code::NO_MATCHING_CELLS));
    }

    // Case B: Unresolved bogus code_hash on lock script (missing cell dep)
    {
        let mut h = LockHarness::new();
        let bogus_script = Script::new_builder()
            .code_hash(Byte32::from_slice(&[0xEEu8; 32]).unwrap())
            .hash_type(ScriptHashType::Data2)
            .args(h.args.clone().pack())
            .build();
        let cell = h.create_locked_cell_with_script(CELL_CAPACITY, bogus_script);
        let out = CellOutput::new_builder()
            .capacity(CELL_CAPACITY)
            .lock(h.recipient_lock())
            .build();
        let fake_dep = CellDepBuilder::default()
            .out_point(h.lock_out_point.clone())
            .dep_type(DepType::Code)
            .build();
        let tx = TransactionBuilder::default()
            .cell_dep(fake_dep)
            .input(CellInput::new_builder().previous_output(cell).build())
            .output(out)
            .outputs_data(vec![Bytes::new()].pack())
            .witness(Bytes::from(LockHarness::well_formed_signature()))
            .build();
        assert!(
            h.context.verify_tx(&tx, MAX_CYCLES).is_err(),
            "script referencing an undeployed code_hash must be rejected"
        );
    }
}

// ── 5. Integer boundaries & bounded deterministic VM fuzz sample ──────────────

#[test]
fn corpus_integer_boundaries_on_molecule_offsets_and_capacities() {
    require_executable_vm!();

    let mut bad_molecules: Vec<Vec<u8>> = Vec::new();

    // u32::MAX total length header
    let mut max_total = u32::MAX.to_le_bytes().to_vec();
    max_total.extend_from_slice(&[16, 0, 0, 0, 16, 0, 0, 0, 16, 0, 0, 0]);
    bad_molecules.push(max_total);

    // Non-monotonic backward offsets: offset_0 = 20, offset_1 = 16, offset_2 = 20
    let mut backward = Vec::new();
    backward.extend_from_slice(&20u32.to_le_bytes());
    backward.extend_from_slice(&20u32.to_le_bytes());
    backward.extend_from_slice(&16u32.to_le_bytes());
    backward.extend_from_slice(&20u32.to_le_bytes());
    backward.extend_from_slice(&[0u8; 4]);
    bad_molecules.push(backward);

    // Offset pointing into the 4-byte total_size header itself (offset_0 = 2)
    let mut into_header = Vec::new();
    into_header.extend_from_slice(&16u32.to_le_bytes());
    into_header.extend_from_slice(&2u32.to_le_bytes());
    into_header.extend_from_slice(&16u32.to_le_bytes());
    into_header.extend_from_slice(&16u32.to_le_bytes());
    bad_molecules.push(into_header);

    // Bytes inner length = u32::MAX inside a 20-byte WitnessArgs
    let mut inner_overflow = Vec::new();
    inner_overflow.extend_from_slice(&20u32.to_le_bytes());
    inner_overflow.extend_from_slice(&16u32.to_le_bytes());
    inner_overflow.extend_from_slice(&20u32.to_le_bytes());
    inner_overflow.extend_from_slice(&20u32.to_le_bytes());
    inner_overflow.extend_from_slice(&u32::MAX.to_le_bytes());
    bad_molecules.push(inner_overflow);

    for (idx, payload) in bad_molecules.iter().enumerate() {
        let mut h = LockHarness::new();
        let cell = h.create_locked_cell(MIN_CAPACITY);
        let tx = h.build_unlock_with_raw_witnesses(
            cell,
            std::slice::from_ref(payload),
            MIN_CAPACITY,
        );
        let failure = h
            .verify(&tx, MAX_CYCLES, &format!("int-boundary-mol-{idx}"))
            .expect_err("corrupted Molecule integer offsets must be rejected");
        let code = script_exit_code(&failure.error);
        assert_clean_script_rejection(&failure.error.to_string(), code);
        assert_eq!(code, Some(exit_code::ENCODING));
    }

    // u64::MAX output capacity overflow attempt
    {
        let mut h = LockHarness::new();
        let cell = h.create_locked_cell(MIN_CAPACITY);
        let tx = h.build_unlock(cell, &LockHarness::well_formed_signature(), u64::MAX, false);
        assert!(h.context.verify_tx(&tx, MAX_CYCLES).is_err());
    }
}

struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn gen_bytes(&mut self, len: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(len);
        while out.len() < len {
            out.extend_from_slice(&self.next_u64().to_le_bytes());
        }
        out.truncate(len);
        out
    }
}

#[test]
fn bounded_deterministic_vm_fuzz_sample_never_panics() {
    require_executable_vm!();

    let iterations: usize = std::env::var("WRAITH_CKB_VM_FUZZ_ITERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(128);

    let mut rng = SplitMix64(0x5752_4149_5448_0281);

    for step in 0..iterations {
        // Alternate between stealth-lock and names-type VM executions
        if step % 2 == 0 {
            let mut h = LockHarness::new();
            let args_len = (rng.next_u64() as usize) % 96;
            let wit_len = (rng.next_u64() as usize) % 160;
            let raw_args = rng.gen_bytes(args_len);
            let raw_wit = rng.gen_bytes(wit_len);

            let script = h
                .context
                .build_script_with_hash_type(
                    &h.lock_out_point,
                    h.hash_type,
                    Bytes::from(raw_args.clone()),
                )
                .expect("lock deployed")
                .as_builder()
                .args(raw_args.pack())
                .build();
            let cell = h.create_locked_cell_with_script(CELL_CAPACITY, script);
            let witnesses = if step % 8 == 0 { vec![] } else { vec![raw_wit] };
            let tx = h.build_unlock_with_raw_witnesses(cell, &witnesses, CELL_CAPACITY);

            if let Err(err) = h.context.verify_tx(&tx, MAX_CYCLES) {
                let code = script_exit_code(&err);
                assert_clean_script_rejection(&err.to_string(), code);
            }
        } else {
            let mut h = NamesTypeHarness::new();
            let args_len = match step % 5 {
                0 => NAMES_ARGS_LEN,
                _ => (rng.next_u64() as usize) % 72,
            };
            let out_count = (rng.next_u64() as usize) % 4;
            let in_count = (rng.next_u64() as usize) % 3;
            let raw_args = rng.gen_bytes(args_len);
            let inputs: Vec<Vec<u8>> = (0..in_count)
                .map(|_| {
                    let len = (rng.next_u64() as usize) % 96;
                    rng.gen_bytes(len)
                })
                .collect();
            let outputs: Vec<Vec<u8>> = (0..out_count)
                .map(|_| {
                    let len = if step % 6 == 1 {
                        NAMES_DATA_LEN
                    } else {
                        (rng.next_u64() as usize) % 110
                    };
                    rng.gen_bytes(len)
                })
                .collect();

            let tx = h.build_name_tx(&raw_args, &inputs, &outputs, 1, CELL_CAPACITY);
            if let Err(err) = h.context.verify_tx(&tx, MAX_CYCLES) {
                let code = script_exit_code(&err);
                // When 0 inputs and 0 outputs carry the type script, CKB never invokes the type script at all (Ok).
                assert_clean_script_rejection(&err.to_string(), code);
            }
        }
    }
}
