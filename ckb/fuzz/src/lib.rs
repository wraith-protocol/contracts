//! Deterministic pure-Rust fuzz models and byte-level parsers for the CKB
//! scripts (`wraith-stealth-lock` and `wraith-names-type`), `ckb-auth` argv
//! validation, cell-dep resolution, output-index bounds, and stealth
//! announcement args (Issue #281).
//!
//! On-chain CKB scripts compile to `no_std` RISC-V binaries executed inside
//! `ckb-vm` (covered end-to-end by `ckb/integration-tests`). This host crate
//! mirrors the exact Molecule wire format, script validation state machines,
//! `ckb-auth` IPC contract, and transaction layout invariants so `cargo fuzz`
//! and `cargo test` can drive millions of malformed inputs with zero panics
//! and zero out-of-bounds reads.

use arbitrary::{Arbitrary, Unstructured};

/// Expected `wraith-stealth-lock` script args length (`ephemeral[33] || hash[20]`).
pub const STEALTH_LOCK_ARGS_LEN: usize = 53;
/// Compressed secp256k1 public key length in bytes.
pub const COMPRESSED_PUBKEY_LEN: usize = 33;
/// `blake160(stealth_pubkey)` length in bytes.
pub const PUBKEY_HASH_LEN: usize = 20;
/// Recoverable secp256k1 signature length in `WitnessArgs.lock`.
pub const SIGNATURE_LEN: usize = 65;
/// Expected `wraith-names-type` script args length (`blake2b(name)[0..32]`).
pub const NAMES_TYPE_ARGS_LEN: usize = 32;
/// Expected `.wraith` name cell data length (`spending_pk[33] || viewing_pk[33]`).
pub const NAMES_CELL_DATA_LEN: usize = 66;
/// Expected `ckb-auth` secp256k1/blake160 algorithm identifier (`0x00`).
pub const CKB_AUTH_SECP256K1_ALGO_ID: u8 = 0x00;
/// Canonical Wraith secp256k1 stealth announcement scheme identifier.
pub const STEALTH_ANNOUNCEMENT_SCHEME_SECP256K1: u8 = 0x01;
/// Upper bound on announcement metadata bytes accepted by the canonical codec.
pub const MAX_ANNOUNCEMENT_METADATA_LEN: usize = 256;
/// Minimum occupied CKB cell capacity (61 CKB in shannons).
pub const MIN_CELL_CAPACITY_SHANNONS: u64 = 6_100_000_000;

/// Deployed `ckb-auth` `Data2` code hash hardcoded in `wraith-stealth-lock`.
pub const CKB_AUTH_CODE_HASH: [u8; 32] = [
    0x09, 0x15, 0x98, 0x3b, 0xb3, 0x15, 0x84, 0xdf, 0x45, 0x66, 0xe0, 0x94, 0x6f, 0xd0, 0x0e, 0xf1,
    0xe9, 0xa7, 0x5a, 0xd3, 0x7a, 0x39, 0xce, 0x70, 0xfe, 0xc9, 0xb5, 0xcb, 0xf3, 0xb8, 0x70, 0x21,
];

// ── 1. Molecule WitnessArgs Wire Codec & Lock Script Model ───────────────────

/// Exit codes matching `ckb/contracts/wraith-stealth-lock/src/main.rs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i8)]
pub enum LockScriptError {
    IndexOutOfBound = 1,
    ItemMissing = 2,
    LengthNotEnough = 3,
    Encoding = 4,
    ArgsLengthNotEnough = 5,
    SignatureLengthNotEnough = 6,
    AuthError = 7,
    UnexpectedSysError = 8,
}

/// Decoded Molecule `WitnessArgs` table (`table WitnessArgs { lock: BytesOpt,
/// input_type: BytesOpt, output_type: BytesOpt }`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DecodedWitnessArgs {
    pub lock: Option<Vec<u8>>,
    pub input_type: Option<Vec<u8>>,
    pub output_type: Option<Vec<u8>>,
}

fn read_u32_le(slice: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    let bytes = slice.get(offset..end)?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn decode_bytes_opt(field: &[u8]) -> Result<Option<Vec<u8>>, LockScriptError> {
    if field.is_empty() {
        return Ok(None);
    }
    let inner_len = read_u32_le(field, 0).ok_or(LockScriptError::Encoding)? as usize;
    let expected_total = 4usize
        .checked_add(inner_len)
        .ok_or(LockScriptError::Encoding)?;
    if expected_total != field.len() {
        return Err(LockScriptError::Encoding);
    }
    Ok(Some(field[4..expected_total].to_vec()))
}

fn encode_bytes_opt(opt: &Option<Vec<u8>>, out: &mut Vec<u8>) -> Result<(), LockScriptError> {
    if let Some(bytes) = opt {
        let len_u32 = u32::try_from(bytes.len()).map_err(|_| LockScriptError::Encoding)?;
        out.extend_from_slice(&len_u32.to_le_bytes());
        out.extend_from_slice(bytes);
    }
    Ok(())
}

/// Strictly decode a Molecule `WitnessArgs` buffer, rejecting truncated headers,
/// trailing excess bytes, non-monotonic offsets, offsets into the 16-byte header,
/// and `u32::MAX` length overflows without panicking.
pub fn decode_witness_args(raw: &[u8]) -> Result<DecodedWitnessArgs, LockScriptError> {
    if raw.len() < 16 {
        return Err(LockScriptError::Encoding);
    }
    let total_size = read_u32_le(raw, 0).ok_or(LockScriptError::Encoding)? as usize;
    if total_size != raw.len() {
        return Err(LockScriptError::Encoding);
    }

    let offset_lock = read_u32_le(raw, 4).ok_or(LockScriptError::Encoding)? as usize;
    let offset_input = read_u32_le(raw, 8).ok_or(LockScriptError::Encoding)? as usize;
    let offset_output = read_u32_le(raw, 12).ok_or(LockScriptError::Encoding)? as usize;

    if offset_lock != 16
        || offset_input < offset_lock
        || offset_output < offset_input
        || offset_output > total_size
    {
        return Err(LockScriptError::Encoding);
    }

    let lock_slice = raw
        .get(offset_lock..offset_input)
        .ok_or(LockScriptError::Encoding)?;
    let input_slice = raw
        .get(offset_input..offset_output)
        .ok_or(LockScriptError::Encoding)?;
    let output_slice = raw
        .get(offset_output..total_size)
        .ok_or(LockScriptError::Encoding)?;

    Ok(DecodedWitnessArgs {
        lock: decode_bytes_opt(lock_slice)?,
        input_type: decode_bytes_opt(input_slice)?,
        output_type: decode_bytes_opt(output_slice)?,
    })
}

/// Encode `DecodedWitnessArgs` into canonical Molecule table bytes.
pub fn encode_witness_args(args: &DecodedWitnessArgs) -> Result<Vec<u8>, LockScriptError> {
    let mut lock_buf = Vec::new();
    let mut input_buf = Vec::new();
    let mut output_buf = Vec::new();
    encode_bytes_opt(&args.lock, &mut lock_buf)?;
    encode_bytes_opt(&args.input_type, &mut input_buf)?;
    encode_bytes_opt(&args.output_type, &mut output_buf)?;

    let offset_lock = 16usize;
    let offset_input = offset_lock
        .checked_add(lock_buf.len())
        .ok_or(LockScriptError::Encoding)?;
    let offset_output = offset_input
        .checked_add(input_buf.len())
        .ok_or(LockScriptError::Encoding)?;
    let total_size = offset_output
        .checked_add(output_buf.len())
        .ok_or(LockScriptError::Encoding)?;

    let total_u32 = u32::try_from(total_size).map_err(|_| LockScriptError::Encoding)?;
    let off0_u32 = u32::try_from(offset_lock).map_err(|_| LockScriptError::Encoding)?;
    let off1_u32 = u32::try_from(offset_input).map_err(|_| LockScriptError::Encoding)?;
    let off2_u32 = u32::try_from(offset_output).map_err(|_| LockScriptError::Encoding)?;

    let mut out = Vec::with_capacity(total_size);
    out.extend_from_slice(&total_u32.to_le_bytes());
    out.extend_from_slice(&off0_u32.to_le_bytes());
    out.extend_from_slice(&off1_u32.to_le_bytes());
    out.extend_from_slice(&off2_u32.to_le_bytes());
    out.extend_from_slice(&lock_buf);
    out.extend_from_slice(&input_buf);
    out.extend_from_slice(&output_buf);
    Ok(out)
}

/// Parsed 53-byte `wraith-stealth-lock` script arguments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParsedStealthLockArgs {
    pub ephemeral_pubkey: [u8; COMPRESSED_PUBKEY_LEN],
    pub pubkey_hash: [u8; PUBKEY_HASH_LEN],
}

/// Parse `wraith-stealth-lock` script args, rejecting truncated or excess bytes.
pub fn parse_stealth_lock_args(raw: &[u8]) -> Result<ParsedStealthLockArgs, LockScriptError> {
    if raw.len() != STEALTH_LOCK_ARGS_LEN {
        return Err(LockScriptError::ArgsLengthNotEnough);
    }
    let mut ephemeral_pubkey = [0u8; COMPRESSED_PUBKEY_LEN];
    let mut pubkey_hash = [0u8; PUBKEY_HASH_LEN];
    ephemeral_pubkey.copy_from_slice(&raw[0..33]);
    pubkey_hash.copy_from_slice(&raw[33..53]);
    Ok(ParsedStealthLockArgs {
        ephemeral_pubkey,
        pubkey_hash,
    })
}

/// Evaluate `wraith-stealth-lock`'s full pre-auth and cell-dep validation path.
pub fn validate_stealth_lock_invocation(
    script_args: &[u8],
    group_witnesses: &[Vec<u8>],
    tx_hash: &[u8; 32],
    cell_deps: &[MockCellDep],
) -> Result<CkbAuthInvocation, LockScriptError> {
    let first_witness = group_witnesses
        .first()
        .ok_or(LockScriptError::IndexOutOfBound)?;
    let witness_args = decode_witness_args(first_witness)?;
    let sig = witness_args.lock.unwrap_or_default();
    if sig.len() != SIGNATURE_LEN {
        return Err(LockScriptError::SignatureLengthNotEnough);
    }

    let parsed_args = parse_stealth_lock_args(script_args)?;
    let mut signature = [0u8; SIGNATURE_LEN];
    signature.copy_from_slice(&sig);

    let inv = CkbAuthInvocation {
        algorithm_id: CKB_AUTH_SECP256K1_ALGO_ID,
        signature,
        message: *tx_hash,
        pubkey_hash: parsed_args.pubkey_hash,
    };

    if resolve_ckb_auth_dep(cell_deps).is_err() {
        return Err(LockScriptError::AuthError);
    }
    validate_ckb_auth_invocation(&inv).map_err(|_| LockScriptError::AuthError)?;
    Ok(inv)
}

// ── 2. `ckb-auth` Input & Algorithm ID Boundary Model ────────────────────────

/// Structured `ckb-auth` IPC payload constructed by `wraith-stealth-lock`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CkbAuthInvocation {
    pub algorithm_id: u8,
    pub signature: [u8; SIGNATURE_LEN],
    pub message: [u8; 32],
    pub pubkey_hash: [u8; PUBKEY_HASH_LEN],
}

/// Raw untrusted `exec_cell` C-string argv slices supplied to `ckb-auth`.
#[derive(Clone, Debug, Arbitrary)]
pub struct RawCkbAuthArgv {
    pub algorithm_id_arg: Vec<u8>,
    pub signature_arg: Vec<u8>,
    pub message_arg: Vec<u8>,
    pub pubkey_hash_arg: Vec<u8>,
}

/// Errors produced when validating `ckb-auth` inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CkbAuthError {
    InteriorNulByte,
    InvalidHexEncoding,
    UnsupportedAlgorithmId(u8),
    InvalidAlgorithmLength,
    InvalidSignatureLength,
    InvalidRecoveryId(u8),
    ZeroRorSComponent,
    InvalidMessageLength,
    InvalidPubkeyHashLength,
}

fn decode_strict_lower_hex(arg: &[u8]) -> Result<Vec<u8>, CkbAuthError> {
    if arg.contains(&0u8) {
        return Err(CkbAuthError::InteriorNulByte);
    }
    if arg.is_empty() || arg.len() % 2 != 0 {
        return Err(CkbAuthError::InvalidHexEncoding);
    }
    let mut out = Vec::with_capacity(arg.len() / 2);
    for pair in arg.chunks_exact(2) {
        let hi = match pair[0] {
            b'0'..=b'9' => pair[0] - b'0',
            b'a'..=b'f' => pair[0] - b'a' + 10,
            _ => return Err(CkbAuthError::InvalidHexEncoding),
        };
        let lo = match pair[1] {
            b'0'..=b'9' => pair[1] - b'0',
            b'a'..=b'f' => pair[1] - b'a' + 10,
            _ => return Err(CkbAuthError::InvalidHexEncoding),
        };
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

/// Validate decoded `ckb-auth` parameters, rejecting unsupported algorithm IDs,
/// invalid recovery IDs (`>= 4`), and all-zero `r`/`s` scalars.
pub fn validate_ckb_auth_invocation(inv: &CkbAuthInvocation) -> Result<(), CkbAuthError> {
    if inv.algorithm_id != CKB_AUTH_SECP256K1_ALGO_ID {
        return Err(CkbAuthError::UnsupportedAlgorithmId(inv.algorithm_id));
    }
    let recid = inv.signature[0];
    if recid >= 4 {
        return Err(CkbAuthError::InvalidRecoveryId(recid));
    }
    if inv.signature[1..33].iter().all(|b| *b == 0)
        || inv.signature[33..65].iter().all(|b| *b == 0)
    {
        return Err(CkbAuthError::ZeroRorSComponent);
    }
    Ok(())
}

/// Parse and validate untrusted `ckb-auth` argv strings (`algorithm_id`,
/// `signature`, `message`, `pubkey_hash`).
pub fn parse_and_validate_ckb_auth_argv(
    argv: &RawCkbAuthArgv,
) -> Result<CkbAuthInvocation, CkbAuthError> {
    let algo_bytes = decode_strict_lower_hex(&argv.algorithm_id_arg)?;
    if algo_bytes.len() != 1 {
        return Err(CkbAuthError::InvalidAlgorithmLength);
    }
    let algorithm_id = algo_bytes[0];
    if algorithm_id != CKB_AUTH_SECP256K1_ALGO_ID {
        return Err(CkbAuthError::UnsupportedAlgorithmId(algorithm_id));
    }

    let sig_bytes = decode_strict_lower_hex(&argv.signature_arg)?;
    if sig_bytes.len() != SIGNATURE_LEN {
        return Err(CkbAuthError::InvalidSignatureLength);
    }

    let msg_bytes = decode_strict_lower_hex(&argv.message_arg)?;
    if msg_bytes.len() != 32 {
        return Err(CkbAuthError::InvalidMessageLength);
    }

    let hash_bytes = decode_strict_lower_hex(&argv.pubkey_hash_arg)?;
    if hash_bytes.len() != PUBKEY_HASH_LEN {
        return Err(CkbAuthError::InvalidPubkeyHashLength);
    }

    let mut signature = [0u8; SIGNATURE_LEN];
    signature.copy_from_slice(&sig_bytes);
    let mut message = [0u8; 32];
    message.copy_from_slice(&msg_bytes);
    let mut pubkey_hash = [0u8; PUBKEY_HASH_LEN];
    pubkey_hash.copy_from_slice(&hash_bytes);

    let inv = CkbAuthInvocation {
        algorithm_id,
        signature,
        message,
        pubkey_hash,
    };
    validate_ckb_auth_invocation(&inv)?;
    Ok(inv)
}

// ── 3. `wraith-names-type` Type Script & Singleton Uniqueness Model ──────────

/// Exit codes matching `ckb/contracts/wraith-names-type/src/main.rs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i8)]
pub enum NamesTypeError {
    IndexOutOfBound = 1,
    ItemMissing = 2,
    LengthNotEnough = 3,
    Encoding = 4,
    InvalidDataLength = 5,
    InvalidArgsLength = 6,
    DuplicateNameCell = 7,
    NoMatchingCells = 8,
    UnexpectedSysError = 9,
}

/// Validated `.wraith` name cell transition kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameTransition {
    Create { output_index: usize },
    Update { input_index: usize, output_index: usize },
    Destroy { input_index: usize },
}

/// Minimal cell representation for type-script and layout fuzzing.
#[derive(Clone, Debug, Arbitrary, PartialEq, Eq)]
pub struct MockCell {
    pub capacity: u64,
    pub lock_hash: [u8; 32],
    pub type_hash: Option<[u8; 32]>,
    pub data: Vec<u8>,
}

/// Mirror `wraith-names-type`'s `validate()` entrypoint over arbitrary inputs
/// and outputs, enforcing 32-byte args, singleton uniqueness (`<= 1` input and
/// `<= 1` output), non-empty invocation, and 66-byte output data.
pub fn validate_names_type_script(
    script_args: &[u8],
    active_script_hash: &[u8; 32],
    inputs: &[MockCell],
    outputs: &[MockCell],
) -> Result<NameTransition, NamesTypeError> {
    if script_args.len() != NAMES_TYPE_ARGS_LEN {
        return Err(NamesTypeError::InvalidArgsLength);
    }

    let mut matching_inputs = Vec::new();
    for (idx, cell) in inputs.iter().enumerate() {
        if cell.type_hash.as_ref() == Some(active_script_hash) {
            matching_inputs.push(idx);
        }
    }

    let mut matching_outputs = Vec::new();
    for (idx, cell) in outputs.iter().enumerate() {
        if cell.type_hash.as_ref() == Some(active_script_hash) {
            matching_outputs.push(idx);
        }
    }

    match (matching_inputs.len(), matching_outputs.len()) {
        (0, 0) => Err(NamesTypeError::NoMatchingCells),
        (in_cnt, _) if in_cnt > 1 => Err(NamesTypeError::DuplicateNameCell),
        (_, out_cnt) if out_cnt > 1 => Err(NamesTypeError::DuplicateNameCell),
        (0, 1) => {
            let out_idx = matching_outputs[0];
            if outputs[out_idx].data.len() != NAMES_CELL_DATA_LEN {
                return Err(NamesTypeError::InvalidDataLength);
            }
            Ok(NameTransition::Create {
                output_index: out_idx,
            })
        }
        (1, 1) => {
            let in_idx = matching_inputs[0];
            let out_idx = matching_outputs[0];
            if outputs[out_idx].data.len() != NAMES_CELL_DATA_LEN {
                return Err(NamesTypeError::InvalidDataLength);
            }
            Ok(NameTransition::Update {
                input_index: in_idx,
                output_index: out_idx,
            })
        }
        (1, 0) => Ok(NameTransition::Destroy {
            input_index: matching_inputs[0],
        }),
        _ => Err(NamesTypeError::NoMatchingCells),
    }
}

// ── 4. Cell Deps & Stealth Announcement Args Codec ───────────────────────────

/// CKB script hash type selector.
#[derive(Clone, Copy, Debug, Arbitrary, PartialEq, Eq)]
pub enum MockHashType {
    Data,
    Type,
    Data1,
    Data2,
}

/// CKB cell dependency kind.
#[derive(Clone, Copy, Debug, Arbitrary, PartialEq, Eq)]
pub enum MockDepType {
    Code,
    DepGroup,
}

/// Cell dependency entry used to fuzz `exec_cell` code/type hash resolution.
#[derive(Clone, Debug, Arbitrary, PartialEq, Eq)]
pub struct MockCellDep {
    pub dep_type: MockDepType,
    pub data_hash: [u8; 32],
    pub type_hash: Option<[u8; 32]>,
    pub duplicate_out_point_tag: u8,
}

/// Errors when resolving cell dependencies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellDepError {
    EmptyCellDeps,
    UnresolvedDepGroup,
    WrongHashOrMissingAuthDep,
    DuplicateOutPoint,
}

/// Resolve the `ckb-auth` `Data2` cell dep required by `wraith-stealth-lock`,
/// rejecting missing deps, wrong hashes, unresolved `DepGroup` entries, and
/// duplicate out-points.
pub fn resolve_ckb_auth_dep(cell_deps: &[MockCellDep]) -> Result<usize, CellDepError> {
    if cell_deps.is_empty() {
        return Err(CellDepError::EmptyCellDeps);
    }

    let mut seen_tags = [false; 256];
    let mut found_index = None;
    for (idx, dep) in cell_deps.iter().enumerate() {
        let slot = dep.duplicate_out_point_tag as usize;
        if seen_tags[slot] {
            return Err(CellDepError::DuplicateOutPoint);
        }
        seen_tags[slot] = true;

        if dep.dep_type == MockDepType::DepGroup && dep.data_hash == CKB_AUTH_CODE_HASH {
            return Err(CellDepError::UnresolvedDepGroup);
        }
        if dep.dep_type == MockDepType::Code
            && dep.data_hash == CKB_AUTH_CODE_HASH
            && found_index.is_none()
        {
            found_index = Some(idx);
        }
    }

    found_index.ok_or(CellDepError::WrongHashOrMissingAuthDep)
}

/// Resolve an arbitrary script `(code_hash, hash_type)` against `cell_deps`.
pub fn resolve_script_dep(
    code_hash: &[u8; 32],
    hash_type: MockHashType,
    cell_deps: &[MockCellDep],
) -> Result<usize, CellDepError> {
    if cell_deps.is_empty() {
        return Err(CellDepError::EmptyCellDeps);
    }
    for (idx, dep) in cell_deps.iter().enumerate() {
        if dep.dep_type != MockDepType::Code {
            continue;
        }
        let matches = match hash_type {
            MockHashType::Data | MockHashType::Data1 | MockHashType::Data2 => {
                &dep.data_hash == code_hash
            }
            MockHashType::Type => dep.type_hash.as_ref() == Some(code_hash),
        };
        if matches {
            return Ok(idx);
        }
    }
    Err(CellDepError::WrongHashOrMissingAuthDep)
}

/// Canonical CKB stealth announcement arguments:
/// `scheme_id: u8 || ephemeral_pubkey[33] || pubkey_hash[20] || view_tag: u8 ||
/// output_index: u32_le || meta_len: u16_le || metadata[meta_len]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnnouncementArgs {
    pub scheme_id: u8,
    pub ephemeral_pubkey: [u8; COMPRESSED_PUBKEY_LEN],
    pub pubkey_hash: [u8; PUBKEY_HASH_LEN],
    pub view_tag: u8,
    pub output_index: u32,
    pub metadata: Vec<u8>,
}

/// Rejection reasons for malformed stealth announcement arguments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnnouncementError {
    TruncatedHeader,
    UnsupportedSchemeId(u8),
    InvalidCompressedPubkeyPrefix(u8),
    MetadataTooLarge(usize),
    TruncatedMetadata,
    TrailingExcessBytes(usize),
    OutputIndexOutOfBounds { output_index: u32, output_count: usize },
}

/// Fixed header length before variable metadata (`1 + 33 + 20 + 1 + 4 + 2 = 61`).
pub const ANNOUNCEMENT_HEADER_LEN: usize = 61;

/// Decode stealth announcement args, rejecting truncation at every field
/// boundary, invalid compressed pubkey prefixes, unsupported scheme IDs, and
/// trailing excess bytes.
pub fn decode_announcement_args(raw: &[u8]) -> Result<AnnouncementArgs, AnnouncementError> {
    if raw.len() < ANNOUNCEMENT_HEADER_LEN {
        return Err(AnnouncementError::TruncatedHeader);
    }
    let scheme_id = raw[0];
    if scheme_id != STEALTH_ANNOUNCEMENT_SCHEME_SECP256K1 {
        return Err(AnnouncementError::UnsupportedSchemeId(scheme_id));
    }

    let prefix = raw[1];
    if prefix != 0x02 && prefix != 0x03 {
        return Err(AnnouncementError::InvalidCompressedPubkeyPrefix(prefix));
    }

    let mut ephemeral_pubkey = [0u8; COMPRESSED_PUBKEY_LEN];
    ephemeral_pubkey.copy_from_slice(&raw[1..34]);

    let mut pubkey_hash = [0u8; PUBKEY_HASH_LEN];
    pubkey_hash.copy_from_slice(&raw[34..54]);

    let view_tag = raw[54];
    let output_index = u32::from_le_bytes([raw[55], raw[56], raw[57], raw[58]]);
    let meta_len = u16::from_le_bytes([raw[59], raw[60]]) as usize;

    if meta_len > MAX_ANNOUNCEMENT_METADATA_LEN {
        return Err(AnnouncementError::MetadataTooLarge(meta_len));
    }

    let remaining = raw.len() - ANNOUNCEMENT_HEADER_LEN;
    if remaining < meta_len {
        return Err(AnnouncementError::TruncatedMetadata);
    }
    if remaining > meta_len {
        return Err(AnnouncementError::TrailingExcessBytes(remaining - meta_len));
    }

    Ok(AnnouncementArgs {
        scheme_id,
        ephemeral_pubkey,
        pubkey_hash,
        view_tag,
        output_index,
        metadata: raw[ANNOUNCEMENT_HEADER_LEN..ANNOUNCEMENT_HEADER_LEN + meta_len].to_vec(),
    })
}

/// Encode `AnnouncementArgs` into canonical wire bytes.
pub fn encode_announcement_args(args: &AnnouncementArgs) -> Result<Vec<u8>, AnnouncementError> {
    if args.scheme_id != STEALTH_ANNOUNCEMENT_SCHEME_SECP256K1 {
        return Err(AnnouncementError::UnsupportedSchemeId(args.scheme_id));
    }
    let prefix = args.ephemeral_pubkey[0];
    if prefix != 0x02 && prefix != 0x03 {
        return Err(AnnouncementError::InvalidCompressedPubkeyPrefix(prefix));
    }
    if args.metadata.len() > MAX_ANNOUNCEMENT_METADATA_LEN {
        return Err(AnnouncementError::MetadataTooLarge(args.metadata.len()));
    }
    let meta_len = args.metadata.len() as u16;
    let mut out = Vec::with_capacity(ANNOUNCEMENT_HEADER_LEN + args.metadata.len());
    out.push(args.scheme_id);
    out.extend_from_slice(&args.ephemeral_pubkey);
    out.extend_from_slice(&args.pubkey_hash);
    out.push(args.view_tag);
    out.extend_from_slice(&args.output_index.to_le_bytes());
    out.extend_from_slice(&meta_len.to_le_bytes());
    out.extend_from_slice(&args.metadata);
    Ok(out)
}

/// Validate that an announcement's `output_index` points to an existing stealth
/// output cell whose lock args commit to the announced `ephemeral_pubkey` and
/// `pubkey_hash`.
pub fn verify_announcement_output_index(
    ann: &AnnouncementArgs,
    outputs_lock_args: &[Vec<u8>],
) -> Result<(), AnnouncementError> {
    let idx = usize::try_from(ann.output_index).map_err(|_| {
        AnnouncementError::OutputIndexOutOfBounds {
            output_index: ann.output_index,
            output_count: outputs_lock_args.len(),
        }
    })?;
    let target_args = outputs_lock_args.get(idx).ok_or(
        AnnouncementError::OutputIndexOutOfBounds {
            output_index: ann.output_index,
            output_count: outputs_lock_args.len(),
        },
    )?;
    if target_args.len() != STEALTH_LOCK_ARGS_LEN {
        return Err(AnnouncementError::TruncatedHeader);
    }
    if target_args[0..33] != ann.ephemeral_pubkey || target_args[33..53] != ann.pubkey_hash {
        return Err(AnnouncementError::TruncatedHeader);
    }
    Ok(())
}

// ── 5. Full CKB Transaction Layout & Integer-Boundary Model ──────────────────

/// Full untrusted CKB transaction bundle driven by `ckb_tx_layout`.
#[derive(Clone, Debug, Arbitrary)]
pub struct FuzzTxLayout {
    pub tx_hash: [u8; 32],
    pub stealth_lock_hash: [u8; 32],
    pub stealth_lock_args: Vec<u8>,
    pub names_type_hash: [u8; 32],
    pub names_type_args: Vec<u8>,
    pub inputs: Vec<MockCell>,
    pub outputs: Vec<MockCell>,
    pub outputs_lock_args: Vec<Vec<u8>>,
    pub witnesses: Vec<Vec<u8>>,
    pub cell_deps: Vec<MockCellDep>,
    pub claimed_output_indices: Vec<u32>,
    pub raw_announcement: Vec<u8>,
}

/// Transaction-level rejection reasons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TxLayoutError {
    EmptyInputs,
    EmptyOutputs,
    InputCapacityOverflow,
    OutputCapacityOverflow,
    BelowMinimumCellCapacity,
    OutputExceedsInputCapacity,
    OutputIndexOutOfBounds,
    DuplicateClaimedOutputIndex,
    LockFailed(LockScriptError),
    NamesTypeFailed(NamesTypeError),
    AnnouncementFailed(AnnouncementError),
}

/// Validated transaction summary used to assert post-condition invariants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedTxSummary {
    pub total_input_capacity: u64,
    pub total_output_capacity: u64,
    pub fee: u64,
    pub stealth_inputs_unlocked: usize,
    pub name_transition: Option<NameTransition>,
}

/// Validate capacity conservation (`u64::checked_add`), output index bounds,
/// stealth lock groups, `.wraith` singleton name cells, and optional stealth
/// announcements without panicking or over-indexing.
pub fn evaluate_tx_layout(tx: &FuzzTxLayout) -> Result<VerifiedTxSummary, TxLayoutError> {
    if tx.inputs.is_empty() {
        return Err(TxLayoutError::EmptyInputs);
    }
    if tx.outputs.is_empty() {
        return Err(TxLayoutError::EmptyOutputs);
    }

    let mut total_in = 0u64;
    for cell in &tx.inputs {
        if cell.capacity < MIN_CELL_CAPACITY_SHANNONS {
            return Err(TxLayoutError::BelowMinimumCellCapacity);
        }
        total_in = total_in
            .checked_add(cell.capacity)
            .ok_or(TxLayoutError::InputCapacityOverflow)?;
    }

    let mut total_out = 0u64;
    for cell in &tx.outputs {
        if cell.capacity < MIN_CELL_CAPACITY_SHANNONS {
            return Err(TxLayoutError::BelowMinimumCellCapacity);
        }
        total_out = total_out
            .checked_add(cell.capacity)
            .ok_or(TxLayoutError::OutputCapacityOverflow)?;
    }

    if total_out > total_in {
        return Err(TxLayoutError::OutputExceedsInputCapacity);
    }

    // Validate claimed output indices: strictly in-bounds and duplicate-free.
    let mut seen_indices = std::collections::BTreeSet::new();
    for &raw_idx in &tx.claimed_output_indices {
        let idx = usize::try_from(raw_idx).map_err(|_| TxLayoutError::OutputIndexOutOfBounds)?;
        if idx >= tx.outputs.len() {
            return Err(TxLayoutError::OutputIndexOutOfBounds);
        }
        if !seen_indices.insert(idx) {
            return Err(TxLayoutError::DuplicateClaimedOutputIndex);
        }
    }

    // Execute stealth-lock group if any input cell uses `stealth_lock_hash`.
    let stealth_input_indices: Vec<usize> = tx
        .inputs
        .iter()
        .enumerate()
        .filter_map(|(i, c)| (c.lock_hash == tx.stealth_lock_hash).then_some(i))
        .collect();

    if !stealth_input_indices.is_empty() {
        let first_idx = stealth_input_indices[0];
        let group_witnesses: Vec<Vec<u8>> = stealth_input_indices
            .iter()
            .filter_map(|&idx| tx.witnesses.get(idx).cloned())
            .collect();
        if tx.witnesses.get(first_idx).is_none() {
            return Err(TxLayoutError::LockFailed(LockScriptError::IndexOutOfBound));
        }
        validate_stealth_lock_invocation(
            &tx.stealth_lock_args,
            &group_witnesses,
            &tx.tx_hash,
            &tx.cell_deps,
        )
        .map_err(TxLayoutError::LockFailed)?;
    }

    // Execute `wraith-names-type` if any input or output carries `names_type_hash`.
    let touches_names = tx
        .inputs
        .iter()
        .chain(tx.outputs.iter())
        .any(|c| c.type_hash == Some(tx.names_type_hash));

    let name_transition = if touches_names {
        Some(
            validate_names_type_script(
                &tx.names_type_args,
                &tx.names_type_hash,
                &tx.inputs,
                &tx.outputs,
            )
            .map_err(TxLayoutError::NamesTypeFailed)?,
        )
    } else {
        None
    };

    if !tx.raw_announcement.is_empty() {
        let ann = decode_announcement_args(&tx.raw_announcement)
            .map_err(TxLayoutError::AnnouncementFailed)?;
        verify_announcement_output_index(&ann, &tx.outputs_lock_args)
            .map_err(TxLayoutError::AnnouncementFailed)?;
    }

    Ok(VerifiedTxSummary {
        total_input_capacity: total_in,
        total_output_capacity: total_out,
        fee: total_in - total_out,
        stealth_inputs_unlocked: stealth_input_indices.len(),
        name_transition,
    })
}

// ── Target Entrypoints Shared Between `cargo-fuzz` and `cargo test` ──────────

/// Entrypoint for `fuzz_targets/lock_witness_and_args.rs`.
pub fn fuzz_entry_lock_witness_and_args(data: &[u8]) {
    if let Ok(decoded) = decode_witness_args(data) {
        let reencoded =
            encode_witness_args(&decoded).expect("decoded WitnessArgs must re-encode cleanly");
        assert_eq!(
            reencoded, data,
            "Molecule WitnessArgs wire format must have a unique canonical encoding"
        );
        let reparsed = decode_witness_args(&reencoded).expect("canonical bytes must decode");
        assert_eq!(decoded, reparsed);
    }

    if data.len() >= STEALTH_LOCK_ARGS_LEN {
        let split = STEALTH_LOCK_ARGS_LEN;
        let args_slice = &data[..split];
        let wit_slice = &data[split..];
        let parsed = parse_stealth_lock_args(args_slice).expect("53-byte slice must parse");
        assert_eq!(&parsed.ephemeral_pubkey[..], &args_slice[0..33]);
        assert_eq!(&parsed.pubkey_hash[..], &args_slice[33..53]);

        let dep = MockCellDep {
            dep_type: MockDepType::Code,
            data_hash: CKB_AUTH_CODE_HASH,
            type_hash: None,
            duplicate_out_point_tag: 0,
        };
        let _ = validate_stealth_lock_invocation(
            args_slice,
            &[wit_slice.to_vec()],
            &[0x42u8; 32],
            &[dep],
        );
    } else {
        assert_eq!(
            parse_stealth_lock_args(data),
            Err(LockScriptError::ArgsLengthNotEnough)
        );
    }
}

/// Structured input for `fuzz_targets/names_type_script.rs`.
#[derive(Clone, Debug, Arbitrary)]
pub struct NamesFuzzInput {
    pub script_args: Vec<u8>,
    pub active_script_hash: [u8; 32],
    pub inputs: Vec<MockCell>,
    pub outputs: Vec<MockCell>,
}

/// Entrypoint for `fuzz_targets/names_type_script.rs`.
pub fn fuzz_entry_names_type_script(data: &[u8]) {
    let mut u = Unstructured::new(data);
    if let Ok(case) = NamesFuzzInput::arbitrary(&mut u) {
        if let Ok(transition) = validate_names_type_script(
            &case.script_args,
            &case.active_script_hash,
            &case.inputs,
            &case.outputs,
        ) {
            assert_eq!(case.script_args.len(), NAMES_TYPE_ARGS_LEN);
            let in_matches = case
                .inputs
                .iter()
                .filter(|c| c.type_hash == Some(case.active_script_hash))
                .count();
            let out_matches = case
                .outputs
                .iter()
                .filter(|c| c.type_hash == Some(case.active_script_hash))
                .count();
            assert!(in_matches <= 1, "accepted transition had duplicate inputs");
            assert!(out_matches <= 1, "accepted transition had duplicate outputs");
            assert!(
                in_matches + out_matches >= 1,
                "accepted transition had zero matching cells"
            );
            match transition {
                NameTransition::Create { output_index } => {
                    assert_eq!(in_matches, 0);
                    assert_eq!(out_matches, 1);
                    assert_eq!(case.outputs[output_index].data.len(), NAMES_CELL_DATA_LEN);
                }
                NameTransition::Update {
                    input_index,
                    output_index,
                } => {
                    assert_eq!(in_matches, 1);
                    assert_eq!(out_matches, 1);
                    assert!(input_index < case.inputs.len());
                    assert_eq!(case.outputs[output_index].data.len(), NAMES_CELL_DATA_LEN);
                }
                NameTransition::Destroy { input_index } => {
                    assert_eq!(in_matches, 1);
                    assert_eq!(out_matches, 0);
                    assert!(input_index < case.inputs.len());
                }
            }
        }
    }
}

/// Entrypoint for `fuzz_targets/ckb_auth_inputs.rs`.
pub fn fuzz_entry_ckb_auth_inputs(data: &[u8]) {
    let mut u = Unstructured::new(data);
    if let Ok(argv) = RawCkbAuthArgv::arbitrary(&mut u) {
        if let Ok(inv) = parse_and_validate_ckb_auth_argv(&argv) {
            assert_eq!(inv.algorithm_id, CKB_AUTH_SECP256K1_ALGO_ID);
            assert!(inv.signature[0] < 4);
            assert!(!inv.signature[1..33].iter().all(|b| *b == 0));
            assert!(!inv.signature[33..65].iter().all(|b| *b == 0));
        }
    }
}

/// Entrypoint for `fuzz_targets/cell_deps_and_announcement.rs`.
pub fn fuzz_entry_cell_deps_and_announcement(data: &[u8]) {
    if let Ok(ann) = decode_announcement_args(data) {
        let encoded = encode_announcement_args(&ann).expect("decoded announcement must encode");
        assert_eq!(encoded, data, "announcement wire codec must be canonical");
        let reparsed = decode_announcement_args(&encoded).expect("canonical bytes must decode");
        assert_eq!(ann, reparsed);
    }

    let mut u = Unstructured::new(data);
    if let Ok((code_hash, hash_type, deps, outputs_lock_args)) =
        <([u8; 32], MockHashType, Vec<MockCellDep>, Vec<Vec<u8>>)>::arbitrary(&mut u)
    {
        if let Ok(idx) = resolve_ckb_auth_dep(&deps) {
            assert_eq!(deps[idx].dep_type, MockDepType::Code);
            assert_eq!(deps[idx].data_hash, CKB_AUTH_CODE_HASH);
        }
        if let Ok(idx) = resolve_script_dep(&code_hash, hash_type, &deps) {
            assert!(idx < deps.len());
            assert_eq!(deps[idx].dep_type, MockDepType::Code);
        }
        if let Ok(ann) = decode_announcement_args(data) {
            let _ = verify_announcement_output_index(&ann, &outputs_lock_args);
        }
    }
}

/// Entrypoint for `fuzz_targets/ckb_tx_layout.rs`.
pub fn fuzz_entry_ckb_tx_layout(data: &[u8]) {
    let mut u = Unstructured::new(data);
    if let Ok(layout) = FuzzTxLayout::arbitrary(&mut u) {
        if let Ok(summary) = evaluate_tx_layout(&layout) {
            assert!(summary.total_input_capacity >= summary.total_output_capacity);
            assert_eq!(
                summary.total_output_capacity + summary.fee,
                summary.total_input_capacity
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

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

    fn corpus_dir(target: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("corpus")
            .join(target)
    }

    fn replay_corpus_directory(target: &str, runner: fn(&[u8])) -> usize {
        let dir = corpus_dir(target);
        let mut entries: Vec<PathBuf> = fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("missing corpus directory {}: {e}", dir.display()))
            .filter_map(|e| e.ok().map(|d| d.path()))
            .filter(|p| p.is_file())
            .collect();
        entries.sort();
        assert!(
            !entries.is_empty(),
            "corpus directory {} must contain seed/regression inputs",
            dir.display()
        );
        for path in &entries {
            let bytes = fs::read(path).expect("read corpus seed");
            runner(&bytes);
        }
        entries.len()
    }

    #[test]
    fn replay_all_committed_seed_and_regression_corpora() {
        let c1 = replay_corpus_directory("lock_witness_and_args", fuzz_entry_lock_witness_and_args);
        let c2 = replay_corpus_directory("names_type_script", fuzz_entry_names_type_script);
        let c3 = replay_corpus_directory("ckb_auth_inputs", fuzz_entry_ckb_auth_inputs);
        let c4 = replay_corpus_directory(
            "cell_deps_and_announcement",
            fuzz_entry_cell_deps_and_announcement,
        );
        let c5 = replay_corpus_directory("ckb_tx_layout", fuzz_entry_ckb_tx_layout);
        assert!(c1 >= 6 && c2 >= 6 && c3 >= 6 && c4 >= 6 && c5 >= 6);
    }

    #[test]
    fn witness_args_truncation_excess_and_integer_boundaries_rejected() {
        let canonical = encode_witness_args(&DecodedWitnessArgs {
            lock: Some(vec![0x11u8; SIGNATURE_LEN]),
            input_type: None,
            output_type: Some(vec![0xAB, 0xCD]),
        })
        .expect("encode");

        // Truncation at every byte boundary from 0..canonical.len()
        for cut in 0..canonical.len() {
            assert_eq!(
                decode_witness_args(&canonical[..cut]),
                Err(LockScriptError::Encoding),
                "truncation at byte {cut} must fail cleanly"
            );
        }

        // Trailing excess bytes
        let mut with_excess = canonical.clone();
        with_excess.push(0x00);
        assert_eq!(
            decode_witness_args(&with_excess),
            Err(LockScriptError::Encoding)
        );

        // u32::MAX inner BytesOpt length
        let mut max_inner = Vec::new();
        max_inner.extend_from_slice(&20u32.to_le_bytes());
        max_inner.extend_from_slice(&16u32.to_le_bytes());
        max_inner.extend_from_slice(&20u32.to_le_bytes());
        max_inner.extend_from_slice(&20u32.to_le_bytes());
        max_inner.extend_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(
            decode_witness_args(&max_inner),
            Err(LockScriptError::Encoding)
        );
    }

    #[test]
    fn invalid_algorithm_ids_and_malformed_ckb_auth_inputs_rejected() {
        let mut valid_sig = [0x22u8; SIGNATURE_LEN];
        valid_sig[0] = 1;
        let hex_encode = |bytes: &[u8]| -> Vec<u8> {
            bytes
                .iter()
                .flat_map(|b| format!("{b:02x}").into_bytes())
                .collect()
        };

        let base = RawCkbAuthArgv {
            algorithm_id_arg: b"00".to_vec(),
            signature_arg: hex_encode(&valid_sig),
            message_arg: hex_encode(&[0x33u8; 32]),
            pubkey_hash_arg: hex_encode(&[0x44u8; PUBKEY_HASH_LEN]),
        };
        assert!(parse_and_validate_ckb_auth_argv(&base).is_ok());

        // Every non-zero algorithm ID (1..=255) must be rejected
        for bad_algo in 1u8..=u8::MAX {
            let mut bad = base.clone();
            bad.algorithm_id_arg = format!("{bad_algo:02x}").into_bytes();
            assert_eq!(
                parse_and_validate_ckb_auth_argv(&bad),
                Err(CkbAuthError::UnsupportedAlgorithmId(bad_algo))
            );
        }

        // Recovery ID >= 4 rejected
        for recid in [4u8, 27, 28, u8::MAX] {
            let mut bad_sig = valid_sig;
            bad_sig[0] = recid;
            let mut bad = base.clone();
            bad.signature_arg = hex_encode(&bad_sig);
            assert_eq!(
                parse_and_validate_ckb_auth_argv(&bad),
                Err(CkbAuthError::InvalidRecoveryId(recid))
            );
        }

        // Interior NUL byte rejected
        let mut nul_argv = base.clone();
        nul_argv.algorithm_id_arg = vec![b'0', 0, b'0'];
        assert_eq!(
            parse_and_validate_ckb_auth_argv(&nul_argv),
            Err(CkbAuthError::InteriorNulByte)
        );
    }

    #[test]
    fn names_type_rejects_duplicated_outputs_inputs_and_zero_matches() {
        let type_hash = [0x77u8; 32];
        let args = [0xAAu8; NAMES_TYPE_ARGS_LEN];
        let valid_cell = MockCell {
            capacity: MIN_CELL_CAPACITY_SHANNONS,
            lock_hash: [0x11u8; 32],
            type_hash: Some(type_hash),
            data: vec![0x02u8; NAMES_CELL_DATA_LEN],
        };

        assert_eq!(
            validate_names_type_script(&args, &type_hash, &[], &[valid_cell.clone()]),
            Ok(NameTransition::Create { output_index: 0 })
        );
        assert_eq!(
            validate_names_type_script(
                &args,
                &type_hash,
                &[],
                &[valid_cell.clone(), valid_cell.clone()],
            ),
            Err(NamesTypeError::DuplicateNameCell)
        );
        assert_eq!(
            validate_names_type_script(
                &args,
                &type_hash,
                &[valid_cell.clone(), valid_cell.clone()],
                &[valid_cell],
            ),
            Err(NamesTypeError::DuplicateNameCell)
        );
        assert_eq!(
            validate_names_type_script(&args, &type_hash, &[], &[]),
            Err(NamesTypeError::NoMatchingCells)
        );
    }

    #[test]
    fn announcement_truncation_excess_wrong_dep_hash_and_u64_overflow_rejected() {
        let mut eph = [0x99u8; COMPRESSED_PUBKEY_LEN];
        eph[0] = 0x02;
        let ann = AnnouncementArgs {
            scheme_id: STEALTH_ANNOUNCEMENT_SCHEME_SECP256K1,
            ephemeral_pubkey: eph,
            pubkey_hash: [0x55u8; PUBKEY_HASH_LEN],
            view_tag: 0xAB,
            output_index: 0,
            metadata: vec![1, 2, 3, 4],
        };
        let encoded = encode_announcement_args(&ann).expect("encode");
        assert_eq!(decode_announcement_args(&encoded), Ok(ann));

        for cut in 0..encoded.len() {
            assert!(decode_announcement_args(&encoded[..cut]).is_err());
        }
        let mut excess = encoded.clone();
        excess.push(0xFF);
        assert_eq!(
            decode_announcement_args(&excess),
            Err(AnnouncementError::TrailingExcessBytes(1))
        );

        // Wrong cell-dep hash rejected
        let wrong_dep = MockCellDep {
            dep_type: MockDepType::Code,
            data_hash: [0xFEu8; 32],
            type_hash: None,
            duplicate_out_point_tag: 0,
        };
        assert_eq!(
            resolve_ckb_auth_dep(&[wrong_dep]),
            Err(CellDepError::WrongHashOrMissingAuthDep)
        );

        // u64::MAX capacity sum overflow rejected cleanly
        let overflow_cell = MockCell {
            capacity: u64::MAX,
            lock_hash: [0x01u8; 32],
            type_hash: None,
            data: Vec::new(),
        };
        let tx = FuzzTxLayout {
            tx_hash: [0u8; 32],
            stealth_lock_hash: [0xEEu8; 32],
            stealth_lock_args: Vec::new(),
            names_type_hash: [0xDDu8; 32],
            names_type_args: Vec::new(),
            inputs: vec![overflow_cell.clone(), overflow_cell.clone()],
            outputs: vec![overflow_cell],
            outputs_lock_args: Vec::new(),
            witnesses: Vec::new(),
            cell_deps: Vec::new(),
            claimed_output_indices: vec![0],
            raw_announcement: Vec::new(),
        };
        assert_eq!(
            evaluate_tx_layout(&tx),
            Err(TxLayoutError::InputCapacityOverflow)
        );
    }

    #[test]
    fn bounded_deterministic_sample_across_all_five_targets() {
        let cases: usize = std::env::var("WRAITH_CKB_FUZZ_CASES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2048);
        let mut rng = SplitMix64(0x0057_5241_4954_0281);

        for i in 0..cases {
            let len = (rng.next_u64() as usize) % 256;
            let buf = rng.gen_bytes(len);
            match i % 5 {
                0 => fuzz_entry_lock_witness_and_args(&buf),
                1 => fuzz_entry_names_type_script(&buf),
                2 => fuzz_entry_ckb_auth_inputs(&buf),
                3 => fuzz_entry_cell_deps_and_announcement(&buf),
                _ => fuzz_entry_ckb_tx_layout(&buf),
            }
        }
    }
}
