#![cfg_attr(not(any(feature = "library", test)), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(any(feature = "library", test))]
extern crate alloc;

use alloc::ffi::CString;
#[cfg(not(feature = "native-simulator"))]
use ckb_std::high_level::exec_cell;
use ckb_std::{
    ckb_constants::Source,
    ckb_types::core::ScriptHashType,
    error::SysError,
    high_level::{load_script, load_tx_hash, load_witness_args},
};
#[cfg(feature = "native-simulator")]
use ckb_std::{high_level::spawn_cell, syscalls::wait};
use hex::encode;

#[cfg(not(any(feature = "library", test)))]
ckb_std::entry!(program_entry);
#[cfg(not(any(feature = "library", test)))]
ckb_std::default_alloc!(16384, 1258306, 64);

/// ckb-auth code hash deployed on CKB testnet.
/// Cell dep: tx_hash 0xa0e99b29fd154385815142b76668d5f4ecf30ae85bc2942bd21e9e51b9066f97, index 0
const CKB_AUTH_CODE_HASH: [u8; 32] = [
    0x09, 0x15, 0x98, 0x3b, 0xb3, 0x15, 0x84, 0xdf, 0x45, 0x66, 0xe0, 0x94, 0x6f, 0xd0, 0x0e,
    0xf1, 0xe9, 0xa7, 0x5a, 0xd3, 0x7a, 0x39, 0xce, 0x70, 0xfe, 0xc9, 0xb5, 0xcb, 0xf3, 0xb8,
    0x70, 0x21,
];

#[repr(i8)]
pub enum Error {
    IndexOutOfBound = 1,
    ItemMissing,
    LengthNotEnough,
    Encoding,
    ArgsLengthNotEnough = 5,
    SignatureLengthNotEnough,
    AuthError,
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

/// Entry point for the wraith-stealth-lock script.
///
/// Verifies a secp256k1 signature against the blake160(stealth_pubkey) stored
/// in the lock script args. The args layout is 53 bytes:
///   args[0..33]  = ephemeral public key (used off-chain for stealth address scanning)
///   args[33..53] = blake160(stealth_pubkey) (used on-chain for signature verification)
///
/// Delegates signature verification to the on-chain ckb-auth cell via exec_cell.
pub fn program_entry() -> i8 {
    match auth() {
        Ok(_) => 0,
        Err(err) => err as i8,
    }
}

fn auth() -> Result<(), Error> {
    // algorithm_id = 0 means secp256k1/blake160
    let algorithm_id_str = CString::new(encode([0u8])).unwrap();

    let signature_str = {
        let signature = {
            let witness_args = load_witness_args(0, Source::GroupInput)?;
            let signature = witness_args
                .lock()
                .to_opt()
                .map(|b| b.raw_data())
                .unwrap_or_default();
            if signature.len() != 65 {
                return Err(Error::SignatureLengthNotEnough);
            }
            signature
        };
        CString::new(encode(signature)).unwrap()
    };

    let message_str = {
        let message = load_tx_hash()?;
        CString::new(encode(message)).unwrap()
    };

    let pubkey_hash_str = {
        let pubkey_hash = {
            let mut hash = [0u8; 20];
            let script_args = load_script()?.args().raw_data();
            // args = ephemeral_pubkey (33 bytes) || blake160(stealth_pubkey) (20 bytes)
            if script_args.len() != 53 {
                return Err(Error::ArgsLengthNotEnough);
            }
            hash.copy_from_slice(&script_args[33..53]);
            hash
        };
        CString::new(encode(pubkey_hash)).unwrap()
    };

    let args = [
        algorithm_id_str.as_c_str(),
        signature_str.as_c_str(),
        message_str.as_c_str(),
        pubkey_hash_str.as_c_str(),
    ];

    #[cfg(feature = "native-simulator")]
    {
        let pid = spawn_cell(&CKB_AUTH_CODE_HASH, ScriptHashType::Data2, &args, &[])
            .map_err(|_| Error::AuthError)?;
        let exit_code = wait(pid).map_err(|_| Error::AuthError)?;
        if exit_code == 0 {
            Ok(())
        } else {
            Err(Error::AuthError)
        }
    }
    #[cfg(not(feature = "native-simulator"))]
    {
        exec_cell(&CKB_AUTH_CODE_HASH, ScriptHashType::Data2, &args)
            .map_err(|_| Error::AuthError)?;
        Ok(())
    }
}
