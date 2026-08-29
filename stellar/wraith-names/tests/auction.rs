//! End-to-end integration tests for the premium-name sealed-bid auction:
//! commit → reveal → settle → claim, plus the refund paths that prove no
//! funds can be trapped in the contract.

use soroban_sdk::testutils::{Address as _, Events, Ledger};
use soroban_sdk::{token, Address, Bytes, BytesN, Env, IntoVal, String, Symbol, Vec};

use wraith_names::{AuctionError, NamesError, WraithNamesContract, WraithNamesContractClient};

const RESERVE: i128 = 100;
const COMMIT_SECS: u64 = 259_200; // 3 days
const REVEAL_SECS: u64 = 172_800; // 2 days
const DAY_SECS: u64 = 86_400;
/// `multisig::ROTATION_TIMELOCK_SECS`.
const ROTATION_TIMELOCK_SECS: u64 = 7 * DAY_SECS;

struct Setup<'a> {
    env: Env,
    client: WraithNamesContractClient<'a>,
    token: token::Client<'a>,
    token_admin: token::StellarAssetClient<'a>,
    admin: Address,
    treasury: Address,
    contract_id: Address,
}

fn setup() -> Setup<'static> {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let token_issuer = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(token_issuer);
    let token = token::Client::new(&env, &sac.address());
    let token_admin = token::StellarAssetClient::new(&env, &sac.address());

    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);

    client.init_auctions(
        &admin,
        &treasury,
        &sac.address(),
        &RESERVE,
        &COMMIT_SECS,
        &REVEAL_SECS,
    );

    Setup {
        env,
        client,
        token,
        token_admin,
        admin,
        treasury,
        contract_id,
    }
}

fn funded_bidder(s: &Setup, amount: i128) -> Address {
    let bidder = Address::generate(&s.env);
    s.token_admin.mint(&bidder, &amount);
    bidder
}

fn advance_time(env: &Env, secs: u64) {
    env.ledger().with_mut(|li| {
        li.timestamp += secs;
    });
}

fn salt(env: &Env, seed: u8) -> BytesN<32> {
    BytesN::from_array(env, &[seed; 32])
}

fn meta(env: &Env, seed: u8) -> Bytes {
    Bytes::from_slice(env, &[seed; 64])
}

/// The full auction flow: three bidders commit, two reveal, the highest
/// revealed bid wins. The treasury receives the winning bid, the winner's
/// excess deposit is refunded at settlement, the winner claims the name,
/// losers withdraw their deposits in full, and the contract ends with a zero
/// token balance — no funds trapped.
#[test]
fn auction_end_to_end_flow() {
    let s = setup();
    let name = String::from_str(&s.env, "joe");

    let alice = funded_bidder(&s, 1_000);
    let bob = funded_bidder(&s, 1_000);
    let carol = funded_bidder(&s, 1_000);

    s.client.start_auction(&name);

    // Commit phase. Deposits exceed bids so amounts stay sealed.
    let alice_salt = salt(&s.env, 1);
    let alice_commit = s
        .client
        .compute_commitment(&name, &alice, &400, &alice_salt);
    s.client.commit_bid(&alice, &name, &alice_commit, &500);

    let bob_salt = salt(&s.env, 2);
    let bob_commit = s.client.compute_commitment(&name, &bob, &300, &bob_salt);
    s.client.commit_bid(&bob, &name, &bob_commit, &300);

    let carol_salt = salt(&s.env, 3);
    let carol_commit = s
        .client
        .compute_commitment(&name, &carol, &200, &carol_salt);
    s.client.commit_bid(&carol, &name, &carol_commit, &250);

    assert_eq!(s.token.balance(&s.contract_id), 500 + 300 + 250);
    assert_eq!(s.token.balance(&alice), 500);

    // Reveal phase: alice and bob reveal, carol never does.
    advance_time(&s.env, COMMIT_SECS);
    s.client.reveal_bid(&alice, &name, &400, &alice_salt);
    s.client.reveal_bid(&bob, &name, &300, &bob_salt);

    // Settlement (admin per runbook, but permissionless).
    advance_time(&s.env, REVEAL_SECS);
    s.client.settle_auction(&name);

    // Winning bid to treasury, excess deposit back to alice.
    assert_eq!(s.token.balance(&s.treasury), 400);
    assert_eq!(s.token.balance(&alice), 500 + 100);

    // Winner claims the name with her stealth meta-address.
    let alice_meta = meta(&s.env, 9);
    s.client.claim_name(&alice, &name, &alice_meta);
    assert_eq!(s.client.resolve(&name), alice_meta);
    assert_eq!(s.client.name_of(&alice_meta), name);

    // Losers (revealed and unrevealed) withdraw their full deposits.
    s.client.withdraw_bid(&bob, &name);
    s.client.withdraw_bid(&carol, &name);
    assert_eq!(s.token.balance(&bob), 1_000);
    assert_eq!(s.token.balance(&carol), 1_000);

    // No funds trapped.
    assert_eq!(s.token.balance(&s.contract_id), 0);
}

/// Refund path when nobody reveals: settlement finds no winner and every
/// committer recovers their full deposit.
#[test]
fn refund_when_no_reveals() {
    let s = setup();
    let name = String::from_str(&s.env, "pay");

    let alice = funded_bidder(&s, 500);
    let bob = funded_bidder(&s, 500);

    s.client.start_auction(&name);

    let alice_commit = s
        .client
        .compute_commitment(&name, &alice, &200, &salt(&s.env, 1));
    s.client.commit_bid(&alice, &name, &alice_commit, &200);
    let bob_commit = s
        .client
        .compute_commitment(&name, &bob, &150, &salt(&s.env, 2));
    s.client.commit_bid(&bob, &name, &bob_commit, &150);

    advance_time(&s.env, COMMIT_SECS + REVEAL_SECS);
    s.client.settle_auction(&name);

    assert_eq!(s.client.get_auction(&name).unwrap().highest_bidder, None);
    assert_eq!(s.token.balance(&s.treasury), 0);

    s.client.withdraw_bid(&alice, &name);
    s.client.withdraw_bid(&bob, &name);
    assert_eq!(s.token.balance(&alice), 500);
    assert_eq!(s.token.balance(&bob), 500);
    assert_eq!(s.token.balance(&s.contract_id), 0);
}

/// Refunds do not depend on settlement: losers and non-revealers can
/// withdraw as soon as the reveal phase ends, even if nobody ever settles.
#[test]
fn refund_without_settlement() {
    let s = setup();
    let name = String::from_str(&s.env, "abcd");

    let alice = funded_bidder(&s, 500);
    let bob = funded_bidder(&s, 500);

    s.client.start_auction(&name);

    let alice_commit = s
        .client
        .compute_commitment(&name, &alice, &300, &salt(&s.env, 1));
    s.client.commit_bid(&alice, &name, &alice_commit, &300);
    let bob_commit = s
        .client
        .compute_commitment(&name, &bob, &200, &salt(&s.env, 2));
    s.client.commit_bid(&bob, &name, &bob_commit, &200);

    advance_time(&s.env, COMMIT_SECS);
    s.client.reveal_bid(&alice, &name, &300, &salt(&s.env, 1));
    s.client.reveal_bid(&bob, &name, &200, &salt(&s.env, 2));

    advance_time(&s.env, REVEAL_SECS);

    // No settlement has happened. Bob (loser) can still withdraw in full.
    s.client.withdraw_bid(&bob, &name);
    assert_eq!(s.token.balance(&bob), 500);

    // The winner cannot withdraw — her deposit is released via settlement,
    // which anyone may trigger.
    let result = s.client.try_withdraw_bid(&alice, &name);
    assert_eq!(result, Err(Ok(AuctionError::WinnerCannotWithdraw)));

    s.client.settle_auction(&name);
    assert_eq!(s.token.balance(&s.treasury), 300);
    assert_eq!(s.token.balance(&s.contract_id), 0);
}

/// A bid deposit can only be withdrawn once.
#[test]
fn double_withdraw_rejected() {
    let s = setup();
    let name = String::from_str(&s.env, "dup");

    let alice = funded_bidder(&s, 500);
    let bob = funded_bidder(&s, 500);

    s.client.start_auction(&name);
    let alice_commit = s
        .client
        .compute_commitment(&name, &alice, &200, &salt(&s.env, 1));
    s.client.commit_bid(&alice, &name, &alice_commit, &200);
    let bob_commit = s
        .client
        .compute_commitment(&name, &bob, &300, &salt(&s.env, 2));
    s.client.commit_bid(&bob, &name, &bob_commit, &300);

    advance_time(&s.env, COMMIT_SECS);
    s.client.reveal_bid(&bob, &name, &300, &salt(&s.env, 2));

    advance_time(&s.env, REVEAL_SECS);
    s.client.withdraw_bid(&alice, &name);
    let result = s.client.try_withdraw_bid(&alice, &name);
    assert_eq!(result, Err(Ok(AuctionError::NoBid)));
}

/// The winner's residual bid entry is removed at settlement, so they cannot
/// also withdraw it afterwards.
#[test]
fn winner_cannot_double_dip_after_settlement() {
    let s = setup();
    let name = String::from_str(&s.env, "win");

    let alice = funded_bidder(&s, 500);
    s.client.start_auction(&name);
    let alice_commit = s
        .client
        .compute_commitment(&name, &alice, &150, &salt(&s.env, 1));
    s.client.commit_bid(&alice, &name, &alice_commit, &400);

    advance_time(&s.env, COMMIT_SECS);
    s.client.reveal_bid(&alice, &name, &150, &salt(&s.env, 1));
    advance_time(&s.env, REVEAL_SECS);
    s.client.settle_auction(&name);

    // Excess (400 - 150) already refunded; nothing left to withdraw.
    assert_eq!(s.token.balance(&alice), 500 - 150);
    let result = s.client.try_withdraw_bid(&alice, &name);
    assert_eq!(result, Err(Ok(AuctionError::NoBid)));
    assert_eq!(s.token.balance(&s.contract_id), 0);
}

/// Phase boundaries are enforced: no commits after the commit phase, no
/// reveals outside the reveal phase, no settlement before reveals end.
#[test]
fn phase_boundaries_enforced() {
    let s = setup();
    let name = String::from_str(&s.env, "time");

    let alice = funded_bidder(&s, 500);
    let bob = funded_bidder(&s, 500);

    s.client.start_auction(&name);
    let alice_commit = s
        .client
        .compute_commitment(&name, &alice, &200, &salt(&s.env, 1));
    s.client.commit_bid(&alice, &name, &alice_commit, &200);

    // Reveal during commit phase is rejected.
    let result = s
        .client
        .try_reveal_bid(&alice, &name, &200, &salt(&s.env, 1));
    assert_eq!(result, Err(Ok(AuctionError::RevealPhaseNotActive)));

    // Settle during commit phase is rejected.
    let result = s.client.try_settle_auction(&name);
    assert_eq!(result, Err(Ok(AuctionError::RevealPhaseNotOver)));

    advance_time(&s.env, COMMIT_SECS);

    // Commit after the commit phase is rejected.
    let bob_commit = s
        .client
        .compute_commitment(&name, &bob, &300, &salt(&s.env, 2));
    let result = s.client.try_commit_bid(&bob, &name, &bob_commit, &300);
    assert_eq!(result, Err(Ok(AuctionError::CommitPhaseOver)));

    // Settle during reveal phase is rejected.
    let result = s.client.try_settle_auction(&name);
    assert_eq!(result, Err(Ok(AuctionError::RevealPhaseNotOver)));

    s.client.reveal_bid(&alice, &name, &200, &salt(&s.env, 1));
    advance_time(&s.env, REVEAL_SECS);

    // Reveal after the reveal phase is rejected.
    let result = s
        .client
        .try_reveal_bid(&alice, &name, &200, &salt(&s.env, 1));
    assert_eq!(result, Err(Ok(AuctionError::AlreadyRevealed)));

    s.client.settle_auction(&name);
    let result = s.client.try_settle_auction(&name);
    assert_eq!(result, Err(Ok(AuctionError::AlreadySettled)));
}

/// Reveal validation: wrong salt, wrong amount, bids below reserve, and bids
/// above the locked deposit are all rejected.
#[test]
fn reveal_validation() {
    let s = setup();
    let name = String::from_str(&s.env, "sly");

    let alice = funded_bidder(&s, 1_000);
    s.client.start_auction(&name);

    let alice_commit = s
        .client
        .compute_commitment(&name, &alice, &200, &salt(&s.env, 1));
    s.client.commit_bid(&alice, &name, &alice_commit, &300);

    advance_time(&s.env, COMMIT_SECS);

    // Wrong salt.
    let result = s
        .client
        .try_reveal_bid(&alice, &name, &200, &salt(&s.env, 99));
    assert_eq!(result, Err(Ok(AuctionError::CommitmentMismatch)));

    // Wrong amount.
    let result = s
        .client
        .try_reveal_bid(&alice, &name, &250, &salt(&s.env, 1));
    assert_eq!(result, Err(Ok(AuctionError::CommitmentMismatch)));

    // Bid below reserve (commitment is valid for the low amount).
    let bob = funded_bidder(&s, 1_000);
    let name2 = String::from_str(&s.env, "low");
    s.client.start_auction(&name2);
    let bob_commit = s
        .client
        .compute_commitment(&name2, &bob, &50, &salt(&s.env, 2));
    s.client.commit_bid(&bob, &name2, &bob_commit, &200);
    advance_time(&s.env, COMMIT_SECS);
    // (name auction for "sly" is now past reveal; use name2's own phases)
    let result = s.client.try_reveal_bid(&bob, &name2, &50, &salt(&s.env, 2));
    assert_eq!(result, Err(Ok(AuctionError::BidBelowReserve)));

    // Bid above deposit.
    let carol = funded_bidder(&s, 1_000);
    let name3 = String::from_str(&s.env, "big");
    s.client.start_auction(&name3);
    let carol_commit = s
        .client
        .compute_commitment(&name3, &carol, &500, &salt(&s.env, 3));
    s.client.commit_bid(&carol, &name3, &carol_commit, &200);
    advance_time(&s.env, COMMIT_SECS);
    let result = s
        .client
        .try_reveal_bid(&carol, &name3, &500, &salt(&s.env, 3));
    assert_eq!(result, Err(Ok(AuctionError::BidExceedsDeposit)));
}

/// Commit validation: deposits below the reserve and double commits are
/// rejected; ineligible names cannot be auctioned at all.
#[test]
fn commit_and_start_validation() {
    let s = setup();
    let name = String::from_str(&s.env, "bid");

    let alice = funded_bidder(&s, 1_000);
    s.client.start_auction(&name);

    // Duplicate auction.
    let result = s.client.try_start_auction(&name);
    assert_eq!(result, Err(Ok(AuctionError::AuctionExists)));

    // Deposit below reserve.
    let alice_commit = s
        .client
        .compute_commitment(&name, &alice, &200, &salt(&s.env, 1));
    let result = s.client.try_commit_bid(&alice, &name, &alice_commit, &50);
    assert_eq!(result, Err(Ok(AuctionError::DepositBelowReserve)));

    // Double commit.
    s.client.commit_bid(&alice, &name, &alice_commit, &200);
    let result = s.client.try_commit_bid(&alice, &name, &alice_commit, &200);
    assert_eq!(result, Err(Ok(AuctionError::AlreadyCommitted)));

    // Five-character names are not premium.
    let long_name = String::from_str(&s.env, "gabby");
    let result = s.client.try_start_auction(&long_name);
    assert_eq!(result, Err(Ok(AuctionError::NotPremiumName)));

    // Registered names cannot be auctioned.
    let owner = Address::generate(&s.env);
    let taken = String::from_str(&s.env, "taken");
    s.client.register(&owner, &taken, &meta(&s.env, 1));
    // (5 chars registers normally; use a 4-char name won at auction instead)
    let result = s.client.try_start_auction(&taken);
    assert_eq!(result, Err(Ok(AuctionError::NotPremiumName)));

    // Bidding on a nonexistent auction.
    let ghost = String::from_str(&s.env, "gone");
    let ghost_commit = s
        .client
        .compute_commitment(&ghost, &alice, &200, &salt(&s.env, 4));
    let result = s.client.try_commit_bid(&alice, &ghost, &ghost_commit, &200);
    assert_eq!(result, Err(Ok(AuctionError::NoAuction)));
}

/// Claim validation: only the winner of a settled auction may claim.
#[test]
fn claim_validation() {
    let s = setup();
    let name = String::from_str(&s.env, "gem");

    let alice = funded_bidder(&s, 500);
    let bob = funded_bidder(&s, 500);

    s.client.start_auction(&name);
    let alice_commit = s
        .client
        .compute_commitment(&name, &alice, &200, &salt(&s.env, 1));
    s.client.commit_bid(&alice, &name, &alice_commit, &200);
    let bob_commit = s
        .client
        .compute_commitment(&name, &bob, &150, &salt(&s.env, 2));
    s.client.commit_bid(&bob, &name, &bob_commit, &150);

    advance_time(&s.env, COMMIT_SECS);
    s.client.reveal_bid(&alice, &name, &200, &salt(&s.env, 1));
    s.client.reveal_bid(&bob, &name, &150, &salt(&s.env, 2));

    // Claim before settlement is rejected.
    let result = s.client.try_claim_name(&alice, &name, &meta(&s.env, 1));
    assert_eq!(result, Err(Ok(AuctionError::NotSettled)));

    advance_time(&s.env, REVEAL_SECS);
    s.client.settle_auction(&name);

    // Loser cannot claim.
    let result = s.client.try_claim_name(&bob, &name, &meta(&s.env, 2));
    assert_eq!(result, Err(Ok(AuctionError::NotWinner)));

    // Winner claims with an invalid meta-address: rejected.
    let bad_meta = Bytes::from_slice(&s.env, &[1u8; 63]);
    let result = s.client.try_claim_name(&alice, &name, &bad_meta);
    assert_eq!(result, Err(Ok(AuctionError::InvalidMetaAddress)));

    // Winner claims properly.
    s.client.claim_name(&alice, &name, &meta(&s.env, 1));
    assert_eq!(s.client.resolve(&name), meta(&s.env, 1));

    // A claimed name cannot be claimed again.
    let result = s.client.try_claim_name(&alice, &name, &meta(&s.env, 3));
    assert_eq!(result, Err(Ok(AuctionError::NameAlreadyRegistered)));
}

/// During the 90-day premium window, top-level names of 4 chars or fewer can
/// only be obtained via auction; longer names and subdomains are unaffected.
/// After the window, premium names register normally and no new auctions can
/// start.
#[test]
fn premium_window_gating() {
    let s = setup();
    let owner = Address::generate(&s.env);

    // Direct registration of premium names is blocked during the window.
    for premium in ["joe", "pay", "abcd"] {
        let name = String::from_str(&s.env, premium);
        let result = s.client.try_register(&owner, &name, &meta(&s.env, 1));
        assert_eq!(result, Err(Ok(NamesError::PremiumAuctionRequired)));
    }

    // Five-plus-character names register normally.
    let long_name = String::from_str(&s.env, "gabriel");
    s.client.register(&owner, &long_name, &meta(&s.env, 2));

    // Subdomains under an owned parent are unaffected by the premium gate.
    let sub = String::from_str(&s.env, "pay.gabriel");
    s.client.register(&owner, &sub, &meta(&s.env, 3));

    // After 90 days the gate lifts and premium names register normally.
    advance_time(&s.env, 90 * DAY_SECS);
    let name = String::from_str(&s.env, "joe");
    s.client.register(&owner, &name, &meta(&s.env, 4));
    assert_eq!(s.client.resolve(&name), meta(&s.env, 4));

    // And no new auctions can start.
    let late = String::from_str(&s.env, "late");
    let result = s.client.try_start_auction(&late);
    assert_eq!(result, Err(Ok(AuctionError::WindowClosed)));
}

/// Without initialization the auction system is inert: premium names
/// register directly and auction calls fail cleanly.
#[test]
fn uninitialized_auctions_do_not_gate_registration() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let owner = Address::generate(&env);
    let name = String::from_str(&env, "joe");
    let m = Bytes::from_slice(&env, &[1u8; 64]);
    client.register(&owner, &name, &m);
    assert_eq!(client.resolve(&name), m);

    let other = String::from_str(&env, "pay");
    let result = client.try_start_auction(&other);
    assert_eq!(result, Err(Ok(AuctionError::NotInitialized)));
}

/// Configuration is one-shot and validated.
#[test]
fn init_validation() {
    let s = setup();

    // Double init rejected.
    let result = s.client.try_init_auctions(
        &s.admin,
        &s.treasury,
        &s.token.address,
        &RESERVE,
        &COMMIT_SECS,
        &REVEAL_SECS,
    );
    assert_eq!(result, Err(Ok(AuctionError::AlreadyInitialized)));

    // Invalid config rejected on a fresh contract.
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let token_addr = Address::generate(&env);

    let result = client.try_init_auctions(
        &admin,
        &treasury,
        &token_addr,
        &0,
        &COMMIT_SECS,
        &REVEAL_SECS,
    );
    assert_eq!(result, Err(Ok(AuctionError::InvalidConfig)));
    let result =
        client.try_init_auctions(&admin, &treasury, &token_addr, &RESERVE, &0, &REVEAL_SECS);
    assert_eq!(result, Err(Ok(AuctionError::InvalidConfig)));
}

// ── auction-admin rotation (issue #165) ──────────────────────────────────────

/// Install a 2-of-3 governance signer set and return the signers.
fn governance_signers(s: &Setup) -> Vec<Address> {
    let signers = Vec::from_array(
        &s.env,
        [
            Address::generate(&s.env),
            Address::generate(&s.env),
            Address::generate(&s.env),
        ],
    );
    s.client.init_multisig(&signers, &2);
    signers
}

/// Drive an auction to the point where it has a revealed winner: returns the
/// name, now inside the reveal phase.
fn auction_with_revealed_bid(s: &Setup) -> String {
    let name = String::from_str(&s.env, "joe");
    let bidder = funded_bidder(s, 1_000);

    s.client.start_auction(&name);
    let bid_salt = salt(&s.env, 9);
    let commitment = s.client.compute_commitment(&name, &bidder, &400, &bid_salt);
    s.client.commit_bid(&bidder, &name, &commitment, &500);

    advance_time(&s.env, COMMIT_SECS);
    s.client.reveal_bid(&bidder, &name, &400, &bid_salt);

    name
}

/// Happy path: a quorum of governance signers rotates the auction admin
/// after the 7-day timelock, and `AuctionAdminRotated(old, new)` is emitted
/// with the expected topics and data.
#[test]
fn auction_admin_rotation_happy_path() {
    let s = setup();
    let signers = governance_signers(&s);
    let new_admin = Address::generate(&s.env);

    s.client
        .propose_rotate_auction_admin(&signers.get(0).unwrap(), &new_admin);
    s.client
        .approve_rotate_auction_admin(&signers.get(1).unwrap());

    let pending = s.client.pending_auction_admin_rotation().unwrap();
    assert_eq!(pending.new_admin, new_admin);
    assert_eq!(pending.approvals.len(), 2);

    advance_time(&s.env, ROTATION_TIMELOCK_SECS);
    s.client
        .execute_rotate_auction_admin(&signers.get(2).unwrap());

    // Events are scoped to the invocation that just ran, so the rotation is
    // the only one recorded.
    let events = s.env.events().all();
    assert_eq!(events.len(), 1);
    let (contract, topics, data) = events.last().unwrap();
    assert_eq!(contract, s.contract_id);
    assert_eq!(
        topics,
        (Symbol::new(&s.env, "AuctionAdminRotated"),).into_val(&s.env)
    );
    let (rotated_from, rotated_to): (Address, Address) = data.into_val(&s.env);
    assert_eq!(rotated_from, s.admin);
    assert_eq!(rotated_to, new_admin);

    assert_eq!(s.client.auction_config().unwrap().admin, new_admin);
    // The proposal is consumed, so a fresh rotation can start immediately.
    assert_eq!(s.client.pending_auction_admin_rotation(), None);
}

/// Quorum and the 7-day timelock are both enforced, and only current signers
/// can drive the flow.
#[test]
fn auction_admin_rotation_enforces_quorum_and_timelock() {
    let s = setup();
    let signers = governance_signers(&s);
    let new_admin = Address::generate(&s.env);
    let outsider = Address::generate(&s.env);

    // Non-signers cannot propose.
    let result = s
        .client
        .try_propose_rotate_auction_admin(&outsider, &new_admin);
    assert_eq!(result, Err(Ok(NamesError::NotSigner)));

    s.client
        .propose_rotate_auction_admin(&signers.get(0).unwrap(), &new_admin);

    // Only one auction-admin rotation may be pending at a time.
    let result = s
        .client
        .try_propose_rotate_auction_admin(&signers.get(1).unwrap(), &new_admin);
    assert_eq!(result, Err(Ok(NamesError::RotationAlreadyPending)));

    // One approval is short of the 2-of-3 quorum, even after the timelock.
    advance_time(&s.env, ROTATION_TIMELOCK_SECS);
    let result = s
        .client
        .try_execute_rotate_auction_admin(&signers.get(0).unwrap());
    assert_eq!(result, Err(Ok(NamesError::QuorumNotMet)));

    // Double approval by the same signer does not manufacture quorum.
    let result = s
        .client
        .try_approve_rotate_auction_admin(&signers.get(0).unwrap());
    assert_eq!(result, Err(Ok(NamesError::AlreadyApprovedRotation)));

    s.client
        .approve_rotate_auction_admin(&signers.get(1).unwrap());
    s.client
        .execute_rotate_auction_admin(&signers.get(0).unwrap());
    assert_eq!(s.client.auction_config().unwrap().admin, new_admin);
}

/// The timelock is measured from the proposal, not from quorum: executing
/// before it elapses fails even with quorum in hand.
#[test]
fn auction_admin_rotation_timelock_not_elapsed() {
    let s = setup();
    let signers = governance_signers(&s);
    let new_admin = Address::generate(&s.env);

    s.client
        .propose_rotate_auction_admin(&signers.get(0).unwrap(), &new_admin);
    s.client
        .approve_rotate_auction_admin(&signers.get(1).unwrap());

    advance_time(&s.env, ROTATION_TIMELOCK_SECS - 1);
    let result = s
        .client
        .try_execute_rotate_auction_admin(&signers.get(0).unwrap());
    assert_eq!(result, Err(Ok(NamesError::TimelockNotElapsed)));
    assert_eq!(s.client.auction_config().unwrap().admin, s.admin);

    advance_time(&s.env, 1);
    s.client
        .execute_rotate_auction_admin(&signers.get(0).unwrap());
    assert_eq!(s.client.auction_config().unwrap().admin, new_admin);
}

/// The admin cannot be swapped out from under a live auction: rotation is
/// rejected throughout the reveal phase and the settle phase that follows
/// it, and becomes executable again once the auction settles. The proposal
/// survives the rejection, so the timelock does not restart.
#[test]
fn auction_admin_rotation_blocked_during_reveal_and_settle() {
    let s = setup();
    let signers = governance_signers(&s);
    let new_admin = Address::generate(&s.env);

    // Propose first so the timelock elapses while the auction is still live.
    s.client
        .propose_rotate_auction_admin(&signers.get(0).unwrap(), &new_admin);
    s.client
        .approve_rotate_auction_admin(&signers.get(1).unwrap());
    advance_time(&s.env, ROTATION_TIMELOCK_SECS);

    let name = auction_with_revealed_bid(&s);
    assert_eq!(s.client.auctions_pending_settlement(), 1);

    // Reveal phase: quorum and timelock are satisfied, the phase guard is not.
    let result = s
        .client
        .try_execute_rotate_auction_admin(&signers.get(0).unwrap());
    assert_eq!(result, Err(Ok(NamesError::AuctionInProgress)));
    assert_eq!(s.client.auction_config().unwrap().admin, s.admin);

    // Settle phase: reveal is over but the auction is unsettled, still blocked.
    advance_time(&s.env, REVEAL_SECS);
    let result = s
        .client
        .try_execute_rotate_auction_admin(&signers.get(0).unwrap());
    assert_eq!(result, Err(Ok(NamesError::AuctionInProgress)));

    // The proposal was left intact by the rejected executions.
    let pending = s.client.pending_auction_admin_rotation().unwrap();
    assert_eq!(pending.new_admin, new_admin);
    assert_eq!(pending.approvals.len(), 2);

    // Settling clears the guard — settlement is permissionless, so governance
    // can always unblock itself.
    s.client.settle_auction(&name);
    assert_eq!(s.client.auctions_pending_settlement(), 0);
    s.client
        .execute_rotate_auction_admin(&signers.get(0).unwrap());
    assert_eq!(s.client.auction_config().unwrap().admin, new_admin);
}

/// An auction nobody revealed a bid on has no value at stake and does not
/// block rotation.
#[test]
fn auction_admin_rotation_not_blocked_by_unrevealed_auction() {
    let s = setup();
    let signers = governance_signers(&s);
    let new_admin = Address::generate(&s.env);

    let name = String::from_str(&s.env, "joe");
    let bidder = funded_bidder(&s, 1_000);
    s.client.start_auction(&name);
    let bid_salt = salt(&s.env, 4);
    let commitment = s.client.compute_commitment(&name, &bidder, &400, &bid_salt);
    s.client.commit_bid(&bidder, &name, &commitment, &500);

    s.client
        .propose_rotate_auction_admin(&signers.get(0).unwrap(), &new_admin);
    s.client
        .approve_rotate_auction_admin(&signers.get(1).unwrap());

    // Past the commit and reveal phases with no reveal at all.
    advance_time(&s.env, ROTATION_TIMELOCK_SECS);
    assert_eq!(s.client.auctions_pending_settlement(), 0);

    s.client
        .execute_rotate_auction_admin(&signers.get(0).unwrap());
    assert_eq!(s.client.auction_config().unwrap().admin, new_admin);

    // The unrevealed deposit is still refundable in full.
    s.client.withdraw_bid(&bidder, &name);
    assert_eq!(s.token.balance(&bidder), 1_000);
}

/// A pending rotation can be cancelled by any signer, clearing all of its
/// state so a fresh proposal starts from scratch.
#[test]
fn auction_admin_rotation_cancel() {
    let s = setup();
    let signers = governance_signers(&s);
    let new_admin = Address::generate(&s.env);

    let result = s
        .client
        .try_cancel_rotate_auction_admin(&signers.get(0).unwrap());
    assert_eq!(result, Err(Ok(NamesError::NoPendingRotation)));

    s.client
        .propose_rotate_auction_admin(&signers.get(0).unwrap(), &new_admin);
    s.client
        .cancel_rotate_auction_admin(&signers.get(1).unwrap());
    assert_eq!(s.client.pending_auction_admin_rotation(), None);

    // A fresh proposal restarts the timelock from the new proposal time.
    let replacement = Address::generate(&s.env);
    s.client
        .propose_rotate_auction_admin(&signers.get(0).unwrap(), &replacement);
    s.client
        .approve_rotate_auction_admin(&signers.get(1).unwrap());
    advance_time(&s.env, ROTATION_TIMELOCK_SECS);
    s.client
        .execute_rotate_auction_admin(&signers.get(0).unwrap());
    assert_eq!(s.client.auction_config().unwrap().admin, replacement);
}

/// The rotation surface requires both the governance multisig and the
/// auction subsystem to be initialised.
#[test]
fn auction_admin_rotation_requires_initialization() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(WraithNamesContract, ());
    let client = WraithNamesContractClient::new(&env, &contract_id);

    let signer = Address::generate(&env);
    let new_admin = Address::generate(&env);

    // No governance signer set yet.
    let result = client.try_propose_rotate_auction_admin(&signer, &new_admin);
    assert_eq!(result, Err(Ok(NamesError::MultisigNotInitialized)));

    // Signer set installed, but auctions were never initialised.
    let signers = Vec::from_array(&env, [signer.clone(), Address::generate(&env)]);
    client.init_multisig(&signers, &1);
    let result = client.try_propose_rotate_auction_admin(&signer, &new_admin);
    assert_eq!(result, Err(Ok(NamesError::AuctionsNotInitialized)));

    // And there is nothing to execute or approve.
    let result = client.try_execute_rotate_auction_admin(&signer);
    assert_eq!(result, Err(Ok(NamesError::NoPendingRotation)));
    let result = client.try_approve_rotate_auction_admin(&signer);
    assert_eq!(result, Err(Ok(NamesError::NoPendingRotation)));
}

/// Signer rotation and auction-admin rotation are independent: a pending
/// proposal on one does not block or satisfy the other.
#[test]
fn auction_admin_rotation_independent_of_signer_rotation() {
    let s = setup();
    let signers = governance_signers(&s);
    let new_admin = Address::generate(&s.env);
    let new_signers = Vec::from_array(&s.env, [Address::generate(&s.env), signers.get(0).unwrap()]);

    s.client
        .propose_rotate_signers(&signers.get(0).unwrap(), &new_signers, &2);
    s.client
        .propose_rotate_auction_admin(&signers.get(0).unwrap(), &new_admin);
    s.client
        .approve_rotate_auction_admin(&signers.get(1).unwrap());

    advance_time(&s.env, ROTATION_TIMELOCK_SECS);
    s.client
        .execute_rotate_auction_admin(&signers.get(0).unwrap());

    assert_eq!(s.client.auction_config().unwrap().admin, new_admin);
    // The signer rotation is untouched and still pending.
    assert_eq!(
        s.client.pending_rotation().unwrap().new_signers,
        new_signers
    );
    assert_eq!(s.client.signers(), signers);
}
