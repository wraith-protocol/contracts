/// Mock: AUTH_IMMUTABLE + AUTH_REQUIRED asset — permanently broken for stealth.
/// AUTH_REQUIRED means every recipient must be pre-authorized.
/// AUTH_IMMUTABLE means the issuer cannot change flags or grant new authorizations
/// in a way that would allow arbitrary stealth addresses to receive.
/// In practice: no stealth address can ever receive this token without the issuer
/// explicitly authorizing it first, and the issuer cannot be removed.
/// This is a permanent UX blocker for stealth flows.
use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, String};

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Balance(Address),
    Authorized(Address),
    Admin,
}

#[contract]
pub struct ImmutableAuthRequiredToken;

#[contractimpl]
impl ImmutableAuthRequiredToken {
    pub fn init(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    /// Admin can authorize addresses, but cannot remove itself (immutable).
    pub fn set_authorized(env: Env, admin: Address, id: Address, authorize: bool) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        assert_eq!(admin, stored_admin);
        env.storage()
            .temporary()
            .set(&DataKey::Authorized(id), &authorize);
    }

    pub fn allowance(_env: Env, _from: Address, _spender: Address) -> i128 {
        0
    }
    pub fn approve(_env: Env, _from: Address, _spender: Address, _amount: i128, _exp: u32) {}
    pub fn balance(env: Env, id: Address) -> i128 {
        env.storage()
            .temporary()
            .get(&DataKey::Balance(id))
            .unwrap_or(0)
    }
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        if !env
            .storage()
            .temporary()
            .get(&DataKey::Authorized(to.clone()))
            .unwrap_or(false)
        {
            panic!("BalanceDeauthorizedError");
        }
        let from_bal: i128 = env
            .storage()
            .temporary()
            .get(&DataKey::Balance(from.clone()))
            .unwrap_or(0);
        env.storage()
            .temporary()
            .set(&DataKey::Balance(from), &(from_bal - amount));
        let to_bal: i128 = env
            .storage()
            .temporary()
            .get(&DataKey::Balance(to.clone()))
            .unwrap_or(0);
        env.storage()
            .temporary()
            .set(&DataKey::Balance(to), &(to_bal + amount));
    }
    pub fn transfer_from(env: Env, _spender: Address, from: Address, to: Address, amount: i128) {
        Self::transfer(env, from, to, amount);
    }
    pub fn burn(env: Env, from: Address, amount: i128) {
        from.require_auth();
        let bal: i128 = env
            .storage()
            .temporary()
            .get(&DataKey::Balance(from.clone()))
            .unwrap_or(0);
        env.storage()
            .temporary()
            .set(&DataKey::Balance(from), &(bal - amount));
    }
    pub fn burn_from(_env: Env, _spender: Address, _from: Address, _amount: i128) {}
    pub fn decimals(_env: Env) -> u32 {
        7
    }
    pub fn name(env: Env) -> String {
        String::from_str(&env, "ImmutableAuthRequired")
    }
    pub fn symbol(env: Env) -> String {
        String::from_str(&env, "IMMAR")
    }
}

impl ImmutableAuthRequiredToken {
    pub fn mint(env: &Env, to: &Address, amount: i128) {
        let bal: i128 = env
            .storage()
            .temporary()
            .get(&DataKey::Balance(to.clone()))
            .unwrap_or(0);
        env.storage()
            .temporary()
            .set(&DataKey::Balance(to.clone()), &(bal + amount));
    }

    pub fn is_authorized(env: &Env, id: &Address) -> bool {
        env.storage()
            .temporary()
            .get(&DataKey::Authorized(id.clone()))
            .unwrap_or(false)
    }
}
