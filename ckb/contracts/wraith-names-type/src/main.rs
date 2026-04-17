#![cfg_attr(not(any(feature = "library", test)), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(any(feature = "library", test))]
extern crate alloc;

use ckb_std::{
    ckb_constants::Source,
    error::SysError,
    high_level::{load_cell_data, load_script, QueryIter},
};

#[cfg(not(any(feature = "library", test)))]
ckb_std::entry!(program_entry);
#[cfg(not(any(feature = "library", test)))]
ckb_std::default_alloc!(16384, 1258306, 64);

/// Expected cell data length: spending_pub_key (33 bytes) + viewing_pub_key (33 bytes).
const NAME_DATA_LEN: usize = 66;

#[repr(i8)]
pub enum Error {
    IndexOutOfBound = 1,
    ItemMissing,
    LengthNotEnough,
    Encoding,
    /// Cell data is not exactly 66 bytes (two compressed secp256k1 public keys).
    InvalidDataLength = 5,
}

impl From<SysError> for Error {
    fn from(err: SysError) -> Self {
        match err {
            SysError::IndexOutOfBound => Self::IndexOutOfBound,
            SysError::ItemMissing => Self::ItemMissing,
            SysError::LengthNotEnough(_) => Self::LengthNotEnough,
            SysError::Encoding => Self::Encoding,
            SysError::Unknown(err_code) => panic!("unexpected sys error {}", err_code),
            _ => panic!("unreachable spawn related sys error"),
        }
    }
}

/// Entry point for the wraith-names-type script.
///
/// This is a Type Script that validates `.wraith` name registration cells.
///
/// Each name cell has:
///   - type script args: blake2b hash of the name string (first 32 bytes)
///   - cell data: 66 bytes = spending_pub_key (33) + viewing_pub_key (33)
///   - lock script: owner's lock (proves ownership by consuming the cell)
///
/// Validation rules:
///   - Create (no input, has output): cell data must be exactly 66 bytes
///   - Update (has input, has output): cell data must be exactly 66 bytes
///   - Destroy (has input, no output): always allowed (name release)
///
/// Name format validation (3-32 chars, lowercase alphanumeric) is handled
/// off-chain in the SDK since the type args is already a hash.
///
/// Ownership is proven by the cell's lock script, so no signature
/// verification is needed here.
pub fn program_entry() -> i8 {
    match validate() {
        Ok(_) => 0,
        Err(err) => err as i8,
    }
}

fn validate() -> Result<(), Error> {
    let script = load_script()?;
    let script_hash = script.calc_script_hash();

    let has_input = has_cell_with_type_hash(&script_hash, Source::Input);
    let has_output = has_cell_with_type_hash(&script_hash, Source::Output);

    match (has_input, has_output) {
        (false, true) => validate_output_data(),
        (true, true) => validate_output_data(),
        (true, false) => Ok(()),
        (false, false) => panic!("type script invoked but no matching cells"),
    }
}

/// Check that the output cell data is exactly 66 bytes.
fn validate_output_data() -> Result<(), Error> {
    let script = load_script()?;
    let script_hash = script.calc_script_hash();

    for (i, _) in QueryIter::new(load_cell_data, Source::Output).enumerate() {
        let type_hash = ckb_std::high_level::load_cell_type_hash(i, Source::Output)?;
        if let Some(hash) = type_hash {
            if hash == script_hash.as_reader().raw_data().as_ref() {
                let data = load_cell_data(i, Source::Output)?;
                if data.len() != NAME_DATA_LEN {
                    return Err(Error::InvalidDataLength);
                }
            }
        }
    }

    Ok(())
}

/// Check whether any cell in the given source has a type script hash matching ours.
fn has_cell_with_type_hash(
    script_hash: &ckb_std::ckb_types::packed::Byte32,
    source: Source,
) -> bool {
    let target: [u8; 32] = script_hash.as_reader().raw_data().try_into().unwrap();
    for i in 0.. {
        match ckb_std::high_level::load_cell_type_hash(i, source) {
            Ok(Some(hash)) if hash == target => return true,
            Ok(_) => continue,
            Err(SysError::IndexOutOfBound) => break,
            Err(_) => break,
        }
    }
    false
}
