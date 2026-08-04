use soroban_sdk::{symbol_short, Address, Env};

const PAUSED_KEY: &str = "PAUSED";
const ADMIN_KEY: &str = "ADMIN";

/// Store the pause admin at contract init
pub fn set_admin(env: &Env, admin: &Address) {
    env.storage()
        .instance()
        .set(&symbol_short!(ADMIN_KEY), admin);
}

/// Get the pause admin
pub fn get_admin(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&symbol_short!(ADMIN_KEY))
        .expect("admin not set")
}

/// Pause the contract — admin only
pub fn pause(env: &Env, caller: &Address) {
    caller.require_auth();
    let admin = get_admin(env);
    if caller != &admin {
        panic!("unauthorized: only admin can pause");
    }
    env.storage()
        .instance()
        .set(&symbol_short!(PAUSED_KEY), &true);
    env.events()
        .publish((symbol_short!("paused"),), (caller.clone(),));
}

/// Unpause the contract — admin only
pub fn unpause(env: &Env, caller: &Address) {
    caller.require_auth();
    let admin = get_admin(env);
    if caller != &admin {
        panic!("unauthorized: only admin can unpause");
    }
    env.storage()
        .instance()
        .set(&symbol_short!(PAUSED_KEY), &false);
    env.events()
        .publish((symbol_short!("unpaused"),), (caller.clone(),));
}

/// Returns true if the contract is paused
pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&symbol_short!(PAUSED_KEY))
        .unwrap_or(false)
}

/// Call at the top of any state-mutating function
pub fn require_not_paused(env: &Env) {
    if is_paused(env) {
        panic!("contract is paused");
    }
}