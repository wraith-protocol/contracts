#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, token, Address, Bytes, BytesN, Env, Vec,
};

/// Storage keys.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// The address of the deployed StealthAnnouncer contract.
    Announcer,
}

/// Errors that the sender contract can produce.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SenderError {
    /// The contract has already been initialised.
    AlreadyInitialized = 1,
    /// The contract has not been initialised yet.
    NotInitialized = 2,
    /// The batch input vectors have mismatched lengths.
    LengthMismatch = 3,
}

/// Lightweight client wrapper that invokes the StealthAnnouncer contract via
/// `env.invoke_contract`. This avoids needing a compiled WASM at build time
/// (unlike `contractimport!`) and keeps the build self-contained.
mod announcer_client {
    use soroban_sdk::{Address, Bytes, BytesN, Env};

    pub fn announce(
        env: &Env,
        announcer: &Address,
        scheme_id: u32,
        stealth_address: &Address,
        ephemeral_pub_key: &BytesN<32>,
        metadata: &Bytes,
    ) {
        let _: () = env.invoke_contract(
            announcer,
            &soroban_sdk::symbol_short!("announce"),
            soroban_sdk::vec![
                env,
                scheme_id.into_val(env),
                stealth_address.into_val(env),
                ephemeral_pub_key.into_val(env),
                metadata.into_val(env),
            ],
        );
    }

    use soroban_sdk::IntoVal;
}

#[contract]
pub struct StealthSenderContract;

#[contractimpl]
impl StealthSenderContract {
    /// Initialise the contract by storing the announcer address.
    ///
    /// Must be called exactly once before any `send` or `batch_send`.
    pub fn init(env: Env, announcer: Address) -> Result<(), SenderError> {
        if env.storage().instance().has(&DataKey::Announcer) {
            return Err(SenderError::AlreadyInitialized);
        }
        env.storage()
            .instance()
            .set(&DataKey::Announcer, &announcer);
        Ok(())
    }

    /// Transfer tokens to a stealth address and emit an announcement.
    ///
    /// # Arguments
    /// * `sender`            - The address sending funds (must authorise).
    /// * `token`             - SAC token contract address (works for native XLM too).
    /// * `amount`            - Amount of tokens to transfer.
    /// * `scheme_id`         - Stealth address scheme identifier.
    /// * `stealth_address`   - The derived one-time stealth address.
    /// * `ephemeral_pub_key` - Ephemeral public key for the recipient to scan.
    /// * `metadata`          - Extra data (e.g. view tag).
    pub fn send(
        env: Env,
        sender: Address,
        token: Address,
        amount: i128,
        scheme_id: u32,
        stealth_address: Address,
        ephemeral_pub_key: BytesN<32>,
        metadata: Bytes,
    ) -> Result<(), SenderError> {
        sender.require_auth();

        let announcer: Address = env
            .storage()
            .instance()
            .get(&DataKey::Announcer)
            .ok_or(SenderError::NotInitialized)?;

        // Transfer tokens from sender to the stealth address.
        let token_client = token::Client::new(&env, &token);
        token_client.transfer(&sender, &stealth_address, &amount);

        // Emit the announcement via the announcer contract.
        announcer_client::announce(
            &env,
            &announcer,
            scheme_id,
            &stealth_address,
            &ephemeral_pub_key,
            &metadata,
        );

        Ok(())
    }

    /// Batch version of `send` — transfers tokens to multiple stealth addresses
    /// and emits an announcement for each.
    ///
    /// All input vectors must have the same length.
    pub fn batch_send(
        env: Env,
        sender: Address,
        token: Address,
        scheme_id: u32,
        stealth_addresses: Vec<Address>,
        ephemeral_pub_keys: Vec<BytesN<32>>,
        metadatas: Vec<Bytes>,
        amounts: Vec<i128>,
    ) -> Result<(), SenderError> {
        sender.require_auth();

        let len = stealth_addresses.len();
        if ephemeral_pub_keys.len() != len || metadatas.len() != len || amounts.len() != len {
            return Err(SenderError::LengthMismatch);
        }

        let announcer: Address = env
            .storage()
            .instance()
            .get(&DataKey::Announcer)
            .ok_or(SenderError::NotInitialized)?;

        let token_client = token::Client::new(&env, &token);

        for i in 0..len {
            let stealth_address = stealth_addresses.get(i).unwrap();
            let ephemeral_pub_key = ephemeral_pub_keys.get(i).unwrap();
            let metadata = metadatas.get(i).unwrap();
            let amount = amounts.get(i).unwrap();

            token_client.transfer(&sender, &stealth_address, &amount);

            announcer_client::announce(
                &env,
                &announcer,
                scheme_id,
                &stealth_address,
                &ephemeral_pub_key,
                &metadata,
            );
        }

        Ok(())
    }
}
