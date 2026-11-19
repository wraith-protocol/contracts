#![cfg_attr(not(any(feature = "library", test)), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(any(feature = "library", test))]
extern crate alloc;

use ckb_std::{
    ckb_constants::Source,
    error::SysError,
    high_level::{load_cell_data, load_cell_type_hash, load_script},
};

#[cfg(not(any(feature = "library", test)))]
ckb_std::entry!(program_entry);
#[cfg(not(any(feature = "library", test)))]
ckb_std::default_alloc!(16384, 1258306, 64);

/// Expected type script args length: blake2b hash of the name string (32 bytes).
const NAME_ARGS_LEN: usize = 32;

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
    /// Type script args is not exactly 32 bytes (blake2b name hash).
    InvalidArgsLength = 6,
    /// More than one input or output cell carries the same name type script hash.
    DuplicateNameCell = 7,
    /// Type script was invoked without any matching input or output cell.
    NoMatchingCells = 8,
    /// Unexpected CKB syscall failure mapped cleanly without panicking.
    UnexpectedSysError = 9,
}

impl From<SysError> for Error {
    fn from(err: SysError) -> Self {
        match err {
            SysError::IndexOutOfBound => Self::IndexOutOfBound,
            SysError::ItemMissing => Self::ItemMissing,
            SysError::LengthNotEnough(_) => Self::LengthNotEnough,
            SysError::Encoding => Self::Encoding,
            SysError::Unknown(_) => Self::UnexpectedSysError,
            _ => Self::UnexpectedSysError,
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
///   - Type script args must be exactly 32 bytes
///   - Singleton cell uniqueness: at most 1 matching input and at most 1 matching output
///   - Create (0 inputs, 1 output): cell data must be exactly 66 bytes
///   - Update (1 input, 1 output): cell data must be exactly 66 bytes
///   - Destroy (1 input, 0 outputs): allowed (name release)
pub fn program_entry() -> i8 {
    match validate() {
        Ok(_) => 0,
        Err(err) => err as i8,
    }
}

fn validate() -> Result<(), Error> {
    let script = load_script()?;
    if script.args().raw_data().len() != NAME_ARGS_LEN {
        return Err(Error::InvalidArgsLength);
    }

    let script_hash = script.calc_script_hash();
    let input_count = count_cells_with_type_hash(&script_hash, Source::Input)?;
    let output_count = count_cells_with_type_hash(&script_hash, Source::Output)?;

    match (input_count, output_count) {
        (0, 0) => Err(Error::NoMatchingCells),
        (inputs, _) if inputs > 1 => Err(Error::DuplicateNameCell),
        (_, outputs) if outputs > 1 => Err(Error::DuplicateNameCell),
        (0, 1) | (1, 1) => validate_output_data(&script_hash),
        (1, 0) => Ok(()),
        _ => Err(Error::NoMatchingCells),
    }
}

/// Check that every matching output cell carries exactly 66 bytes of data.
fn validate_output_data(script_hash: &ckb_std::ckb_types::packed::Byte32) -> Result<(), Error> {
    let target = script_hash.as_reader().raw_data();
    let mut index = 0usize;
    loop {
        match load_cell_type_hash(index, Source::Output) {
            Ok(Some(hash)) => {
                if hash.as_slice() == target {
                    let data = load_cell_data(index, Source::Output)?;
                    if data.len() != NAME_DATA_LEN {
                        return Err(Error::InvalidDataLength);
                    }
                }
            }
            Ok(None) => {}
            Err(SysError::IndexOutOfBound) => break,
            Err(err) => return Err(Error::from(err)),
        }
        index = match index.checked_add(1) {
            Some(next) => next,
            None => break,
        };
    }

    Ok(())
}

/// Count how many cells in `source` carry a type script hash matching `script_hash`.
fn count_cells_with_type_hash(
    script_hash: &ckb_std::ckb_types::packed::Byte32,
    source: Source,
) -> Result<usize, Error> {
    let target = script_hash.as_reader().raw_data();
    let mut count = 0usize;
    let mut index = 0usize;
    loop {
        match load_cell_type_hash(index, source) {
            Ok(Some(hash)) => {
                if hash.as_slice() == target {
                    count = count.saturating_add(1);
                }
            }
            Ok(None) => {}
            Err(SysError::IndexOutOfBound) => break,
            Err(err) => return Err(Error::from(err)),
        }
        index = match index.checked_add(1) {
            Some(next) => next,
            None => break,
        };
    }
    Ok(count)
}
