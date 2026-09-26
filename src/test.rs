#[test]
fn test_pause_blocks_register() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let client = ContractClient::new(&env, &env.register_contract(None, Contract {}));
    
    client.initialize(&admin);
    
    // Should work before pause
    client.register(/* args */);
    
    // Pause
    client.pause(&admin);
    assert!(client.is_paused());
    
    // Should panic when paused
    let result = std::panic::catch_unwind(|| client.register(/* args */));
    assert!(result.is_err());
}

#[test]
fn test_unpause_restores_access() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let client = ContractClient::new(&env, &env.register_contract(None, Contract {}));
    
    client.initialize(&admin);
    client.pause(&admin);
    client.unpause(&admin);
    
    assert!(!client.is_paused());
    // Should work again
    client.register(/* args */);
}

#[test]
#[should_panic(expected = "unauthorized")]
fn test_non_admin_cannot_pause() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    let client = ContractClient::new(&env, &env.register_contract(None, Contract {}));
    
    client.initialize(&admin);
    client.pause(&non_admin); // should panic
}