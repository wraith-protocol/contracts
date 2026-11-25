use proptest::prelude::*;
use soroban_sdk::testutils::{Address as _, EnvTestConfig, Events, Ledger};
use soroban_sdk::{Address, Bytes, BytesN, Env, String};
use wraith_names::{
    DataKey, NameEntry, NamesError, WraithNamesContract, WraithNamesContractClient,
};

fn cases() -> u32 {
    std::env::var("WRAITH_PROPTEST_CASES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1024)
}

fn bytes(env: &Env, data: &[u8]) -> Bytes {
    Bytes::from_slice(env, data)
}

fn env() -> Env {
    Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    })
}

fn name(env: &Env, value: &str) -> String {
    String::from_str(env, value)
}

fn valid_name_strategy() -> impl Strategy<Value = std::string::String> {
    "[a-z0-9]{3,32}"
}

fn invalid_name_strategy() -> impl Strategy<Value = std::string::String> {
    prop_oneof![
        "[a-z0-9]{0,2}",
        "[a-z0-9]{33,40}",
        "[A-Z][a-z0-9]{2,10}",
        "[a-z0-9]{1,8}[-_][a-z0-9]{1,8}",
    ]
}

const ACTORS: usize = 3;
const CHILDREN: usize = 3;

#[derive(Clone, Debug)]
enum Operation {
    RegisterParent(u8),
    RegisterChild(u8, u8),
    UpdateChild(u8, u8),
    ReleaseChild(u8, u8),
    ReleaseParent(u8),
    RenewChild(u8),
    ResolveChild(u8),
    TransferParent(u8),
    ExpireParent,
}

fn operation_strategy() -> impl Strategy<Value = Operation> {
    prop_oneof![
        (0u8..ACTORS as u8).prop_map(Operation::RegisterParent),
        ((0u8..ACTORS as u8), (0u8..CHILDREN as u8))
            .prop_map(|(actor, child)| Operation::RegisterChild(actor, child)),
        ((0u8..ACTORS as u8), (0u8..CHILDREN as u8))
            .prop_map(|(actor, child)| Operation::UpdateChild(actor, child)),
        ((0u8..ACTORS as u8), (0u8..CHILDREN as u8))
            .prop_map(|(actor, child)| Operation::ReleaseChild(actor, child)),
        (0u8..ACTORS as u8).prop_map(Operation::ReleaseParent),
        (0u8..CHILDREN as u8).prop_map(Operation::RenewChild),
        (0u8..CHILDREN as u8).prop_map(Operation::ResolveChild),
        (0u8..ACTORS as u8).prop_map(Operation::TransferParent),
        Just(Operation::ExpireParent),
    ]
}

fn property_name(env: &Env, child: usize) -> String {
    String::from_str(env, &format!("child{child}.alice"))
}

fn property_meta(env: &Env, seed: u8) -> Bytes {
    Bytes::from_slice(env, &[seed; 64])
}

fn name_key(env: &Env, value: &str) -> DataKey {
    let bytes = Bytes::from_slice(env, value.as_bytes());
    let hash = BytesN::from_array(env, &env.crypto().sha256(&bytes).to_array());
    DataKey::Name(hash)
}

fn force_parent_transfer(env: &Env, contract_id: &Address, new_owner: &Address) {
    env.as_contract(contract_id, || {
        let key = name_key(env, "alice");
        let mut entry: NameEntry = env
            .storage()
            .persistent()
            .get(&key)
            .expect("property transfer requires a live parent");
        entry.owner = new_owner.clone();
        env.storage().persistent().set(&key, &entry);
    });
}

fn expire_parent(env: &Env, contract_id: &Address) {
    env.as_contract(contract_id, || {
        // Keep the instance and child records live while archiving only the
        // parent. This models the network state seen by a child operation.
        env.storage().instance().extend_ttl(2_000_000, 2_000_000);
        for child in 0..CHILDREN {
            let key = name_key(env, &format!("child{child}.alice"));
            if env.storage().persistent().has(&key) {
                env.storage()
                    .persistent()
                    .extend_ttl(&key, 2_000_000, 2_000_000);
            }
        }
    });
    // The contract extends names to roughly 30 days on each access. Advance
    // beyond that horizon while keeping the child and instance alive.
    env.ledger()
        .with_mut(|ledger| ledger.sequence_number += 518_401);
}

struct Model {
    parent_owner: Option<usize>,
    parent_expired: bool,
    child_live: [bool; CHILDREN],
    child_meta: [[u8; 64]; CHILDREN],
}

impl Default for Model {
    fn default() -> Self {
        Self {
            parent_owner: None,
            parent_expired: false,
            child_live: [false; CHILDREN],
            child_meta: [[0u8; 64]; CHILDREN],
        }
    }
}

impl Model {
    fn parent_live(&self) -> bool {
        self.parent_owner.is_some()
    }

    fn authorized(&self, actor: usize) -> bool {
        self.parent_owner == Some(actor)
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: cases(), .. ProptestConfig::default() })]
    #[test]
    fn register_then_resolve_round_trips_valid_names(name_value in valid_name_strategy(), meta in any::<[u8; 64]>()) {
        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let name = name(&env, &name_value);
        let meta = bytes(&env, &meta);

        client.register(&owner, &name, &meta);

        prop_assert_eq!(client.resolve(&name), meta);
    }

    #[test]
    fn name_of_round_trips_the_registered_name(name_value in valid_name_strategy(), meta in any::<[u8; 64]>()) {
        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let name = name(&env, &name_value);
        let meta = bytes(&env, &meta);

        client.register(&owner, &name, &meta);

        prop_assert_eq!(client.name_of(&meta), name);
    }

    #[test]
    fn update_replaces_meta_address_for_owner(name_value in valid_name_strategy(), first in any::<[u8; 64]>(), second in any::<[u8; 64]>()) {
        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let name = name(&env, &name_value);
        let first = bytes(&env, &first);
        let second = bytes(&env, &second);

        client.register(&owner, &name, &first);
        client.update(&owner, &name, &second);

        prop_assert_eq!(client.resolve(&name), second);
        prop_assert_eq!(client.try_name_of(&first), Err(Ok(NamesError::NameNotFound)));
    }

    #[test]
    fn invalid_names_are_rejected(name_value in invalid_name_strategy(), meta in any::<[u8; 64]>()) {
        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let name = name(&env, &name_value);
        let meta = bytes(&env, &meta);

        let result = client.try_register(&owner, &name, &meta);

        prop_assert!(result.is_err());
    }

    #[test]
    fn non_64_byte_meta_addresses_are_rejected(name_value in valid_name_strategy(), data in prop::collection::vec(any::<u8>(), 0..96)) {
        prop_assume!(data.len() != 64);

        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let name = name(&env, &name_value);

        let result = client.try_register(&owner, &name, &bytes(&env, &data));

        prop_assert_eq!(result, Err(Ok(NamesError::InvalidMetaAddress)));
    }

    #[test]
    fn release_removes_forward_and_reverse_lookup(name_value in valid_name_strategy(), meta in any::<[u8; 64]>()) {
        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let name = name(&env, &name_value);
        let meta = bytes(&env, &meta);

        client.register(&owner, &name, &meta);
        client.release(&owner, &name);

        prop_assert_eq!(client.try_resolve(&name), Err(Ok(NamesError::NameNotFound)));
        prop_assert_eq!(client.try_name_of(&meta), Err(Ok(NamesError::NameNotFound)));
    }

    #[test]
    fn successful_register_emits_domain_and_metric_events(name_value in valid_name_strategy(), meta in any::<[u8; 64]>()) {
        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);
        let owner = Address::generate(&env);
        let name = name(&env, &name_value);
        let meta = bytes(&env, &meta);

        client.register(&owner, &name, &meta);

        // One `register` domain event followed by one wraith-metrics event.
        prop_assert_eq!(env.events().all().len(), 2);
    }

    #[test]
    fn subdomain_state_machine_preserves_ownership_and_expiry_invariants(
        operations in prop::collection::vec(operation_strategy(), 1..48),
    ) {
        let env = env();
        env.mock_all_auths();
        let contract_id = env.register(WraithNamesContract, ());
        let client = WraithNamesContractClient::new(&env, &contract_id);
        let actors: [Address; ACTORS] = std::array::from_fn(|_| Address::generate(&env));
        let parent = name(&env, "alice");
        let mut model = Model::default();

        for (step, operation) in operations.into_iter().enumerate() {
            // An archived parent rejects state-changing operations before the
            // contract body is entered. Keep the generated trace useful by
            // waiting for a parent re-registration, while still checking
            // resolution during the archived state below.
            if model.parent_expired
                && !matches!(&operation, Operation::RegisterParent(_) | Operation::ResolveChild(_))
            {
                continue;
            }
            match operation {
                Operation::RegisterParent(actor) => {
                    let caller = &actors[actor as usize];
                    let result = client.try_register(caller, &parent, &property_meta(&env, 1));
                    if model.parent_live() {
                        prop_assert_eq!(result, Err(Ok(NamesError::NameTaken)));
                    } else {
                        prop_assert_eq!(result, Ok(Ok(())));
                        model.parent_owner = Some(actor as usize);
                        model.parent_expired = false;
                    }
                }
                Operation::RegisterChild(actor, child) => {
                    let actor = actor as usize;
                    let child = child as usize;
                    let child_name = property_name(&env, child);
                    let result = client.try_register(
                        &actors[actor],
                        &child_name,
                        &property_meta(&env, child as u8 + 10),
                    );

                    if !model.parent_live() {
                        prop_assert_eq!(result, Err(Ok(NamesError::ParentNotFound)));
                    } else if !model.authorized(actor) {
                        prop_assert_eq!(result, Err(Ok(NamesError::NotOwner)));
                    } else if model.child_live[child] {
                        prop_assert_eq!(result, Err(Ok(NamesError::NameTaken)));
                    } else {
                        prop_assert_eq!(result, Ok(Ok(())));
                        model.child_live[child] = true;
                        model.child_meta[child] = [child as u8 + 10; 64];
                    }
                }
                Operation::UpdateChild(actor, child) => {
                    let actor = actor as usize;
                    let child = child as usize;
                    let child_name = property_name(&env, child);
                    let updated = child as u8 + 100;
                    let result = client.try_update(
                        &actors[actor],
                        &child_name,
                        &property_meta(&env, updated),
                    );

                    if !model.parent_live() || !model.child_live[child] {
                        prop_assert_eq!(result, Err(Ok(NamesError::NameNotFound)));
                    } else if !model.authorized(actor) {
                        prop_assert_eq!(result, Err(Ok(NamesError::NotOwner)));
                    } else {
                        prop_assert_eq!(result, Ok(Ok(())));
                        model.child_meta[child] = [updated; 64];
                    }
                }
                Operation::ReleaseChild(actor, child) => {
                    let actor = actor as usize;
                    let child = child as usize;
                    let child_name = property_name(&env, child);
                    let result = client.try_release(&actors[actor], &child_name);

                    if !model.parent_live() || !model.child_live[child] {
                        prop_assert_eq!(result, Err(Ok(NamesError::NameNotFound)));
                    } else if !model.authorized(actor) {
                        prop_assert_eq!(result, Err(Ok(NamesError::NotOwner)));
                    } else {
                        prop_assert_eq!(result, Ok(Ok(())));
                        model.child_live[child] = false;
                    }
                }
                Operation::ReleaseParent(actor) => {
                    let result = client.try_release(&actors[actor as usize], &parent);
                    if !model.parent_live() {
                        prop_assert_eq!(result, Err(Ok(NamesError::NameNotFound)));
                    } else if !model.authorized(actor as usize) {
                        prop_assert_eq!(result, Err(Ok(NamesError::NotOwner)));
                    } else {
                        prop_assert_eq!(result, Ok(Ok(())));
                        model.parent_owner = None;
                        model.parent_expired = false;
                        model.child_live = [false; CHILDREN];
                    }
                }
                Operation::RenewChild(child) => {
                    let child = child as usize;
                    let child_name = property_name(&env, child);
                    let extend_to = env.ledger().sequence() + 1_000;
                    let result = client.try_extend_name_ttl(&child_name, &extend_to);
                    if model.parent_live() && model.child_live[child] {
                        prop_assert_eq!(result, Ok(Ok(())));
                    } else {
                        prop_assert_eq!(result, Err(Ok(NamesError::NameNotFound)));
                    }
                }
                Operation::ResolveChild(child) => {
                    let child = child as usize;
                    let child_name = property_name(&env, child);
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        client.try_resolve(&child_name)
                    }));
                    if model.parent_live() && model.child_live[child] {
                        let result = result.expect("live child resolution must not panic");
                        prop_assert_eq!(result, Ok(Ok(property_meta(&env, model.child_meta[child][0]))));
                    } else {
                        if let Ok(result) = result {
                            prop_assert!(matches!(result, Err(Ok(NamesError::NameNotFound))));
                        }
                    }
                }
                Operation::TransferParent(actor) => {
                    if model.parent_live() {
                        force_parent_transfer(&env, &contract_id, &actors[actor as usize]);
                        model.parent_owner = Some(actor as usize);
                    }
                }
                Operation::ExpireParent => {
                    if model.parent_live() {
                        expire_parent(&env, &contract_id);
                        model.parent_owner = None;
                        model.parent_expired = true;
                        model.child_live = [false; CHILDREN];
                    }
                }
            }

            // A child is resolvable exactly when both its own record and its
            // parent registration are live. This catches operation-ordering
            // bugs even when the generated command itself was rejected.
            for child in 0..CHILDREN {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    client.try_resolve(&property_name(&env, child))
                }));
                if model.parent_live() && model.child_live[child] {
                    let result = result.expect("live child resolution must not panic");
                    prop_assert_eq!(result, Ok(Ok(property_meta(&env, model.child_meta[child][0]))));
                } else {
                    if let Ok(result) = result {
                        prop_assert!(matches!(result, Err(Ok(NamesError::NameNotFound))));
                    }
                }
            }

            prop_assert!(step < 48);
        }
    }
}

#[test]
fn default_property_case_count_is_at_least_1024() {
    assert!(cases() >= 1024);
}
