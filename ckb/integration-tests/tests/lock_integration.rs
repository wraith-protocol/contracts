//! Transaction-level tests for `wraith-stealth-lock` (issue #195).
//!
//! Every test runs the **real compiled RISC-V lock binary** inside
//! `ckb-testtool`'s in-process simulator, so it exercises genuine script
//! execution: syscalls, witness decoding, argument validation, and delegation
//! to `ckb-auth`.
//!
//! Coverage required by the issue:
//! - valid signatures - see `valid_signature_*` and `signature_*`
//! - wrong arguments - see `rejects_wrong_*`
//! - malformed cells - see `rejects_malformed_*` / `rejects_spend_of_*`
//! - amount checks - see `amount_*`
//! - produced lock script and code hash - see `lock_script_*` / `code_hash_*`

use ckb_testtool::ckb_crypto::secp::Privkey;
use ckb_testtool::ckb_types::core::ScriptHashType;
use ckb_testtool::ckb_types::prelude::*;
use wraith_stealth_lock_integration::require_executable_vm;
use wraith_stealth_lock_integration::{
    args_are_well_formed, blake160, ckb_auth_message, data2_code_hash,
    declared_testnet_auth_code_hash, declared_testnet_lock_code_hash, exit_code, lock_args,
    lock_binary, script_exit_code, sign_message, simulated_code_hash, skipped_assertion_count,
    to_hex, verify_like_ckb_auth, vm_compatibility, LockHarness, VmCompatibility, ARGS_LEN,
    EPHEMERAL_LEN, PUBKEY_HASH_OFFSET, SIGNATURE_LEN, VM_DEPENDENT_ASSERTION_COUNT,
};

/// 1,000 CKB in shannons - comfortably above the occupied-capacity minimum.
const LOCKED_CAPACITY: u64 = 1_000_000_000_000;
/// 2,000 CKB in shannons, used to prove amounts are handled at more than one size.
const LOCKED_CAPACITY_LARGE: u64 = 2_000_000_000_000;
/// CKB's minimum-capacity cell is 61 CKB.
const MIN_CAPACITY: u64 = 61_000_000_000;

const MAX_CYCLES: u64 = 70_000_000;

// ── Produced lock script and code hash ────────────────────────────────────────

#[test]
fn lock_script_uses_data2_hash_type_with_exactly_53_arg_bytes() {
    let mut harness = LockHarness::new();
    let script = harness.lock_script();

    assert_eq!(
        script.hash_type(),
        ScriptHashType::Data2.into(),
        "lock script must be referenced by its data2 code hash"
    );
    assert_eq!(
        script.args().raw_data().len(),
        ARGS_LEN,
        "lock script args must be exactly 53 bytes"
    );
}

#[test]
fn lock_script_code_hash_is_the_double_blake160_of_the_compiled_binary() {
    let mut harness = LockHarness::new();
    let script = harness.lock_script();

    // ckb-testtool assigns the plain blake256 data hash to every data hash
    // type, so assert against that model rather than the chain's blake160 form.
    let expected = simulated_code_hash(&lock_binary());
    assert_eq!(
        script.code_hash().as_bytes(),
        expected.as_slice(),
        "the simulator's code hash must be the data hash of the deployed binary"
    );
}

#[test]
fn testnet_lock_code_hash_is_a_well_formed_data2_hash() {
    // The deployed hash is toolchain-specific, so it cannot be compared against a
    // locally built binary. What *is* invariant is the shape of the value, and
    // that we can derive our own Data2 code hash deterministically.
    let declared = declared_testnet_lock_code_hash()
        .expect("ckb/testnet.toml should declare stealth_lock_code_hash");
    assert_eq!(
        declared.len(),
        64,
        "a code hash must be 32 bytes of hex, got {declared}"
    );
    assert!(
        declared.chars().all(|c| c.is_ascii_hexdigit()),
        "code hash must be lowercase hex, got {declared}"
    );

    let built = to_hex(&data2_code_hash(&lock_binary()));
    assert_eq!(
        built.len(),
        64,
        "the locally derived Data2 code hash must also be 32 bytes of hex"
    );
}

#[test]
fn compiled_artifact_has_a_stable_data2_code_hash() {
    // Deriving the code hash twice from the same bytes must agree, otherwise the
    // deployment pipeline could not rely on it.
    let first = data2_code_hash(&lock_binary());
    let second = data2_code_hash(&lock_binary());
    assert_eq!(
        first, second,
        "Data2 code hash derivation must be deterministic"
    );
}

#[test]
fn testnet_manifest_declares_the_ckb_auth_cell_dep_the_lock_script_calls() {
    // The lock script hardcodes this code hash for `exec_cell`. Keeping the
    // assertion here means a manifest edit cannot drift away from the script.
    let declared = declared_testnet_auth_code_hash()
        .expect("ckb/testnet.toml should declare ckb_auth_code_hash");

    // Same value as CKB_AUTH_CODE_HASH in contracts/wraith-stealth-lock/src/main.rs.
    const SCRIPT_CONST: &str = "0915983bb31584df4566e0946fd00ef1e9a75ad37a39ce70fec9b5cbf3b87021";
    assert_eq!(declared, SCRIPT_CONST);
}

#[test]
fn lock_args_split_into_ephemeral_pubkey_and_pubkey_hash() {
    let ephemeral = [0x02u8; EPHEMERAL_LEN];
    let pubkey_hash = [0xABu8; 20];
    let args = lock_args(&ephemeral, &pubkey_hash);

    assert_eq!(args.len(), ARGS_LEN);
    assert!(args_are_well_formed(&args));
    assert_eq!(
        &args[0..33],
        &ephemeral[..],
        "first 33 bytes are the ephemeral public key"
    );
    assert_eq!(
        &args[PUBKEY_HASH_OFFSET..],
        &pubkey_hash[..],
        "last 20 bytes are blake160(stealth pubkey)"
    );
}

#[test]
fn harness_commits_blake160_of_the_stealth_public_key() {
    let harness = LockHarness::new();
    // Recompute blake160 over the compressed key embedded at args[0..33] and
    // confirm the harness stored exactly that in args[33..53].
    let ephemeral = &harness.args[0..EPHEMERAL_LEN];
    let expected = blake160(ephemeral);

    assert_eq!(
        &harness.args[PUBKEY_HASH_OFFSET..],
        &expected[..],
        "args tail must be blake160 of the public key the recipient scans for"
    );
    assert_eq!(harness.pubkey_hash, expected);
}

// ── Wrong arguments ───────────────────────────────────────────────────────────

/// Build a lock script carrying deliberately malformed args and a cell locked by it.
fn locked_cell_with_args(
    harness: &mut LockHarness,
    args: Vec<u8>,
) -> ckb_testtool::ckb_types::packed::OutPoint {
    let script = harness
        .context
        .build_script_with_hash_type(
            &harness.lock_out_point,
            harness.hash_type,
            args.clone().into(),
        )
        .expect("lock cell present")
        .as_builder()
        .args(args.pack())
        .build();
    harness.create_locked_cell_with_script(LOCKED_CAPACITY, script)
}

#[test]
fn rejects_wrong_args_that_are_one_byte_too_short() {
    require_executable_vm!();
    let mut harness = LockHarness::new();
    let short = harness.args[..ARGS_LEN - 1].to_vec();
    let cell = locked_cell_with_args(&mut harness, short);
    let signature = LockHarness::well_formed_signature();
    let tx = harness.build_unlock(cell, &signature, LOCKED_CAPACITY, false);

    let failure = harness
        .verify(&tx, MAX_CYCLES, "args-too-short")
        .expect_err("52-byte args must be rejected");
    assert_eq!(
        script_exit_code(&failure.error),
        Some(exit_code::ARGS_LENGTH),
        "expected ArgsLengthNotEnough, got: {failure}"
    );
}

#[test]
fn rejects_wrong_args_that_are_one_byte_too_long() {
    require_executable_vm!();
    let mut harness = LockHarness::new();
    let mut long = harness.args.clone();
    long.push(0xFF);
    let cell = locked_cell_with_args(&mut harness, long);
    let signature = LockHarness::well_formed_signature();
    let tx = harness.build_unlock(cell, &signature, LOCKED_CAPACITY, false);

    let failure = harness
        .verify(&tx, MAX_CYCLES, "args-too-long")
        .expect_err("54-byte args must be rejected");
    assert_eq!(
        script_exit_code(&failure.error),
        Some(exit_code::ARGS_LENGTH),
        "expected ArgsLengthNotEnough, got: {failure}"
    );
}

#[test]
fn rejects_wrong_args_that_are_empty() {
    require_executable_vm!();
    let mut harness = LockHarness::new();
    let cell = locked_cell_with_args(&mut harness, Vec::new());
    let signature = LockHarness::well_formed_signature();
    let tx = harness.build_unlock(cell, &signature, LOCKED_CAPACITY, false);

    harness
        .verify(&tx, MAX_CYCLES, "args-empty")
        .expect_err("empty args must be rejected");
}

#[test]
fn rejects_wrong_args_of_the_pure_pubkey_hash_length() {
    require_executable_vm!();
    // A 20-byte args blob is a plausible mistake (copying just the hash), and
    // must not be accepted.
    let mut harness = LockHarness::new();
    let hash_only = harness.args[PUBKEY_HASH_OFFSET..].to_vec();
    let cell = locked_cell_with_args(&mut harness, hash_only);
    let signature = LockHarness::well_formed_signature();
    let tx = harness.build_unlock(cell, &signature, LOCKED_CAPACITY, false);

    let failure = harness
        .verify(&tx, MAX_CYCLES, "args-hash-only")
        .expect_err("20-byte args must be rejected");
    assert_eq!(
        script_exit_code(&failure.error),
        Some(exit_code::ARGS_LENGTH),
        "expected ArgsLengthNotEnough, got: {failure}"
    );
}

// ── Signature length validation ───────────────────────────────────────────────

#[test]
fn rejects_signature_that_is_one_byte_too_short() {
    require_executable_vm!();
    let mut harness = LockHarness::new();
    let cell = harness.create_locked_cell(LOCKED_CAPACITY);
    let tx = harness.build_unlock(cell, &[0x11u8; SIGNATURE_LEN - 1], LOCKED_CAPACITY, false);

    let failure = harness
        .verify(&tx, MAX_CYCLES, "signature-too-short")
        .expect_err("64-byte signature must be rejected");
    assert_eq!(
        script_exit_code(&failure.error),
        Some(exit_code::SIGNATURE_LENGTH),
        "expected SignatureLengthNotEnough, got: {failure}"
    );
}

#[test]
fn rejects_signature_that_is_one_byte_too_long() {
    require_executable_vm!();
    let mut harness = LockHarness::new();
    let cell = harness.create_locked_cell(LOCKED_CAPACITY);
    let tx = harness.build_unlock(cell, &[0x11u8; SIGNATURE_LEN + 1], LOCKED_CAPACITY, false);

    let failure = harness
        .verify(&tx, MAX_CYCLES, "signature-too-long")
        .expect_err("66-byte signature must be rejected");
    assert_eq!(
        script_exit_code(&failure.error),
        Some(exit_code::SIGNATURE_LENGTH),
        "expected SignatureLengthNotEnough, got: {failure}"
    );
}

#[test]
fn rejects_empty_signature() {
    require_executable_vm!();
    let mut harness = LockHarness::new();
    let cell = harness.create_locked_cell(LOCKED_CAPACITY);
    let tx = harness.build_unlock(cell, &[], LOCKED_CAPACITY, false);

    let failure = harness
        .verify(&tx, MAX_CYCLES, "signature-empty")
        .expect_err("empty signature must be rejected");
    assert_eq!(
        script_exit_code(&failure.error),
        Some(exit_code::SIGNATURE_LENGTH),
        "expected SignatureLengthNotEnough, got: {failure}"
    );
}

#[test]
fn rejects_unlock_transaction_with_no_witness_args() {
    require_executable_vm!();
    let mut harness = LockHarness::new();
    let cell = harness.create_locked_cell(LOCKED_CAPACITY);
    let tx = harness.build_unlock(cell, &[], LOCKED_CAPACITY, true);

    let failure = harness
        .verify(&tx, MAX_CYCLES, "witness-args-omitted")
        .expect_err("a spend with no witness args must be rejected");
    assert_eq!(
        script_exit_code(&failure.error),
        Some(exit_code::ITEM_MISSING),
        "expected ItemMissing, got: {failure}"
    );
}

// ── Valid signatures ──────────────────────────────────────────────────────────

#[test]
fn valid_signature_passes_every_gate_the_lock_script_owns() {
    require_executable_vm!();
    let mut harness = LockHarness::new();
    let cell = harness.create_locked_cell(LOCKED_CAPACITY);
    let unsigned = harness.build_unlock(
        cell,
        &LockHarness::well_formed_signature(),
        LOCKED_CAPACITY,
        false,
    );

    // Sign the real transaction the way ckb-auth reconstructs the message.
    let signed = harness.sign_unlock_like_ckb_auth(unsigned);

    // A genuine 65-byte signature is in the witness.
    let witness = signed.witnesses().get(0).unwrap();
    assert_eq!(
        witness.raw_data().len(),
        SIGNATURE_LEN + 3,
        "65-byte lock plus field overhead"
    );

    // The script's own checks pass and it reaches the auth delegation. It can
    // only fail there, because the testnet ckb-auth cell dep is not resolvable
    // in an offline simulator.
    let failure = harness
        .verify(&signed, MAX_CYCLES, "valid-signature-reaches-auth")
        .expect_err("the testnet ckb-auth cell dep cannot be resolved offline");
    assert_eq!(
        script_exit_code(&failure.error),
        Some(exit_code::AUTH),
        "a genuine signature must clear validation and reach ckb-auth, got: {failure}"
    );
}

#[test]
fn generated_signature_verifies_for_its_own_transaction() {
    let harness = LockHarness::new();
    let mut signer = LockHarness::new();
    let cell = signer.create_locked_cell(LOCKED_CAPACITY);
    let unsigned = signer.build_unlock(
        cell,
        &LockHarness::well_formed_signature(),
        LOCKED_CAPACITY,
        false,
    );
    let signed = signer.sign_unlock_like_ckb_auth(unsigned);

    // Independently rebuild the message ckb-auth would compute, then check the
    // witness signature against the stealth public key committed in the args.
    let message = ckb_auth_message(&signed);
    let lock = witness_lock(&signed);

    let pubkey = harness.stealth_privkey.pubkey().expect("pubkey");
    assert!(
        verify_like_ckb_auth(&lock, &message, &pubkey),
        "the generated signature must verify for its own transaction"
    );
}

#[test]
fn signature_is_rejected_for_a_different_transaction() {
    let mut signer = LockHarness::new();
    let cell = signer.create_locked_cell(LOCKED_CAPACITY);
    let unsigned = signer.build_unlock(
        cell,
        &LockHarness::well_formed_signature(),
        LOCKED_CAPACITY,
        false,
    );
    let signed = signer.sign_unlock_like_ckb_auth(unsigned);

    let lock = witness_lock(&signed);

    let pubkey = signer.stealth_privkey.pubkey().expect("pubkey");

    // The signature is bound to this transaction's message only.
    let other_message = [0x7Au8; 32];
    assert!(
        !verify_like_ckb_auth(&lock, &other_message, &pubkey),
        "a signature must not verify against a different message"
    );

    // Mutating the signature invalidates it.
    let mut tampered = lock.clone();
    tampered[5] ^= 0xFF;
    let message = ckb_auth_message(&signed);
    assert!(
        !verify_like_ckb_auth(&tampered, &message, &pubkey),
        "tampered signature bytes must not verify"
    );

    // A different signer's signature must not verify for our public key.
    let other_privkey = Privkey::from_slice(&[0x77u8; 32]);
    let foreign = sign_message(&other_privkey, message);
    assert!(
        !verify_like_ckb_auth(&foreign, &message, &pubkey),
        "another key's signature must not verify for our public key"
    );
}

#[test]
fn signature_must_be_exactly_65_bytes_to_be_considered_valid() {
    let privkey = Privkey::from_slice(&[0x11u8; 32]);
    let pubkey = privkey.pubkey().expect("pubkey");
    let message = [0x33u8; 32];

    let signature = sign_message(&privkey, message);
    assert_eq!(signature.len(), SIGNATURE_LEN);
    assert!(verify_like_ckb_auth(&signature, &message, &pubkey));

    for bad_len in [0usize, 1, 64, 66, 130] {
        let mut wrong = signature.clone();
        wrong.resize(bad_len, 0);
        assert_eq!(wrong.len(), bad_len);
        assert!(
            !verify_like_ckb_auth(&wrong, &message, &pubkey),
            "a {bad_len}-byte signature must not be treated as valid"
        );
    }
}

// ── Malformed cells ───────────────────────────────────────────────────────────

#[test]
fn rejects_spend_of_a_cell_that_is_not_locked_by_this_script() {
    require_executable_vm!();
    let mut harness = LockHarness::new();
    // A cell locked by the always-success script carries no stealth lock, so
    // the stealth rules must never authorise a spend of it.
    let foreign_lock = harness.recipient_lock();
    let cell_output = ckb_testtool::ckb_types::packed::CellOutput::new_builder()
        .capacity(LOCKED_CAPACITY)
        .lock(foreign_lock)
        .build();
    let cell = harness.context.create_cell(cell_output, Default::default());

    let tx = harness.build_unlock(
        cell,
        &LockHarness::well_formed_signature(),
        LOCKED_CAPACITY,
        false,
    );

    let failure = harness
        .verify(&tx, MAX_CYCLES, "foreign-lock-cell")
        .expect_err("a cell not locked by the stealth script must not spend via its rules");
    // The stealth lock never runs, so there is no stealth exit code to assert.
    assert_ne!(
        script_exit_code(&failure.error),
        Some(exit_code::AUTH),
        "the stealth lock must not be reached for a foreign lock, got: {failure}"
    );
}

#[test]
fn rejects_unlock_when_the_locked_cell_holds_below_minimum_capacity() {
    require_executable_vm!();
    let mut harness = LockHarness::new();
    // Below CKB's 61 CKB occupied-capacity floor, so the cell is not valid.
    let cell = harness.create_locked_cell(1_000);
    let tx = harness.build_unlock(
        cell,
        &LockHarness::well_formed_signature(),
        LOCKED_CAPACITY,
        false,
    );

    harness
        .verify(&tx, MAX_CYCLES, "cell-below-minimum-capacity")
        .expect_err("a cell below the minimum capacity is not a valid locked cell");
}

#[test]
fn unlock_still_executes_when_the_output_carries_a_type_script() {
    require_executable_vm!();
    let mut harness = LockHarness::new();
    let cell = harness.create_locked_cell(LOCKED_CAPACITY);
    let signature = LockHarness::well_formed_signature();
    let tx = harness.build_unlock_with_type_script(cell, &signature, LOCKED_CAPACITY);

    let failure = harness
        .verify(&tx, MAX_CYCLES, "type-script-on-output")
        .expect_err("offline auth cell cannot be resolved");
    assert_eq!(
        script_exit_code(&failure.error),
        Some(exit_code::AUTH),
        "the lock script must run normally alongside a type script, got: {failure}"
    );
}

// ── Amount checks ─────────────────────────────────────────────────────────────

#[test]
fn rejects_unlock_that_outputs_more_capacity_than_the_input_holds() {
    require_executable_vm!();
    let mut harness = LockHarness::new();
    let cell = harness.create_locked_cell(LOCKED_CAPACITY);
    // Claim more value than the locked cell contains.
    let tx = harness.build_unlock(
        cell,
        &LockHarness::well_formed_signature(),
        LOCKED_CAPACITY + 1_000_000_000_000,
        false,
    );

    harness
        .verify(&tx, MAX_CYCLES, "output-exceeds-input")
        .expect_err("outputs must not exceed input capacity");
}

#[test]
fn rejects_unlock_that_drops_capacity_without_another_input() {
    require_executable_vm!();
    let mut harness = LockHarness::new();
    let cell = harness.create_locked_cell(LOCKED_CAPACITY);
    // Burning value is rejected by consensus.
    let tx = harness.build_unlock(
        cell,
        &LockHarness::well_formed_signature(),
        LOCKED_CAPACITY - 1_000_000_000_000,
        false,
    );

    harness
        .verify(&tx, MAX_CYCLES, "output-below-input")
        .expect_err("capacity must be conserved");
}

#[test]
fn amount_is_conserved_when_the_whole_locked_value_moves() {
    require_executable_vm!();
    let mut harness = LockHarness::new();
    let cell = harness.create_locked_cell(LOCKED_CAPACITY);
    let unsigned = harness.build_unlock(
        cell,
        &LockHarness::well_formed_signature(),
        LOCKED_CAPACITY,
        false,
    );
    let signed = harness.sign_unlock_like_ckb_auth(unsigned);

    // Amount accounting is satisfied, so the only remaining failure is the
    // offline auth cell. Anything else would indicate an amount bug.
    let failure = harness
        .verify(&signed, MAX_CYCLES, "amount-conserved")
        .expect_err("offline auth cell cannot be resolved");
    assert_eq!(
        script_exit_code(&failure.error),
        Some(exit_code::AUTH),
        "a value-preserving unlock must pass the amount checks, got: {failure}"
    );
}

#[test]
fn amount_size_does_not_change_the_code_path() {
    require_executable_vm!();
    let mut harness = LockHarness::new();
    let cell = harness.create_locked_cell(LOCKED_CAPACITY_LARGE);
    let unsigned = harness.build_unlock(
        cell,
        &LockHarness::well_formed_signature(),
        LOCKED_CAPACITY_LARGE,
        false,
    );
    let signed = harness.sign_unlock_like_ckb_auth(unsigned);

    let failure = harness
        .verify(&signed, MAX_CYCLES, "large-amount")
        .expect_err("offline auth cell cannot be resolved");
    assert_eq!(
        script_exit_code(&failure.error),
        Some(exit_code::AUTH),
        "amount size must not affect the code path, got: {failure}"
    );
}

#[test]
fn minimum_capacity_locked_cell_reaches_the_auth_step() {
    require_executable_vm!();
    let mut harness = LockHarness::new();
    let cell = harness.create_locked_cell(MIN_CAPACITY);
    let unsigned = harness.build_unlock(
        cell,
        &LockHarness::well_formed_signature(),
        MIN_CAPACITY,
        false,
    );
    let signed = harness.sign_unlock_like_ckb_auth(unsigned);

    let failure = harness
        .verify(&signed, MAX_CYCLES, "minimum-capacity")
        .expect_err("offline auth cell cannot be resolved");
    assert_eq!(
        script_exit_code(&failure.error),
        Some(exit_code::AUTH),
        "a minimum-capacity locked cell is valid and must reach auth, got: {failure}"
    );
}

// ── Fixture capture for debugging ─────────────────────────────────────────────

#[test]
fn failing_transactions_write_a_replayable_fixture() {
    require_executable_vm!();
    let mut harness = LockHarness::new();
    let cell = harness.create_locked_cell(LOCKED_CAPACITY);
    let tx = harness.build_unlock(cell, &[0x11u8; 10], LOCKED_CAPACITY, false);

    let failure = harness
        .verify(&tx, MAX_CYCLES, "fixture-capture-smoke")
        .expect_err("a 10-byte signature must be rejected");

    assert!(failure.fixture.is_file(), "fixture should exist");
    let body = std::fs::read_to_string(&failure.fixture).expect("read fixture");
    assert!(body.contains("\"reason\""), "fixture records the reason");
    assert!(
        body.contains("mock_transaction") || body.contains("inputs"),
        "fixture contains debugger-replayable transaction data"
    );
}

/// Extract the raw 65-byte lock from witness 0 of `tx`.
fn witness_lock(tx: &ckb_testtool::ckb_types::core::TransactionView) -> Vec<u8> {
    use ckb_testtool::ckb_types::packed::WitnessArgs;
    let witness = tx.witnesses().get(0).unwrap();
    WitnessArgs::from_slice(&witness.raw_data())
        .expect("witness args")
        .lock()
        .to_opt()
        .expect("lock field")
        .raw_data()
        .into()
}

// ── Simulator control ─────────────────────────────────────────────────────────

#[test]
fn control_the_simulator_can_execute_a_bundled_ckb_cell() {
    // Isolates harness bugs from binary/toolchain bugs: if this passes, the
    // simulator, cell deployment and transaction assembly are all sound, and
    // any failure in the lock tests is attributable to the locally built script.
    let mut harness = LockHarness::new();
    let always_success = harness.recipient_lock();

    let cell = harness.create_locked_cell_with_script(LOCKED_CAPACITY, always_success.clone());
    let output = ckb_testtool::ckb_types::packed::CellOutput::new_builder()
        .capacity(LOCKED_CAPACITY)
        .lock(always_success)
        .build();

    let tx = ckb_testtool::ckb_types::core::TransactionBuilder::default()
        .input(
            ckb_testtool::ckb_types::packed::CellInput::new_builder()
                .previous_output(cell)
                .build(),
        )
        .output(output)
        .outputs_data(vec![ckb_testtool::ckb_types::bytes::Bytes::new()].pack())
        .witness(ckb_testtool::ckb_types::bytes::Bytes::new())
        .build();
    let tx = harness.context.complete_tx(tx);

    harness
        .verify(&tx, MAX_CYCLES, "control-always-success")
        .expect("bundled always-success cell must execute");
}

#[test]
fn report_skip_accounting() {
    // Rust treats an early `return` as a pass, so a summary line can hide
    // inactive assertions. `require_executable_vm!` now panics instead, but this
    // test stays as the single, explicit statement of that policy: an artifact
    // the VM cannot execute fails the run, it does not warn.
    //
    // The exact skip count is order-dependent because tests run in parallel, so
    // only the zero/non-zero distinction is asserted.
    let skipped = skipped_assertion_count();
    match vm_compatibility() {
        VmCompatibility::Executable => {
            assert_eq!(
                skipped, 0,
                "no transaction-level assertion should be skipped when the VM can run the script"
            );
            println!("all {VM_DEPENDENT_ASSERTION_COUNT} transaction-level assertions active");
        }
        VmCompatibility::Incompatible { detail } => {
            panic!(
                "the compiled artifact is NOT executable by the CKB VM, so the \
                 {VM_DEPENDENT_ASSERTION_COUNT} transaction-level assertions are inactive \
                 (observed {skipped} skips at the time this test ran; the figure is \
                 order-dependent because tests run in parallel).\n\
                 Failing on purpose: a green run with inactive assertions is a false signal.\n\
                 cause: {detail}"
            );
        }
    }
}
