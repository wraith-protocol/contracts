/// Mock: AUTH_CLAWBACK_ENABLED asset.
/// Transfer succeeds. The admin can clawback (burn) tokens from any contract
/// balance, including a stealth address. The clawback flag is set at balance
/// creation time and cannot be removed.
/// This defeats unlinkability: the issuer can reverse a stealth payment.
use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, String};

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Balance(Address),
    Admin,
}

#[contract]
pub struct ClawbackToken;

#[contractimpl]
impl ClawbackToken {
    pub fn init(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    /// Admin clawback: burns `amount` from `from`. No consent required.
    pub fn clawback(env: Env, admin: Address, from: Address, amount: i128) {
        admin.require_auth();
        let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        assert_eq!(admin, stored_admin);
        let bal: i128 = env
            .storage()
            .temporary()
            .get(&DataKey::Balance(from.clone()))
            .unwrap_or(0);
        env.storage()
            .temporary()
            .set(&DataKey::Balance(from), &(bal - amount));
    }

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

#[contractimpl]
impl token::Interface for ClawbackToken {
    fn allowance(_env: Env, _from: Address, _spender: Address) -> i128 {
        0
    }
    fn approve(_env: Env, _from: Address, _spender: Address, _amount: i128, _exp: u32) {}
    fn balance(env: Env, id: Address) -> i128 {
        env.storage()
            .temporary()
            .get(&DataKey::Balance(id))
            .unwrap_or(0)
    }
    fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
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
    fn transfer_from(
        env: Env,
        _spender: Address,
        from: Address,
        to: Address,
        amount: i128,
    ) {
        Self::transfer(env, from, to, amount);
    }
    fn burn(env: Env, from: Address, amount: i128) {
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
    fn burn_from(_env: Env, _spender: Address, _from: Address, _amount: i128) {}
    fn decimals(_env: Env) -> u32 {
        7
    }
    fn name(env: Env) -> String {
        String::from_str(&env, "Clawback")
    }
    fn symbol(env: Env) -> String {
        String::from_str(&env, "CLAW")
    }
}
