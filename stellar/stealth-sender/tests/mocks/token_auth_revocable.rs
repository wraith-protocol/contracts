/// Mock: AUTH_REVOCABLE asset.
/// Transfer succeeds, but the admin can revoke authorization from any address
/// (including a stealth address) after receipt, freezing the balance.
/// This defeats unlinkability: the issuer can identify and freeze stealth recipients.
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String};

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Balance(Address),
    Authorized(Address),
    Admin,
}

#[contract]
pub struct AuthRevocableToken;

#[contractimpl]
impl AuthRevocableToken {
    pub fn init(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    /// Admin can revoke (or restore) authorization from any address.
    pub fn set_authorized(env: Env, admin: Address, id: Address, authorize: bool) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        assert_eq!(admin, stored_admin);
        env.storage()
            .temporary()
            .set(&DataKey::Authorized(id), &authorize);
    }

    pub fn is_authorized(env: Env, id: Address) -> bool {
        // Default: authorized (no AUTH_REQUIRED, just revocable).
        env.storage()
            .temporary()
            .get(&DataKey::Authorized(id))
            .unwrap_or(true)
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
        // Sender must be authorized to spend.
        if !env
            .storage()
            .temporary()
            .get(&DataKey::Authorized(from.clone()))
            .unwrap_or(true)
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
        String::from_str(&env, "AuthRevocable")
    }
    pub fn symbol(env: Env) -> String {
        String::from_str(&env, "AREV")
    }
}

impl AuthRevocableToken {
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
}
