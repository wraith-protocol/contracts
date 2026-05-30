/// Mock token contracts for adversarial testing.
/// These mocks simulate SAC behavior and various failure modes.

#![cfg(test)]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Bytes, BytesN,
    Env, IntoVal, Symbol, Val, Vec,
};

/// A standard mock token that behaves like a normal SAC.
#[contract]
pub struct MockTokenContract;

#[contracttype]
#[derive(Clone)]
pub enum MockTokenDataKey {
    Balance(Address),
    Allowance(Address, Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MockTokenError {
    InsufficientBalance = 1,
    InsufficientAllowance = 2,
}

#[contractimpl]
impl MockTokenContract {
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();

        let from_balance: i128 = env
            .storage()
            .instance()
            .get(&MockTokenDataKey::Balance(from.clone()))
            .unwrap_or(0);

        if from_balance < amount {
            panic!("insufficient balance");
        }

        let to_balance: i128 = env
            .storage()
            .instance()
            .get(&MockTokenDataKey::Balance(to.clone()))
            .unwrap_or(0);

        env.storage()
            .instance()
            .set(&MockTokenDataKey::Balance(from.clone()), &(from_balance - amount));
        env.storage()
            .instance()
            .set(&MockTokenDataKey::Balance(to.clone()), &(to_balance + amount));
    }

    pub fn mint(env: Env, to: Address, amount: i128) {
        let balance: i128 = env
            .storage()
            .instance()
            .get(&MockTokenDataKey::Balance(to.clone()))
            .unwrap_or(0);

        env.storage()
            .instance()
            .set(&MockTokenDataKey::Balance(to), &(balance + amount));
    }

    pub fn balance_of(env: Env, account: Address) -> i128 {
        env.storage()
            .instance()
            .get(&MockTokenDataKey::Balance(account))
            .unwrap_or(0)
    }
}

/// A malicious token that attempts to reenter the sender contract.
#[contract]
pub struct MaliciousTokenContract;

#[contracttype]
#[derive(Clone)]
pub enum MaliciousTokenDataKey {
    Balance(Address),
    ReentryAttempted,
    SenderContractAddress,
}

#[contractimpl]
impl MaliciousTokenContract {
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();

        let from_balance: i128 = env
            .storage()
            .instance()
            .get(&MaliciousTokenDataKey::Balance(from.clone()))
            .unwrap_or(0);

        if from_balance < amount {
            panic!("insufficient balance");
        }

        // Attempt to reenter the sender contract.
        // In a real scenario, this would try to call back into the sender.
        // In Soroban, this is not possible due to the execution model,
        // but we record the attempt for testing purposes.
        if let Ok(sender_addr) = env
            .storage()
            .instance()
            .get::<_, Address>(&MaliciousTokenDataKey::SenderContractAddress)
        {
            // Record that we attempted reentry.
            env.storage()
                .instance()
                .set(&MaliciousTokenDataKey::ReentryAttempted, &true);

            // In Soroban, we cannot actually reenter, so this is just a marker.
            // The test will verify that the marker is set but the transaction still succeeds.
        }

        let to_balance: i128 = env
            .storage()
            .instance()
            .get(&MaliciousTokenDataKey::Balance(to.clone()))
            .unwrap_or(0);

        env.storage()
            .instance()
            .set(&MaliciousTokenDataKey::Balance(from.clone()), &(from_balance - amount));
        env.storage()
            .instance()
            .set(&MaliciousTokenDataKey::Balance(to.clone()), &(to_balance + amount));
    }

    pub fn mint(env: Env, to: Address, amount: i128) {
        let balance: i128 = env
            .storage()
            .instance()
            .get(&MaliciousTokenDataKey::Balance(to.clone()))
            .unwrap_or(0);

        env.storage()
            .instance()
            .set(&MaliciousTokenDataKey::Balance(to), &(balance + amount));
    }

    pub fn set_sender_address(env: Env, sender: Address) {
        env.storage()
            .instance()
            .set(&MaliciousTokenDataKey::SenderContractAddress, &sender);
    }

    pub fn reentry_attempted(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&MaliciousTokenDataKey::ReentryAttempted)
            .unwrap_or(false)
    }

    pub fn balance_of(env: Env, account: Address) -> i128 {
        env.storage()
            .instance()
            .get(&MaliciousTokenDataKey::Balance(account))
            .unwrap_or(0)
    }
}

/// A token that fails on transfer (simulates a frozen account or other error).
#[contract]
pub struct FailingTokenContract;

#[contracttype]
#[derive(Clone)]
pub enum FailingTokenDataKey {
    Balance(Address),
    FailOnTransfer,
}

#[contractimpl]
impl FailingTokenContract {
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();

        // Always fail.
        panic!("transfer failed");
    }

    pub fn mint(env: Env, to: Address, amount: i128) {
        let balance: i128 = env
            .storage()
            .instance()
            .get(&FailingTokenDataKey::Balance(to.clone()))
            .unwrap_or(0);

        env.storage()
            .instance()
            .set(&FailingTokenDataKey::Balance(to), &(balance + amount));
    }

    pub fn balance_of(env: Env, account: Address) -> i128 {
        env.storage()
            .instance()
            .get(&FailingTokenDataKey::Balance(account))
            .unwrap_or(0)
    }
}

/// A token that fails on the Nth transfer (for batch testing).
#[contract]
pub struct FailOnNthTokenContract;

#[contracttype]
#[derive(Clone)]
pub enum FailOnNthTokenDataKey {
    Balance(Address),
    TransferCount,
    FailOnTransferNumber,
}

#[contractimpl]
impl FailOnNthTokenContract {
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();

        let count: u32 = env
            .storage()
            .instance()
            .get(&FailOnNthTokenDataKey::TransferCount)
            .unwrap_or(0);

        let fail_on: u32 = env
            .storage()
            .instance()
            .get(&FailOnNthTokenDataKey::FailOnTransferNumber)
            .unwrap_or(u32::MAX);

        if count == fail_on {
            panic!("transfer failed on nth attempt");
        }

        let from_balance: i128 = env
            .storage()
            .instance()
            .get(&FailOnNthTokenDataKey::Balance(from.clone()))
            .unwrap_or(0);

        if from_balance < amount {
            panic!("insufficient balance");
        }

        let to_balance: i128 = env
            .storage()
            .instance()
            .get(&FailOnNthTokenDataKey::Balance(to.clone()))
            .unwrap_or(0);

        env.storage()
            .instance()
            .set(&FailOnNthTokenDataKey::Balance(from.clone()), &(from_balance - amount));
        env.storage()
            .instance()
            .set(&FailOnNthTokenDataKey::Balance(to.clone()), &(to_balance + amount));

        env.storage()
            .instance()
            .set(&FailOnNthTokenDataKey::TransferCount, &(count + 1));
    }

    pub fn mint(env: Env, to: Address, amount: i128) {
        let balance: i128 = env
            .storage()
            .instance()
            .get(&FailOnNthTokenDataKey::Balance(to.clone()))
            .unwrap_or(0);

        env.storage()
            .instance()
            .set(&FailOnNthTokenDataKey::Balance(to), &(balance + amount));
    }

    pub fn set_fail_on_transfer_number(env: Env, n: u32) {
        env.storage()
            .instance()
            .set(&FailOnNthTokenDataKey::FailOnTransferNumber, &n);
    }

    pub fn balance_of(env: Env, account: Address) -> i128 {
        env.storage()
            .instance()
            .get(&FailOnNthTokenDataKey::Balance(account))
            .unwrap_or(0)
    }
}
