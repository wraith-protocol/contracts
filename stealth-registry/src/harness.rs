use reqwest::Client;
use std::env;

pub const FUTURENET_RPC: &str = "https://rpc-futurenet.stellar.org";
pub const FUTURENET_FRIENDBOT: &str = "https://friendbot-futurenet.stellar.org";
pub const FUTURENET_PASSPHRASE: &str = "Test SDF Future Network ; October 2022";

pub struct TestAccount {
    pub keypair_secret: String,
    pub public_key: String,
}

/// Fund a new test account via friendbot
pub async fn fund_account(public_key: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let url = format!("{}?addr={}", FUTURENET_FRIENDBOT, public_key);
    let resp = client.get(&url).send().await?;
    if !resp.status().is_success() {
        return Err(format!("Friendbot failed: {}", resp.status()).into());
    }
    Ok(())
}