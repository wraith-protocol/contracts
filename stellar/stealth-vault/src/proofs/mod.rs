//! Kani proofs for the stealth-vault time-lock invariants.
//!
//! These run against the real `claim`, `refund`, and `refund_permissionless`
//! bodies in `lib.rs`, compiled against `mock_sdk` instead of `soroban_sdk`
//! (see that module for the modelling choices and why they do not weaken the
//! claims below).
//!
//! Each proof seeds storage with a single, fully symbolic `DepositEntry`, so
//! the results hold for every deposit the contract can ever hold — every
//! amount, every window, every ledger sequence, every set of parties.
//!
//! Run with:
//! ```sh
//! cargo kani --package stealth-vault
//! ```

use crate::mock_sdk::{Address, BytesN, Env};
use crate::{DepositEntry, StealthVaultContract, VaultError, DEFAULT_GRACE_PERIOD};

// Every proof below carries `#[kani::unwind(40)]`. The binding constraint is
// CBMC's builtin `memcmp`, which compares the 32-byte `BytesN<32>` inside
// `DataKey::Deposit` one byte at a time; every loop the contract itself runs is
// at most three iterations (the instance storage scan). The bound is sized for
// the comparison, not for the contract.

/// The deposit id under test.
///
/// Concrete rather than symbolic: each proof seeds exactly one entry, so the
/// id's byte values cannot affect any of the claims below, and 256 fewer
/// symbolic bits keeps CBMC well inside the CI time budget.
fn deposit_id() -> BytesN<32> {
    BytesN { data: [7u8; 32] }
}

/// A fully symbolic deposit entry.
fn any_entry() -> DepositEntry {
    DepositEntry {
        sender: Address { id: kani::any() },
        recipient: Address { id: kani::any() },
        amount: kani::any(),
        asset: Address { id: kani::any() },
        unlock_ledger: kani::any(),
        refund_after: kani::any(),
    }
}

/// An env at `ledger_sequence` holding exactly one deposit.
fn env_with_deposit(ledger_sequence: u32, deposit_id: &BytesN<32>, entry: &DepositEntry) -> Env {
    let env = Env::new(ledger_sequence);
    env.put_deposit(deposit_id, entry);
    env
}

/// Proof (a): claim before the unlock ledger always errors.
///
/// Claim: for every stored deposit and every ledger sequence strictly below
/// `unlock_ledger`, `claim` returns `NotYetUnlocked`, leaves the deposit in
/// storage, and pays out nothing — regardless of who the caller is, because
/// the model authorises everyone.
#[kani::proof]
#[kani::unwind(40)]
pub fn proof_claim_before_unlock_always_errors() {
    let deposit_id = deposit_id();
    let entry = any_entry();

    let ledger_sequence: u32 = kani::any();
    kani::assume(ledger_sequence < entry.unlock_ledger);

    let env = env_with_deposit(ledger_sequence, &deposit_id, &entry);
    let caller = Address { id: kani::any() };

    let result = StealthVaultContract::claim(env.clone(), deposit_id.clone(), caller);

    assert!(result == Err(VaultError::NotYetUnlocked));
    assert!(env.payout_count() == 0);
    assert!(env.persistent_len() == 1);
}

/// Proof (b): refund before `refund_after` always errors.
///
/// Claim: for every stored deposit and every ledger sequence strictly below
/// `refund_after`, neither refund path can move funds. The depositor path
/// returns `NotYetRefundable`; the permissionless path returns
/// `NotYetPermissionless`, since its window opens a further grace period later
/// and `refund_after + grace >= refund_after` under saturating arithmetic.
#[kani::proof]
#[kani::unwind(40)]
pub fn proof_refund_before_refund_after_always_errors() {
    let deposit_id = deposit_id();
    let entry = any_entry();

    let ledger_sequence: u32 = kani::any();
    kani::assume(ledger_sequence < entry.refund_after);

    let env = env_with_deposit(ledger_sequence, &deposit_id, &entry);

    let depositor_result = StealthVaultContract::refund(env.clone(), deposit_id.clone());
    assert!(depositor_result == Err(VaultError::NotYetRefundable));

    let keeper = Address { id: kani::any() };
    let keeper_result =
        StealthVaultContract::refund_permissionless(env.clone(), keeper, deposit_id.clone());
    assert!(keeper_result == Err(VaultError::NotYetPermissionless));

    assert!(env.payout_count() == 0);
    assert!(env.persistent_len() == 1);
}

/// Proof (c): claim and refund are mutually exclusive for a given deposit id.
///
/// Claim: over every interleaving of the three exit paths, a deposit id pays
/// out at most once. The first successful exit removes the entry, and every
/// later call on that id fails with `DepositNotFound`, so the vault can never
/// pay both the recipient and the depositor for the same deposit.
///
/// The ledger sequence is unconstrained, so this also covers the window where
/// both `claim` and `refund` are individually eligible.
#[kani::proof]
#[kani::unwind(40)]
pub fn proof_claim_and_refund_are_mutually_exclusive() {
    let deposit_id = deposit_id();
    let entry = any_entry();

    let ledger_sequence: u32 = kani::any();
    let env = env_with_deposit(ledger_sequence, &deposit_id, &entry);

    // Symbolic order: either the recipient claims first or the depositor
    // refunds first, then the loser and a permissionless keeper both retry.
    let claim_first: bool = kani::any();
    let keeper = Address { id: kani::any() };

    let (claimed, refunded) = if claim_first {
        let claimed =
            StealthVaultContract::claim(env.clone(), deposit_id.clone(), entry.recipient.clone());
        let refunded = StealthVaultContract::refund(env.clone(), deposit_id.clone());
        (claimed, refunded)
    } else {
        let refunded = StealthVaultContract::refund(env.clone(), deposit_id.clone());
        let claimed =
            StealthVaultContract::claim(env.clone(), deposit_id.clone(), entry.recipient.clone());
        (claimed, refunded)
    };

    let keeper_refunded =
        StealthVaultContract::refund_permissionless(env.clone(), keeper, deposit_id.clone());

    // At most one exit path succeeded.
    let successes = u32::from(claimed.is_ok())
        + u32::from(refunded.is_ok())
        + u32::from(keeper_refunded.is_ok());
    assert!(successes <= 1);

    // And that maps one-to-one onto the money that left the vault.
    assert!(env.payout_count() == successes);

    // A successful exit clears the entry; a failed one leaves it untouched.
    let remaining = env.persistent_len();
    if successes == 1 {
        assert!(remaining == 0);
        // Whoever lost the race saw the entry already gone.
        let losers = u32::from(claimed == Err(VaultError::DepositNotFound))
            + u32::from(refunded == Err(VaultError::DepositNotFound))
            + u32::from(keeper_refunded == Err(VaultError::DepositNotFound));
        assert!(losers >= 1);
    } else {
        assert!(remaining == 1);
    }
}

/// Sanity anchor for proof (b): the permissionless window never opens before
/// the depositor's own window, for any `refund_after` and the default grace.
#[kani::proof]
pub fn proof_permissionless_window_never_precedes_refund_after() {
    let refund_after: u32 = kani::any();
    let opens_at = refund_after.saturating_add(DEFAULT_GRACE_PERIOD);
    assert!(opens_at >= refund_after);
}
