//! Futurenet verification for a sponsored announcement transaction.
//!
//! Submit a fee-bumped `sponsored_announce` transaction with the Stellar CLI or
//! client SDK, then run this test with `FUTURENET_TX_HASH` and
//! `FUTURENET_SPONSOR_ADDRESS` set. The test verifies the network recorded the
//! sponsor as the transaction fee source.

use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Transaction {
    successful: bool,
    fee_account: String,
}

#[tokio::test]
#[ignore = "requires a submitted futurenet fee-bumped transaction"]
async fn sponsored_announcement_fee_is_charged_to_sponsor() {
    let transaction_hash = std::env::var("FUTURENET_TX_HASH")
        .expect("FUTURENET_TX_HASH must identify the submitted transaction");
    let sponsor = std::env::var("FUTURENET_SPONSOR_ADDRESS")
        .expect("FUTURENET_SPONSOR_ADDRESS must identify the fee-bump source");
    let horizon = std::env::var("FUTURENET_HORIZON_URL")
        .unwrap_or_else(|_| "https://horizon-futurenet.stellar.org".to_owned());

    let transaction: Transaction = Client::new()
        .get(format!("{horizon}/transactions/{transaction_hash}"))
        .send()
        .await
        .expect("Horizon request failed")
        .error_for_status()
        .expect("Horizon rejected the transaction lookup")
        .json()
        .await
        .expect("Horizon returned invalid transaction JSON");

    assert!(transaction.successful, "sponsored transaction failed");
    assert_eq!(transaction.fee_account, sponsor);
}
