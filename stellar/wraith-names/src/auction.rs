//! Sealed-bid auctions for premium `.wraith` names.
//!
//! Short, memorable names (4 characters or fewer) would be squatted at scale
//! on day one under first-come registration. For the first 90 days after the
//! auction system is initialized ("launch"), those names can only be obtained
//! through a commit → reveal → settle sealed-bid auction:
//!
//! 1. **Commit** — bidders submit a hash commitment to their bid and lock a
//!    deposit that must cover the bid. The deposit may exceed the bid, so the
//!    deposit amount does not leak the bid.
//! 2. **Reveal** — bidders disclose `(amount, salt)`; the contract verifies
//!    the commitment and tracks the highest revealed bid.
//! 3. **Settle** — after the reveal phase, anyone (the admin, per the
//!    runbook) settles: the winning bid is paid to the treasury and the
//!    winner's excess deposit is returned. The winner then claims the name
//!    with their stealth meta-address.
//!
//! Losing and unrevealed bids are refundable in full via `withdraw_bid` once
//! the reveal phase ends. Settlement and withdrawal are permissionless so no
//! funds can ever be trapped by an absent admin.

use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{
    contracterror, contracttype, symbol_short, token, Address, Bytes, BytesN, Env, String,
};

/// Domain separator for bid commitments.
pub const AUCTION_COMMITMENT_DOMAIN: &[u8] = b"wraith-names:auction:v1";

/// Top-level names of this length or shorter are premium and must be
/// auctioned during the premium window.
pub const PREMIUM_NAME_MAX_LEN: usize = 4;

/// Length of the premium window after launch: 90 days, in seconds.
pub const PREMIUM_WINDOW_SECS: u64 = 90 * 24 * 60 * 60;

const MAX_NAME_LEN: usize = 32;

const TTL_THRESHOLD: u32 = 17280; // ~1 day
const TTL_EXTEND_TO: u32 = 518400; // ~30 days

/// Storage keys for the auction subsystem.
#[contracttype]
#[derive(Clone)]
pub enum AuctionKey {
    /// Auction configuration (instance storage).
    Config,
    /// Auction state per name hash.
    Auction(BytesN<32>),
    /// Sealed bid per (name hash, bidder).
    Bid(BytesN<32>, Address),
}

/// Auction configuration, set once at initialization.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuctionConfig {
    /// Operator that runs settlements per the runbook.
    pub admin: Address,
    /// Receives winning bids.
    pub treasury: Address,
    /// Payment token (native XLM SAC on mainnet).
    pub token: Address,
    /// Minimum bid; deposits must also be at least this amount.
    pub reserve_price: i128,
    /// Ledger timestamp at initialization; the premium window runs for
    /// `PREMIUM_WINDOW_SECS` from this point.
    pub launch_time: u64,
    /// Duration of the commit phase of each auction, in seconds.
    pub commit_secs: u64,
    /// Duration of the reveal phase of each auction, in seconds.
    pub reveal_secs: u64,
}

/// State of a single name auction.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Auction {
    pub name: String,
    /// Timestamp at which the commit phase ends and the reveal phase begins.
    pub commit_end: u64,
    /// Timestamp at which the reveal phase ends and settlement is possible.
    pub reveal_end: u64,
    /// Highest revealed bidder so far. Ties go to the earliest reveal.
    pub highest_bidder: Option<Address>,
    /// Highest revealed bid amount so far.
    pub highest_amount: i128,
    pub settled: bool,
}

/// A sealed bid.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealedBid {
    /// sha256 commitment binding bidder, name, amount and salt.
    pub commitment: BytesN<32>,
    /// Tokens locked in the contract; refunded in full to losers.
    pub deposit: i128,
    pub revealed: bool,
}

/// Auction errors. Codes start at 100 to stay disjoint from `NamesError`.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum AuctionError {
    NotInitialized = 100,
    AlreadyInitialized = 101,
    InvalidConfig = 102,
    WindowClosed = 103,
    NotPremiumName = 104,
    NameAlreadyRegistered = 105,
    AuctionExists = 106,
    NoAuction = 107,
    CommitPhaseOver = 108,
    AlreadyCommitted = 109,
    DepositBelowReserve = 110,
    RevealPhaseNotActive = 111,
    NoBid = 112,
    AlreadyRevealed = 113,
    CommitmentMismatch = 114,
    BidBelowReserve = 115,
    BidExceedsDeposit = 116,
    RevealPhaseNotOver = 117,
    AlreadySettled = 118,
    NotSettled = 119,
    NotWinner = 120,
    WinnerCannotWithdraw = 121,
    InvalidMetaAddress = 122,
    RegistrationFailed = 123,
}

/// One-time initialization. `launch_time` is the current ledger timestamp.
pub fn init(
    env: &Env,
    admin: Address,
    treasury: Address,
    token: Address,
    reserve_price: i128,
    commit_secs: u64,
    reveal_secs: u64,
) -> Result<(), AuctionError> {
    admin.require_auth();

    if reserve_price <= 0 || commit_secs == 0 || reveal_secs == 0 {
        return Err(AuctionError::InvalidConfig);
    }
    if env.storage().instance().has(&AuctionKey::Config) {
        return Err(AuctionError::AlreadyInitialized);
    }

    let config = AuctionConfig {
        admin,
        treasury,
        token,
        reserve_price,
        launch_time: env.ledger().timestamp(),
        commit_secs,
        reveal_secs,
    };
    env.storage().instance().set(&AuctionKey::Config, &config);

    env.events().publish(
        (symbol_short!("auction"), symbol_short!("init")),
        (config.launch_time, reserve_price),
    );

    Ok(())
}

/// Read the auction configuration, if initialized.
pub fn config(env: &Env) -> Option<AuctionConfig> {
    env.storage().instance().get(&AuctionKey::Config)
}

/// Read an auction's state, if one exists for the name hash.
pub fn load(env: &Env, name_hash: &BytesN<32>) -> Option<Auction> {
    env.storage()
        .persistent()
        .get(&AuctionKey::Auction(name_hash.clone()))
}

/// Whether direct registration of a top-level name of `name_len` characters
/// is currently blocked in favor of the auction.
pub fn premium_block_active(env: &Env, name_len: usize) -> bool {
    if name_len > PREMIUM_NAME_MAX_LEN {
        return false;
    }
    match config(env) {
        None => false,
        Some(cfg) => env.ledger().timestamp() < cfg.launch_time + PREMIUM_WINDOW_SECS,
    }
}

/// Open an auction for an eligible premium name. The caller (lib.rs) has
/// already validated the name and checked that it is unregistered.
pub fn start(env: &Env, name_hash: BytesN<32>, name: String) -> Result<(), AuctionError> {
    let cfg = config(env).ok_or(AuctionError::NotInitialized)?;
    let now = env.ledger().timestamp();

    if now >= cfg.launch_time + PREMIUM_WINDOW_SECS {
        return Err(AuctionError::WindowClosed);
    }

    let auction_key = AuctionKey::Auction(name_hash.clone());
    if env.storage().persistent().has(&auction_key) {
        return Err(AuctionError::AuctionExists);
    }

    let commit_end = now + cfg.commit_secs;
    let reveal_end = commit_end + cfg.reveal_secs;
    let auction = Auction {
        name,
        commit_end,
        reveal_end,
        highest_bidder: None,
        highest_amount: 0,
        settled: false,
    };
    env.storage().persistent().set(&auction_key, &auction);
    env.storage()
        .persistent()
        .extend_ttl(&auction_key, TTL_THRESHOLD, TTL_EXTEND_TO);

    env.events().publish(
        (symbol_short!("auction"), symbol_short!("start"), name_hash),
        (commit_end, reveal_end),
    );

    Ok(())
}

/// Commit a sealed bid and lock `deposit` tokens in the contract.
pub fn commit(
    env: &Env,
    bidder: Address,
    name_hash: BytesN<32>,
    commitment: BytesN<32>,
    deposit: i128,
) -> Result<(), AuctionError> {
    bidder.require_auth();

    let cfg = config(env).ok_or(AuctionError::NotInitialized)?;
    let auction = load(env, &name_hash).ok_or(AuctionError::NoAuction)?;

    if env.ledger().timestamp() >= auction.commit_end {
        return Err(AuctionError::CommitPhaseOver);
    }
    if deposit < cfg.reserve_price {
        return Err(AuctionError::DepositBelowReserve);
    }

    let bid_key = AuctionKey::Bid(name_hash.clone(), bidder.clone());
    if env.storage().persistent().has(&bid_key) {
        return Err(AuctionError::AlreadyCommitted);
    }

    token::Client::new(env, &cfg.token).transfer(
        &bidder,
        &env.current_contract_address(),
        &deposit,
    );

    let bid = SealedBid {
        commitment,
        deposit,
        revealed: false,
    };
    env.storage().persistent().set(&bid_key, &bid);
    env.storage()
        .persistent()
        .extend_ttl(&bid_key, TTL_THRESHOLD, TTL_EXTEND_TO);

    env.events().publish(
        (symbol_short!("auction"), symbol_short!("commit"), name_hash),
        (bidder, deposit),
    );

    Ok(())
}

/// Reveal a committed bid. The bid must match the commitment, meet the
/// reserve price, and be covered by the deposit.
pub fn reveal(
    env: &Env,
    bidder: Address,
    name_hash: BytesN<32>,
    amount: i128,
    salt: BytesN<32>,
) -> Result<(), AuctionError> {
    bidder.require_auth();

    let cfg = config(env).ok_or(AuctionError::NotInitialized)?;
    let auction_key = AuctionKey::Auction(name_hash.clone());
    let mut auction = load(env, &name_hash).ok_or(AuctionError::NoAuction)?;

    let bid_key = AuctionKey::Bid(name_hash.clone(), bidder.clone());
    let mut bid: SealedBid = env
        .storage()
        .persistent()
        .get(&bid_key)
        .ok_or(AuctionError::NoBid)?;

    if bid.revealed {
        return Err(AuctionError::AlreadyRevealed);
    }

    let now = env.ledger().timestamp();
    if now < auction.commit_end || now >= auction.reveal_end {
        return Err(AuctionError::RevealPhaseNotActive);
    }

    let expected = compute_commitment(env, &auction.name, &bidder, amount, &salt);
    if expected != bid.commitment {
        return Err(AuctionError::CommitmentMismatch);
    }
    if amount < cfg.reserve_price {
        return Err(AuctionError::BidBelowReserve);
    }
    if amount > bid.deposit {
        return Err(AuctionError::BidExceedsDeposit);
    }

    bid.revealed = true;
    env.storage().persistent().set(&bid_key, &bid);

    if amount > auction.highest_amount {
        auction.highest_bidder = Some(bidder.clone());
        auction.highest_amount = amount;
        env.storage().persistent().set(&auction_key, &auction);
    }

    env.events().publish(
        (symbol_short!("auction"), symbol_short!("reveal"), name_hash),
        (bidder, amount),
    );

    Ok(())
}

/// Settle an auction after the reveal phase ends.
///
/// If there is a winner, their bid amount is transferred to the treasury and
/// any excess deposit is refunded to them immediately. Permissionless.
pub fn settle(env: &Env, name_hash: BytesN<32>) -> Result<(), AuctionError> {
    let cfg = config(env).ok_or(AuctionError::NotInitialized)?;
    let auction_key = AuctionKey::Auction(name_hash.clone());
    let mut auction = load(env, &name_hash).ok_or(AuctionError::NoAuction)?;

    if env.ledger().timestamp() < auction.reveal_end {
        return Err(AuctionError::RevealPhaseNotOver);
    }
    if auction.settled {
        return Err(AuctionError::AlreadySettled);
    }

    if let Some(ref winner) = auction.highest_bidder {
        let bid_key = AuctionKey::Bid(name_hash.clone(), winner.clone());
        let bid: SealedBid = env
            .storage()
            .persistent()
            .get(&bid_key)
            .ok_or(AuctionError::NoBid)?;

        let token_client = token::Client::new(env, &cfg.token);
        token_client.transfer(
            &env.current_contract_address(),
            &cfg.treasury,
            &auction.highest_amount,
        );
        let excess = bid.deposit - auction.highest_amount;
        if excess > 0 {
            token_client.transfer(&env.current_contract_address(), winner, &excess);
        }
        env.storage().persistent().remove(&bid_key);
    }

    auction.settled = true;
    env.storage().persistent().set(&auction_key, &auction);
    env.storage()
        .persistent()
        .extend_ttl(&auction_key, TTL_THRESHOLD, TTL_EXTEND_TO);

    env.events().publish(
        (symbol_short!("auction"), symbol_short!("settle"), name_hash),
        (auction.highest_bidder.clone(), auction.highest_amount),
    );

    Ok(())
}

/// Refund a losing or unrevealed bid in full once the reveal phase ends.
///
/// The highest bidder cannot withdraw; their deposit is handled by `settle`
/// (which anyone can call), so no funds can be trapped.
pub fn withdraw(env: &Env, bidder: Address, name_hash: BytesN<32>) -> Result<(), AuctionError> {
    bidder.require_auth();

    let cfg = config(env).ok_or(AuctionError::NotInitialized)?;
    let auction = load(env, &name_hash).ok_or(AuctionError::NoAuction)?;

    if env.ledger().timestamp() < auction.reveal_end {
        return Err(AuctionError::RevealPhaseNotOver);
    }
    if auction.highest_bidder == Some(bidder.clone()) && !auction.settled {
        return Err(AuctionError::WinnerCannotWithdraw);
    }

    let bid_key = AuctionKey::Bid(name_hash.clone(), bidder.clone());
    let bid: SealedBid = env
        .storage()
        .persistent()
        .get(&bid_key)
        .ok_or(AuctionError::NoBid)?;

    env.storage().persistent().remove(&bid_key);

    token::Client::new(env, &cfg.token).transfer(
        &env.current_contract_address(),
        &bidder,
        &bid.deposit,
    );

    env.events().publish(
        (symbol_short!("auction"), symbol_short!("refund"), name_hash),
        (bidder, bid.deposit),
    );

    Ok(())
}

/// Check that `winner` may claim the name for a settled auction.
pub fn verify_claim(
    env: &Env,
    winner: &Address,
    name_hash: &BytesN<32>,
) -> Result<(), AuctionError> {
    let auction = load(env, name_hash).ok_or(AuctionError::NoAuction)?;
    if !auction.settled {
        return Err(AuctionError::NotSettled);
    }
    if auction.highest_bidder != Some(winner.clone()) {
        return Err(AuctionError::NotWinner);
    }
    Ok(())
}

/// Compute the sealed-bid commitment:
/// `sha256(domain || name || bidder_xdr || amount_be || salt)`.
///
/// Binding the bidder prevents commitment copying; the 32-byte salt keeps
/// low-entropy amounts unguessable.
pub fn compute_commitment(
    env: &Env,
    name: &String,
    bidder: &Address,
    amount: i128,
    salt: &BytesN<32>,
) -> BytesN<32> {
    let mut message = Bytes::from_slice(env, AUCTION_COMMITMENT_DOMAIN);

    let name_len = (name.len() as usize).min(MAX_NAME_LEN);
    let mut name_buf = [0u8; MAX_NAME_LEN];
    name.copy_into_slice(&mut name_buf[..name_len]);
    message.extend_from_slice(&name_buf[..name_len]);

    message.append(&bidder.clone().to_xdr(env));
    message.extend_from_slice(&amount.to_be_bytes());
    message.extend_from_slice(&salt.to_array());

    BytesN::from_array(env, &env.crypto().sha256(&message).to_array())
}
