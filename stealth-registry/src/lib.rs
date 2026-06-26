mod pausable;
use pausable::{pause, unpause, is_paused, require_not_paused, set_admin};

// In the trait
fn pause(env: Env, admin: Address);
fn unpause(env: Env, admin: Address);
fn is_paused(env: Env) -> bool;

// In initialize/constructor — add:
set_admin(&env, &admin);

// In impl
fn pause(env: Env, admin: Address) {
    pause(&env, &admin);
}

fn unpause(env: Env, admin: Address) {
    unpause(&env, &admin);
}

fn is_paused(env: Env) -> bool {
    is_paused(&env)
}

// At top of register() and any state-mutating fn:
fn register(env: Env, ...) {
    require_not_paused(&env);
    // ... rest of fn
}