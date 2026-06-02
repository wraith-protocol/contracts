/// Mock: Custom Soroban-native token (non-SAC) with a transfer fee.
/// Implements TokenInterface directly. Deducts a 1% fee from the transferred
/// amount, crediting the fee to a treasury address.
/// This means the stealth address receives less than `amount`, which can break
/// assumptions in the announcement (the announced amount ≠ received amount).
use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, String};

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Balance(Address),
    Treasury,
}

/// Fee in basis points (100 = 1%).
const FEE_BPS: i128 = 100;

#[contract]
pub struct FeeToken;

#[contractimpl]
impl FeeToken {
    pub fn init(env: Env, treasury: Address) {
        env.storage().instance().set(&DataKey::Treasury, &treasury);
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
impl token::Interface for FeeToken {
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
        let fee = amount * FEE_BPS / 10_000;
        let net = amount - fee;

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
            .set(&DataKey::Balance(to), &(to_bal + net));

        let treasury: Address = env.storage().instance().get(&DataKey::Treasury).unwrap();
        let treasury_bal: i128 = env
            .storage()
            .temporary()
            .get(&DataKey::Balance(treasury.clone()))
            .unwrap_or(0);
        env.storage()
            .temporary()
            .set(&DataKey::Balance(treasury), &(treasury_bal + fee));
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
        String::from_str(&env, "FeeToken")
    }
    fn symbol(env: Env) -> String {
        String::from_str(&env, "FEE")
    }
}
