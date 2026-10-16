/// Mock: Custom Soroban-native token (non-SAC) that credits more than it is
/// asked to. `transfer` debits `amount` from the sender but credits the
/// recipient `amount` plus a 1% bonus minted on the spot, as a rebasing or
/// reward-on-transfer token might.
/// Implements only the two calls stealth-sender makes (`balance`, `transfer`);
/// it exists to show that the sender requires the stealth address to receive
/// exactly the transferred amount, not merely at least that amount.
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Balance(Address),
}

/// Bonus in basis points (100 = 1%).
const BONUS_BPS: i128 = 100;

#[contract]
pub struct BonusToken;

#[contractimpl]
impl BonusToken {
    pub fn balance(env: Env, id: Address) -> i128 {
        env.storage()
            .temporary()
            .get(&DataKey::Balance(id))
            .unwrap_or(0)
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        let bonus = amount * BONUS_BPS / 10_000;

        let from_bal = Self::balance(env.clone(), from.clone());
        env.storage()
            .temporary()
            .set(&DataKey::Balance(from), &(from_bal - amount));

        let to_bal = Self::balance(env.clone(), to.clone());
        env.storage()
            .temporary()
            .set(&DataKey::Balance(to), &(to_bal + amount + bonus));
    }
}

impl BonusToken {
    pub fn mint(env: &Env, to: &Address, amount: i128) {
        let bal = Self::balance(env.clone(), to.clone());
        env.storage()
            .temporary()
            .set(&DataKey::Balance(to.clone()), &(bal + amount));
    }
}
