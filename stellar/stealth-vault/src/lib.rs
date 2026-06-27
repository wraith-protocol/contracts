#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, token, Address, Bytes, BytesN, Env,
};

const GRACE_PERIOD: u32 = 1000;
const TTL_THRESHOLD: u32 = 17280;
const TTL_EXTEND_TO: u32 = 518400;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Deposit(BytesN<32>),
    Announcer,
}

#[contracttype]
#[derive(Clone)]
pub struct DepositEntry {
    pub sender: Address,
    pub recipient: Address,
    pub amount: i128,
    pub asset: Address,
    pub unlock_ledger: u32,
    pub refund_after: u32,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum VaultError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InvalidWindow = 3,
    DepositNotFound = 4,
    NotYetUnlocked = 5,
    NotYetRefundable = 6,
    WrongRecipient = 7,
}

mod announcer_client {
    use soroban_sdk::{Address, Bytes, BytesN, Env, IntoVal};

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
}

#[contract]
pub struct StealthVaultContract;

#[contractimpl]
impl StealthVaultContract {
    pub fn init(env: Env, announcer: Address) -> Result<(), VaultError> {
        if env.storage().instance().has(&DataKey::Announcer) {
            return Err(VaultError::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Announcer, &announcer);
        env.storage().instance().extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        Ok(())
    }

    pub fn deposit(
        env: Env,
        sender: Address,
        recipient: Address,
        amount: i128,
        asset: Address,
        unlock_ledger: u32,
        refund_after: u32,
        ephemeral_pub_key: BytesN<32>,
    ) -> Result<BytesN<32>, VaultError> {
        sender.require_auth();

        if refund_after <= unlock_ledger + GRACE_PERIOD {
            return Err(VaultError::InvalidWindow);
        }

        let announcer: Address = env
            .storage()
            .instance()
            .get(&DataKey::Announcer)
            .ok_or(VaultError::NotInitialized)?;

        env.storage().instance().extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);

        // Build deposit_id from key fields
        let seq = env.ledger().sequence();
        let mut id_bytes = Bytes::new(&env);
        id_bytes.append(&Bytes::from_slice(&env, &amount.to_be_bytes()));
        id_bytes.append(&Bytes::from_slice(&env, &unlock_ledger.to_be_bytes()));
        id_bytes.append(&Bytes::from_slice(&env, &refund_after.to_be_bytes()));
        id_bytes.append(&ephemeral_pub_key.clone().into());
        id_bytes.append(&Bytes::from_slice(&env, &seq.to_be_bytes()));
        let deposit_id: BytesN<32> = env.crypto().sha256(&id_bytes).into();

        // Transfer tokens from sender to this contract
        token::Client::new(&env, &asset).transfer(
            &sender,
            &env.current_contract_address(),
            &amount,
        );

        // Store deposit
        let entry = DepositEntry {
            sender: sender.clone(),
            recipient: recipient.clone(),
            amount,
            asset: asset.clone(),
            unlock_ledger,
            refund_after,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Deposit(deposit_id.clone()), &entry);
        env.storage()
            .persistent()
            .extend_ttl(&DataKey::Deposit(deposit_id.clone()), TTL_THRESHOLD, TTL_EXTEND_TO);

        // Emit announcement so recipient finds it during normal scan
        // metadata = [view_tag] where view_tag = first byte of ephemeral_pub_key
        let view_tag = ephemeral_pub_key.get(0).unwrap_or(0);
        let metadata = Bytes::from_slice(&env, &[view_tag]);
        announcer_client::announce(&env, &announcer, 1u32, &recipient, &ephemeral_pub_key, &metadata);

        // Emit deposit event
        env.events().publish(
            (soroban_sdk::symbol_short!("deposit"), deposit_id.clone()),
            (sender, amount, asset, unlock_ledger),
        );

        Ok(deposit_id)
    }

    pub fn claim(env: Env, deposit_id: BytesN<32>, recipient: Address) -> Result<(), VaultError> {
        let entry: DepositEntry = env
            .storage()
            .persistent()
            .get(&DataKey::Deposit(deposit_id.clone()))
            .ok_or(VaultError::DepositNotFound)?;

        if env.ledger().sequence() < entry.unlock_ledger {
            return Err(VaultError::NotYetUnlocked);
        }

        recipient.require_auth();

        if recipient != entry.recipient {
            return Err(VaultError::WrongRecipient);
        }

        token::Client::new(&env, &entry.asset).transfer(
            &env.current_contract_address(),
            &recipient,
            &entry.amount,
        );

        env.storage()
            .persistent()
            .remove(&DataKey::Deposit(deposit_id.clone()));

        env.events().publish(
            (soroban_sdk::symbol_short!("claim"), deposit_id),
            (recipient, entry.amount),
        );

        Ok(())
    }

    pub fn refund(env: Env, deposit_id: BytesN<32>) -> Result<(), VaultError> {
        let entry: DepositEntry = env
            .storage()
            .persistent()
            .get(&DataKey::Deposit(deposit_id.clone()))
            .ok_or(VaultError::DepositNotFound)?;

        if env.ledger().sequence() < entry.refund_after {
            return Err(VaultError::NotYetRefundable);
        }

        entry.sender.require_auth();

        token::Client::new(&env, &entry.asset).transfer(
            &env.current_contract_address(),
            &entry.sender,
            &entry.amount,
        );

        env.storage()
            .persistent()
            .remove(&DataKey::Deposit(deposit_id.clone()));

        env.events().publish(
            (soroban_sdk::symbol_short!("refund"), deposit_id),
            (entry.sender, entry.amount),
        );

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger};
    use soroban_sdk::{Bytes, BytesN, Env};

    #[contract]
    pub struct MockAnnouncer;

    #[contractimpl]
    impl MockAnnouncer {
        pub fn announce(
            _env: Env,
            _scheme_id: u32,
            _stealth_address: Address,
            _ephemeral_pub_key: BytesN<32>,
            _metadata: Bytes,
        ) {
        }
    }

    fn setup() -> (Env, StealthVaultContractClient<'static>, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().with_mut(|li| {
            li.min_persistent_entry_ttl = 600000;
        });

        let announcer_id = env.register(MockAnnouncer, ());
        let vault_id = env.register(StealthVaultContract, ());
        let client = StealthVaultContractClient::new(&env, &vault_id);
        client.init(&announcer_id);

        let sender = Address::generate(&env);
        let recipient = Address::generate(&env);

        let token_admin = Address::generate(&env);
        let token_id = env.register_stellar_asset_contract_v2(token_admin).address();
        let token_admin_client = token::StellarAssetClient::new(&env, &token_id);
        token_admin_client.mint(&sender, &10000);

        (env, client, sender, recipient, token_id)
    }

    #[test]
    fn test_deposit_and_claim() {
        let (env, client, sender, recipient, token_id) = setup();
        let epk = BytesN::from_array(&env, &[1u8; 32]);

        let deposit_id = client.deposit(&sender, &recipient, &1000, &token_id, &100, &2000, &epk);

        env.ledger().with_mut(|li| li.sequence_number = 100);

        let token_client = token::Client::new(&env, &token_id);
        assert_eq!(token_client.balance(&recipient), 0);
        client.claim(&deposit_id, &recipient);
        assert_eq!(token_client.balance(&recipient), 1000);
    }

    #[test]
    fn test_claim_before_unlock_fails() {
        let (env, client, sender, recipient, token_id) = setup();
        let epk = BytesN::from_array(&env, &[2u8; 32]);

        let deposit_id = client.deposit(&sender, &recipient, &500, &token_id, &500, &2000, &epk);
        // sequence is 0 < 500
        let res = client.try_claim(&deposit_id, &recipient);
        assert_eq!(res, Err(Ok(VaultError::NotYetUnlocked)));
    }

    #[test]
    fn test_refund_after_window() {
        let (env, client, sender, recipient, token_id) = setup();
        let epk = BytesN::from_array(&env, &[3u8; 32]);
        let token_client = token::Client::new(&env, &token_id);

        let deposit_id = client.deposit(&sender, &recipient, &800, &token_id, &100, &2000, &epk);
        let balance_after_deposit = token_client.balance(&sender);

        env.ledger().with_mut(|li| li.sequence_number = 2000);
        client.refund(&deposit_id);
        assert_eq!(token_client.balance(&sender), balance_after_deposit + 800);
    }

    #[test]
    fn test_refund_before_window_fails() {
        let (env, client, sender, recipient, token_id) = setup();
        let epk = BytesN::from_array(&env, &[4u8; 32]);

        let deposit_id = client.deposit(&sender, &recipient, &300, &token_id, &100, &2000, &epk);
        // sequence = 0 < 2000
        let res = client.try_refund(&deposit_id);
        assert_eq!(res, Err(Ok(VaultError::NotYetRefundable)));
    }

    #[test]
    fn test_double_claim_fails() {
        let (env, client, sender, recipient, token_id) = setup();
        let epk = BytesN::from_array(&env, &[5u8; 32]);

        let deposit_id = client.deposit(&sender, &recipient, &200, &token_id, &0, &2000, &epk);
        client.claim(&deposit_id, &recipient);
        let res = client.try_claim(&deposit_id, &recipient);
        assert_eq!(res, Err(Ok(VaultError::DepositNotFound)));
    }

    #[test]
    fn test_wrong_recipient_cannot_claim() {
        let (env, client, sender, recipient, token_id) = setup();
        let epk = BytesN::from_array(&env, &[6u8; 32]);
        let wrong = Address::generate(&env);

        let deposit_id = client.deposit(&sender, &recipient, &100, &token_id, &0, &2000, &epk);
        let res = client.try_claim(&deposit_id, &wrong);
        assert_eq!(res, Err(Ok(VaultError::WrongRecipient)));
    }

    #[test]
    fn test_deposit_validates_refund_window() {
        let (env, client, sender, recipient, token_id) = setup();
        let epk = BytesN::from_array(&env, &[7u8; 32]);

        // refund_after = unlock_ledger + GRACE_PERIOD (not strictly greater)
        let res = client.try_deposit(&sender, &recipient, &100, &token_id, &100, &1100, &epk);
        assert_eq!(res, Err(Ok(VaultError::InvalidWindow)));
    }

    #[test]
    fn test_sender_cannot_refund_early() {
        let (env, client, sender, recipient, token_id) = setup();
        let epk = BytesN::from_array(&env, &[8u8; 32]);

        let deposit_id = client.deposit(&sender, &recipient, &150, &token_id, &100, &5000, &epk);
        env.ledger().with_mut(|li| li.sequence_number = 4999);
        let res = client.try_refund(&deposit_id);
        assert_eq!(res, Err(Ok(VaultError::NotYetRefundable)));
    }
}
