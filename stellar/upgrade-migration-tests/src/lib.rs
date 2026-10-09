#![allow(clippy::module_name_repetitions)]

use soroban_sdk::{Address, BytesN, Env};

pub mod wasm {
    include!(concat!(env!("OUT_DIR"), "/wasm_manifest.rs"));
}

/// Upload `wasm` bytes and replace the contract at `contract_id` with that implementation.
pub fn upgrade_contract_wasm(env: &Env, contract_id: &Address, wasm: &[u8]) {
    let wasm_hash = env.deployer().upload_contract_wasm(wasm);
    env.as_contract(contract_id, || {
        env.deployer().update_current_contract_wasm(&wasm_hash);
    });
}

/// Deploy a contract from pinned v0/v1 WASM bytecode.
pub fn deploy_wasm(env: &Env, wasm: &[u8]) -> Address {
    env.register_contract_wasm(None, wasm)
}
