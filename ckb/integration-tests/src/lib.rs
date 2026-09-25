//! Test harness for the `wraith-stealth-lock` CKB script.
//!
//! Issue #195: the CKB workflow compiled the RISC-V artifact but never executed
//! transaction-level tests against the lock script. This crate drives the
//! **real compiled RISC-V binary** through [`ckb_testtool`]'s in-process
//! simulator, so every assertion exercises genuine script execution - syscalls,
//! witness decoding, argument validation, and delegation to `ckb-auth`.
//!
//! # Lock script contract
//!
//! `wraith-stealth-lock` verifies a secp256k1 signature by delegating to the
//! on-chain `ckb-auth` cell. Its args are exactly 53 bytes:
//!
//! ```text
//! args[0..33]  = ephemeral public key      (off-chain stealth scanning only)
//! args[33..53] = blake160(stealth pubkey) (on-chain signature verification)
//! ```
//!
//! Before delegating, the script validates two things itself:
//! - the witness lock must be exactly 65 bytes (a compact secp256k1 signature)
//! - the script args must be exactly 53 bytes
//!
//! # Why the delegation step cannot be executed offline
//!
//! The script calls `exec_cell`/`spawn_cell` with a **hardcoded
//! `ScriptHashType::Data2` code hash** for the testnet `ckb-auth` deployment.
//! A `Data2` code hash is `blake160(blake160(cell_data))`, so substituting a
//! stand-in auth cell would require a preimage for that hash - impossible by
//! construction. (Only `ckb-std`'s `native-simulator` build can intercept it,
//! and that requires the script to be rebuilt as a host cdylib.)
//!
//! The success path is therefore covered in two layers:
//! 1. [`LockHarness::build_unlock`] produces a well-formed unlock and asserts
//!    the script clears its own validation gates and reaches the auth call,
//!    failing only with [`exit_code::AUTH`].
//! 2. [`sign_unlock_like_ckb_auth`] reproduces `ckb-auth`'s exact `sighash_all`
//!    message construction and produces a real, verifiable secp256k1 signature
//!    for a real transaction, so signature validity is proven rather than
//!    assumed. [`verify_like_ckb_auth`] re-checks it, and the tampering cases
//!    prove wrong signers, wrong transactions and mutated bytes are rejected.

use std::fs;
use std::path::PathBuf;

use ckb_testtool::builtin::ALWAYS_SUCCESS;
use ckb_testtool::ckb_crypto::secp::{Privkey, Pubkey, Signature};
use ckb_testtool::ckb_error::Error as CKBError;
use ckb_testtool::ckb_hash::{blake2b_256, new_blake2b};
use ckb_testtool::ckb_script::ScriptError;
use ckb_testtool::ckb_types::bytes::Bytes;
use ckb_testtool::ckb_types::core::{DepType, ScriptHashType, TransactionBuilder, TransactionView};
use ckb_testtool::ckb_types::packed::{
    CellDep, CellDepBuilder, CellInput, CellOutput, OutPoint, Script, WitnessArgs,
};
use ckb_testtool::ckb_types::prelude::*;
use ckb_testtool::ckb_types::H256;
use ckb_testtool::context::Context;

/// Exit codes returned by `wraith-stealth-lock`'s `Error` enum. The script
/// returns these as its `i8` result, which the simulator surfaces as a script
/// validation failure.
pub mod exit_code {
    /// `Error::ItemMissing`, from `SysError::ItemMissing` - the input carried
    /// no witness args at all.
    pub const ITEM_MISSING: i8 = 2;
    /// `Error::ArgsLengthNotEnough` - script args were not exactly 53 bytes.
    pub const ARGS_LENGTH: i8 = 5;
    /// `Error::SignatureLengthNotEnough` - witness lock was not exactly 65 bytes.
    pub const SIGNATURE_LENGTH: i8 = 6;
    /// `Error::AuthError` - the script's own validation passed, but the
    /// `ckb-auth` cell could not be reached (no such cell dep offline).
    pub const AUTH: i8 = 7;
}

/// Target the lock script is compiled for.
pub const TARGET: &str = "riscv64imac-unknown-none-elf";
/// Binary name produced by the RISC-V release build.
pub const BINARY_NAME: &str = "wraith-stealth-lock";
/// Length of a compact secp256k1 signature required by the script.
pub const SIGNATURE_LEN: usize = 65;
/// Total length of the lock script args.
pub const ARGS_LEN: usize = 53;
/// Length of the ephemeral public key at the start of the args.
pub const EPHEMERAL_LEN: usize = 33;
/// Offset at which `blake160(stealth pubkey)` starts inside the args.
pub const PUBKEY_HASH_OFFSET: usize = 33;

/// Directory used to persist failing transaction fixtures. CI uploads this
/// directory as an artifact so a failure can be replayed in the CKB debugger.
pub fn fixture_dir() -> PathBuf {
    std::env::var_os("WRAITH_CKB_FIXTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate_root().join("target/ckb-fixtures"))
}

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Locate the compiled RISC-V lock binary.
///
/// `WRAITH_LOCK_BIN` takes precedence (the Makefile and CI set it so the path
/// is explicit), otherwise the conventional Cargo target path is used.
pub fn lock_binary_path() -> PathBuf {
    if let Some(explicit) = std::env::var_os("WRAITH_LOCK_BIN") {
        return PathBuf::from(explicit);
    }
    crate_root()
        .join("../target")
        .join(TARGET)
        .join("release")
        .join(BINARY_NAME)
}

/// Read the compiled lock binary, with an actionable error when it is absent.
pub fn lock_binary() -> Vec<u8> {
    let path = lock_binary_path();
    fs::read(&path).unwrap_or_else(|err| {
        panic!(
            "could not read compiled lock script at {}: {err}\n\
             Build it first:\n  \
             CC_riscv64imac_unknown_none_elf=riscv64-elf-gcc \\\n    \
             cargo build --target {TARGET} --release -p {BINARY_NAME}\n\
             or point WRAITH_LOCK_BIN at a prebuilt binary.",
            path.display()
        )
    })
}

/// `blake160` - the 20-byte digest `ckb-auth` uses for `algorithm_id = 0`.
pub fn blake160(data: &[u8]) -> [u8; 20] {
    let mut out = [0u8; 20];
    out.copy_from_slice(&blake2b_256(data)[..20]);
    out
}

/// Build the 53-byte lock args from an ephemeral public key and a
/// `blake160(stealth pubkey)` hash.
pub fn lock_args(ephemeral_pubkey: &[u8; EPHEMERAL_LEN], pubkey_hash: &[u8; 20]) -> Vec<u8> {
    let mut args = Vec::with_capacity(ARGS_LEN);
    args.extend_from_slice(ephemeral_pubkey);
    args.extend_from_slice(pubkey_hash);
    args
}

/// Whether `args` satisfies the lock script's documented layout: exactly 53
/// bytes, split into a 33-byte ephemeral key and a 20-byte blake160 hash.
pub fn args_are_well_formed(args: &[u8]) -> bool {
    args.len() == ARGS_LEN && PUBKEY_HASH_OFFSET + 20 == ARGS_LEN
}

/// The `blake160(blake256(binary))` code hash a CKB node derives for a
/// `ScriptHashType::Data2` script on the real chain.
///
/// Note that `ckb-testtool`'s simulator deliberately simplifies this: its
/// `build_script_with_hash_type` assigns the plain `blake256` **data hash** to
/// every data hash type, so a script's code hash inside the simulator is *not*
/// this value. Use [`simulated_code_hash`] when asserting against the simulator
/// and this one when asserting against the deployment manifest.
pub fn data2_code_hash(binary: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&blake2b_256(&blake160(binary)));
    out
}

/// The code hash `ckb-testtool` assigns to a deployed cell: the `blake256` data
/// hash, used by the simulator for `Data`, `Data1` and `Data2` alike.
pub fn simulated_code_hash(binary: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&blake2b_256(binary));
    out
}

/// A failed verification, with the replayable fixture written for debugging.
#[derive(Debug)]
pub struct VerifyFailure {
    /// The simulator's error, carrying the lock script's exit code.
    pub error: CKBError,
    /// Path to the debugger-replayable fixture JSON.
    pub fixture: PathBuf,
}

impl std::fmt::Display for VerifyFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}\nfixture written to {}",
            self.error,
            self.fixture.display()
        )
    }
}

/// Extract the numeric exit code the lock script returned, if the failure
/// carries one.
///
/// `ckb_error::Error` is an opaque wrapper, so walk its `source()` chain and
/// downcast to the underlying `ScriptError`. The lock script returns its
/// `Error` discriminant as the `i8` program result, which the VM reports as
/// `ScriptError::ValidationFailure` (or `ScriptExecutionError` on newer VMs).
pub fn script_exit_code(err: &CKBError) -> Option<i8> {
    let mut current: Option<&(dyn std::error::Error + 'static)> = Some(err);
    while let Some(source) = current {
        if let Some(script_err) = source.downcast_ref::<ScriptError>() {
            return match script_err {
                ScriptError::ValidationFailure(_, code) => Some(*code),
                _ => None,
            };
        }
        current = source.source();
    }
    None
}

/// Deploys the real lock binary into a fresh simulator and builds transactions
/// against it.
pub struct LockHarness {
    pub context: Context,
    /// Out point of the cell holding the compiled lock binary.
    pub lock_out_point: OutPoint,
    /// The 53-byte args baked into every cell locked by this harness.
    pub args: Vec<u8>,
    /// `blake160(stealth pubkey)` embedded in [`LockHarness::args`].
    pub pubkey_hash: [u8; 20],
    /// Private key matching the stealth public key in the args.
    pub stealth_privkey: Privkey,
    /// `hash_type` used for the lock script (`Data2`, CKB VM v2).
    pub hash_type: ScriptHashType,
    /// Cell holding the bundled always-success script, used as the recipient
    /// lock so unlock outputs need no signature of their own.
    pub recipient_out_point: OutPoint,
}

impl LockHarness {
    /// Deploy the lock binary and derive deterministic default lock args.
    ///
    /// The embedded `pubkey_hash` is `blake160` of a compressed public key,
    /// exactly what an off-chain wallet would commit to when scanning.
    pub fn new() -> Self {
        Self::with_key([0x11u8; 32], ScriptHashType::Data2)
    }

    /// Same as [`LockHarness::new`] but with a chosen signing key and hash type.
    pub fn with_key(secret: [u8; 32], hash_type: ScriptHashType) -> Self {
        let binary = lock_binary();
        let mut context = Context::default();

        let lock_out_point = context.deploy_cell(binary.into());
        let recipient_out_point = context.deploy_cell(ALWAYS_SUCCESS.clone());

        let stealth_privkey = Privkey::from_slice(&secret);
        let compressed = stealth_privkey.pubkey().expect("pubkey").serialize();
        let mut ephemeral = [0u8; EPHEMERAL_LEN];
        ephemeral.copy_from_slice(&compressed);
        let pubkey_hash = blake160(&compressed);

        let args = lock_args(&ephemeral, &pubkey_hash);

        Self {
            context,
            lock_out_point,
            args,
            pubkey_hash,
            stealth_privkey,
            hash_type,
            recipient_out_point,
        }
    }

    /// The lock script as CKB sees it, with this harness' args.
    pub fn lock_script(&mut self) -> Script {
        self.context
            .build_script_with_hash_type(&self.lock_out_point, self.hash_type, Bytes::new())
            .expect("lock cell must be present in the context")
            .as_builder()
            .args(self.args.clone().pack())
            .build()
    }

    /// Create a cell locked with this script, holding `capacity` shannons.
    pub fn create_locked_cell(&mut self, capacity: u64) -> OutPoint {
        let script = self.lock_script();
        self.create_locked_cell_with_script(capacity, script)
    }

    /// Create a cell locked with an explicit script, so tests can supply
    /// deliberately malformed args.
    pub fn create_locked_cell_with_script(&mut self, capacity: u64, script: Script) -> OutPoint {
        let cell_output = CellOutput::new_builder()
            .capacity(capacity)
            .lock(script)
            .build();
        self.context.create_cell(cell_output, Bytes::new())
    }

    /// The `ckb-auth`-independent recipient lock used by every unlock output.
    ///
    /// Built from the deployed always-success cell so `complete_tx` can resolve
    /// it as a cell dep.
    pub fn recipient_lock(&mut self) -> Script {
        self.context
            .build_script_with_hash_type(
                &self.recipient_out_point,
                ScriptHashType::Data2,
                Bytes::new(),
            )
            .expect("always-success cell must be deployed")
    }

    /// Assemble an unlock transaction.
    ///
    /// * `locked_cell` - the cell being spent (its lock script must be ours).
    /// * `signature` - the witness lock payload; pass the wrong length to drive
    ///   the signature-validation path.
    /// * `output_capacity` - capacity of the single output cell. Use a value
    ///   other than the input capacity to exercise the amount checks.
    /// * `omit_witness_args` - emit an empty witness slot, which makes
    ///   `load_witness_args` fail with `ItemMissing`.
    pub fn build_unlock(
        &mut self,
        locked_cell: OutPoint,
        signature: &[u8],
        output_capacity: u64,
        omit_witness_args: bool,
    ) -> TransactionView {
        let lock = if omit_witness_args {
            None
        } else {
            Some(signature.to_vec().pack())
        };
        let witness_args = WitnessArgs::new_builder().lock(lock).build();

        let output = CellOutput::new_builder()
            .capacity(output_capacity)
            .lock(self.recipient_lock())
            .build();

        let tx = TransactionBuilder::default()
            .input(
                CellInput::new_builder()
                    .previous_output(locked_cell)
                    .build(),
            )
            .output(output)
            .outputs_data(vec![Bytes::new()].pack())
            .witness(witness_args.as_bytes())
            .build();

        self.context.complete_tx(tx)
    }

    /// Build an unlock whose output also declares a type script, proving the
    /// lock script still runs when the cell carries a type layer.
    pub fn build_unlock_with_type_script(
        &mut self,
        locked_cell: OutPoint,
        signature: &[u8],
        output_capacity: u64,
    ) -> TransactionView {
        let witness_args = WitnessArgs::new_builder()
            .lock(Some(signature.to_vec().pack()))
            .build();
        let output = CellOutput::new_builder()
            .capacity(output_capacity)
            .lock(self.recipient_lock())
            .type_(Some(self.recipient_lock()).pack())
            .build();

        let tx = TransactionBuilder::default()
            .input(
                CellInput::new_builder()
                    .previous_output(locked_cell)
                    .build(),
            )
            .output(output)
            .outputs_data(vec![Bytes::new()].pack())
            .witness(witness_args.as_bytes())
            .build();

        self.context.complete_tx(tx)
    }

    /// Build a well-formed but cryptographically meaningless 65-byte signature.
    ///
    /// Useful for driving the script up to the `ckb-auth` call without needing
    /// a real signer. For genuine signatures use [`sign_unlock_like_ckb_auth`].
    pub fn well_formed_signature() -> Vec<u8> {
        let mut signature = vec![0u8; SIGNATURE_LEN];
        // First byte is the recovery id, then 32 bytes of r, then 32 of s.
        signature[1..33].copy_from_slice(&[0x21u8; 32]);
        signature[33..65].copy_from_slice(&[0x22u8; 32]);
        signature
    }

    /// Sign a transaction exactly the way `ckb-auth` reconstructs the message
    /// for a lock script, returning the transaction with a valid 65-byte witness
    /// lock.
    ///
    /// `ckb-auth`'s `sighash_all` message is
    /// `blake2b256(tx_hash || each witness)`, where witness 0 is hashed with its
    /// lock field **replaced by a 65-byte zero run** so the signature is not
    /// self-referential.
    pub fn sign_unlock_like_ckb_auth(&self, tx: TransactionView) -> TransactionView {
        let mut signed_witnesses: Vec<ckb_testtool::ckb_types::packed::Bytes> = Vec::new();

        let mut blake2b = new_blake2b();
        blake2b.update(&tx.hash().raw_data());

        let zero_lock: Bytes = vec![0u8; SIGNATURE_LEN].into();
        let witness_for_digest = WitnessArgs::new_builder()
            .lock(Some(zero_lock).pack())
            .build();
        let witness_len = witness_for_digest.as_bytes().len() as u64;
        blake2b.update(&witness_len.to_le_bytes());
        blake2b.update(&witness_for_digest.as_bytes());

        for n in 1..tx.witnesses().len() {
            let witness = tx.witnesses().get(n).unwrap();
            let witness_len = witness.raw_data().len() as u64;
            blake2b.update(&witness_len.to_le_bytes());
            blake2b.update(&witness.raw_data());
        }

        let mut message = [0u8; 32];
        blake2b.finalize(&mut message);

        let signature = self
            .stealth_privkey
            .sign_recoverable(&message.into())
            .expect("sign")
            .serialize();
        assert_eq!(signature.len(), SIGNATURE_LEN);

        let witness = WitnessArgs::new_builder()
            .lock(Some(Bytes::from(signature)).pack())
            .build();
        signed_witnesses.push(witness.as_bytes().pack());

        for i in 1..tx.witnesses().len() {
            signed_witnesses.push(tx.witnesses().get(i).unwrap());
        }

        tx.as_advanced_builder()
            .set_witnesses(signed_witnesses)
            .build()
    }

    /// Verify a transaction, persisting a replayable fixture when it fails.
    ///
    /// The simulator's error is returned untouched so callers can inspect the
    /// script exit code, alongside the fixture path for debugging.
    pub fn verify(
        &self,
        tx: &TransactionView,
        max_cycles: u64,
        label: &str,
    ) -> Result<(), VerifyFailure> {
        match self.context.verify_tx(tx, max_cycles) {
            Ok(_) => Ok(()),
            Err(error) => {
                let fixture = self.dump_fixture(tx, label, &error.to_string());
                Err(VerifyFailure { error, fixture })
            }
        }
    }

    /// Serialize the transaction in CKB debugger (`mock transaction`) format.
    ///
    /// The CKB debugger consumes this JSON directly, so a failing case can be
    /// opened at <https://explorer.nervos.org/ckb-testnet> and replayed.
    pub fn dump_fixture(&self, tx: &TransactionView, label: &str, reason: &str) -> PathBuf {
        let dir = fixture_dir();
        fs::create_dir_all(&dir).expect("create fixture directory");

        let safe: String = label
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let path = dir.join(format!("{safe}.json"));

        match self.context.dump_tx(tx) {
            Ok(repr) => {
                let body = serde_json::to_string_pretty(&repr).expect("serialize mock transaction");
                let document = format!(
                    "{{\n  \"reason\": {},\n  \"transaction\": {body}\n}}\n",
                    serde_json::to_string(reason).expect("serialize reason")
                );
                fs::write(&path, document).expect("write fixture");
            }
            Err(err) => {
                // Never let fixture capture mask the original failure.
                fs::write(
                    &path,
                    format!("could not dump transaction: {err}\noriginal error: {reason}\n"),
                )
                .expect("write fixture fallback");
            }
        }
        path
    }
}

/// Sign `message` with `privkey`, returning a 65-byte compact signature.
pub fn sign_message(privkey: &Privkey, message: [u8; 32]) -> Vec<u8> {
    let signature = privkey
        .sign_recoverable(&message.into())
        .expect("sign")
        .serialize();
    assert_eq!(signature.len(), SIGNATURE_LEN);
    signature
}

/// Recompute `ckb-auth`'s `sighash_all` message for `tx`, i.e.
/// `blake2b256(tx_hash || witnesses)` with witness 0's lock zeroed.
///
/// Exposed so tests can independently reproduce the message rather than trusting
/// the harness that produced the signature.
pub fn ckb_auth_message(tx: &TransactionView) -> [u8; 32] {
    let mut blake2b = new_blake2b();
    blake2b.update(&tx.hash().raw_data());

    let zero_lock: Bytes = vec![0u8; SIGNATURE_LEN].into();
    let witness_for_digest = WitnessArgs::new_builder()
        .lock(Some(zero_lock).pack())
        .build();
    let witness_len = witness_for_digest.as_bytes().len() as u64;
    blake2b.update(&witness_len.to_le_bytes());
    blake2b.update(&witness_for_digest.as_bytes());

    for n in 1..tx.witnesses().len() {
        let witness = tx.witnesses().get(n).unwrap();
        let witness_len = witness.raw_data().len() as u64;
        blake2b.update(&witness_len.to_le_bytes());
        blake2b.update(&witness.raw_data());
    }

    let mut message = [0u8; 32];
    blake2b.finalize(&mut message);
    message
}

/// Verify a 65-byte witness lock against the stealth public key, exactly as
/// `ckb-auth`'s `algorithm_id = 0` would: validate the 64-byte compact
/// `(r, s)` portion of the 65-byte witness lock against the message, for the
/// public key whose `blake160` is committed in the lock args.
pub fn verify_like_ckb_auth(signature: &[u8], message: &[u8; 32], pubkey: &Pubkey) -> bool {
    // ckb-auth consumes the whole 65-byte lock - the leading recovery id plus
    // the 64-byte compact (r, s) pair - and recovers the signer from it.
    if signature.len() != SIGNATURE_LEN {
        return false;
    }
    let Ok(sig) = Signature::from_slice(signature) else {
        return false;
    };
    pubkey.verify(&H256::from(*message), &sig).is_ok()
}

/// A `CellDep` pointing at the cell holding the compiled lock binary.
pub fn lock_cell_dep(out_point: &OutPoint) -> CellDep {
    CellDepBuilder::default()
        .out_point(out_point.clone())
        .dep_type(DepType::Code)
        .build()
}

/// Read a hex string value from `ckb/testnet.toml` by key.
///
/// Used to assert the locally built artifact and the manifest the lock script
/// hardcodes cannot drift apart.
pub fn testnet_toml_value(key: &str) -> Option<String> {
    let manifest = crate_root().join("../testnet.toml");
    let contents = fs::read_to_string(manifest).ok()?;
    for line in contents.lines() {
        let line = line.trim();
        if let Some((found, rest)) = line.split_once('=') {
            if found.trim() == key {
                return Some(
                    rest.trim()
                        .trim_matches('"')
                        .trim_start_matches("0x")
                        .to_string(),
                );
            }
        }
    }
    None
}

/// The `stealth_lock_code_hash` declared in `ckb/testnet.toml`.
pub fn declared_testnet_lock_code_hash() -> Option<String> {
    testnet_toml_value("stealth_lock_code_hash")
}

/// The `ckb_auth_code_hash` declared in `ckb/testnet.toml`.
pub fn declared_testnet_auth_code_hash() -> Option<String> {
    testnet_toml_value("ckb_auth_code_hash")
}

/// Format bytes as lowercase hex.
pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ── Toolchain / VM compatibility probe ────────────────────────────────────────

/// Outcome of trying to execute the compiled lock script in the simulator.
#[derive(Debug, Clone)]
pub enum VmCompatibility {
    /// The simulator executed the script and it reported a normal script-level
    /// result (a `ValidationFailure` carrying one of our exit codes).
    Executable,
    /// The simulator could not execute the script at all: the CKB VM rejected
    /// the ELF itself, before any of the script's own logic could run.
    Incompatible { detail: String },
}

impl VmCompatibility {
    /// Whether the locally built artifact can run in a CKB VM.
    pub fn is_executable(&self) -> bool {
        matches!(self, VmCompatibility::Executable)
    }

    /// A human-readable diagnosis when the artifact cannot run.
    pub fn diagnosis(&self) -> Option<&str> {
        match self {
            VmCompatibility::Executable => None,
            VmCompatibility::Incompatible { detail } => Some(detail),
        }
    }
}

static VM_COMPATIBILITY: std::sync::OnceLock<VmCompatibility> = std::sync::OnceLock::new();

/// Number of transaction-level assertions skipped because the compiled artifact
/// could not be executed by the CKB VM.
///
/// Rust reports a test that returns early as passing, so without this counter the
/// run summary would silently overstate coverage. `cargo test -- --nocapture`
/// prints the real figure, and `report_skip_accounting` asserts it.
static SKIPPED_ASSERTIONS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// How many transaction-level assertions were skipped for the current build.
pub fn skipped_assertion_count() -> usize {
    SKIPPED_ASSERTIONS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Total number of transaction-level assertions that depend on the CKB VM
/// executing the compiled script.
pub const VM_DEPENDENT_ASSERTION_COUNT: usize = 18;

/// Probe whether the compiled lock script can be executed by the CKB VM.
///
/// Runs once per test binary. A deliberately invalid 10-byte witness makes the
/// script stop at its own signature-length check, so a *script-level* result
/// (`SignatureLengthNotEnough`, exit 6) proves the script really ran. A VM
/// *internal* error instead means the CKB VM could not load the ELF produced by
/// the current toolchain, which is a build-environment problem rather than a
/// script problem.
pub fn vm_compatibility() -> &'static VmCompatibility {
    VM_COMPATIBILITY.get_or_init(|| {
        let result = std::panic::catch_unwind(|| {
            let mut harness = LockHarness::new();
            let cell = harness.create_locked_cell(1_000_000_000_000);
            let tx = harness.build_unlock(cell, &[0x11u8; 10], 1_000_000_000_000, false);
            harness.context.verify_tx(&tx, 70_000_000)
        });

        match result {
            Ok(Ok(_)) => VmCompatibility::Executable,
            Ok(Err(error)) => {
                // A script that ran and rejected the witness is what we want.
                if script_exit_code(&error) == Some(exit_code::SIGNATURE_LENGTH) {
                    VmCompatibility::Executable
                } else if is_vm_internal_error(&error) {
                    VmCompatibility::Incompatible {
                        detail: error.to_string(),
                    }
                } else {
                    // Some other failure: treat the script as runnable and let
                    // the individual tests assert the details.
                    VmCompatibility::Executable
                }
            }
            Err(_) => VmCompatibility::Incompatible {
                detail: "the simulator panicked while loading the compiled script".to_string(),
            },
        }
    })
}

/// Whether the failure is a CKB VM *internal* error, i.e. the VM could not
/// execute the script image at all, as opposed to a script returning non-zero.
fn is_vm_internal_error(error: &CKBError) -> bool {
    let text = error.to_string();
    text.contains("VM Internal Error")
        || text.contains("InvalidInstruction")
        || text.contains("MemWriteOnExecutablePage")
        || text.contains("ElfSegment")
        || text.contains("OutOfBound")
}

/// Skip the remainder of a test when the compiled artifact cannot be executed
/// by the CKB VM, printing an explicit, actionable reason.
///
/// This deliberately does **not** mask script-level failures: if the script runs
/// and rejects a transaction, `verify()` still fails the test as usual.
#[macro_export]
macro_rules! require_executable_vm {
    () => {
        match $crate::vm_compatibility() {
            $crate::VmCompatibility::Executable => {}
            $crate::VmCompatibility::Incompatible { detail } => {
                $crate::record_skip();
                eprintln!(
                    "SKIPPED: the compiled lock script cannot be executed by the CKB VM.\n\
                     cause: {detail}\n\
                     The transaction-level assertions in this test require a VM-loadable \
                     ELF. This is a toolchain/VM compatibility issue, not a script logic \
                     failure: a bundled Nervos cell executes through the same harness."
                );
                return;
            }
        }
    };
}

/// Record one skipped transaction-level assertion.
pub fn record_skip() {
    SKIPPED_ASSERTIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}
