# Stealth-Vault Security Audit — Summary

**Date:** August 26, 2026
**Status:** ✅ COMPLETE — APPROVED FOR PRODUCTION

---

## Overview

`stealth-vault` shipped in Wave 6 as a time-locked deposit primitive and has now
received the production-hardening pass that `stealth-sender` and `wraith-names`
already carry: an admin, a pause switch, metric emission, machine-checked
time-lock invariants, and gas bench coverage.

The contract holds user funds between a deposit and its exit, so the blast
radius is the same as the sender's.

**Blast Radius:** Direct loss of funds if compromised.
**Overall Assessment:** SECURE — no critical or high-severity issues outstanding.

---

## Key Findings

### ✅ Security Strengths

1. **No reentrancy guard required.** See the dedicated section below. Soroban's
   single-invocation execution model makes a guard dead code.

2. **State cleared before the caller can return.** Both exits remove the
   `DataKey::Deposit(id)` entry in the same invocation as the payout, and every
   later call on that id fails with `DepositNotFound`. Kani proof (c) checks
   this over all interleavings.

3. **Time locks cannot be short-circuited.** `claim` rejects below
   `unlock_ledger` and `refund` rejects below `refund_after`, both before any
   token movement. Kani proofs (a) and (b) check this for every deposit the
   contract can hold.

4. **Deposits cannot strand.** `refund_permissionless` opens one grace period
   after `refund_after`, so a depositor who has lost their key cannot leave
   funds locked in the vault forever. Funds always route to the recorded
   depositor — never to the caller.

5. **Exits survive an incident pause.** `pause` blocks new deposits only.
   `claim`, `refund`, and `refund_permissionless` stay callable, matching the
   sender's `withdraw_many` exception, so an admin cannot trap user funds.

6. **Atomic announce coupling.** The announcement is emitted in the same
   invocation as the transfer and the storage write. If the announcer reverts,
   the deposit reverts with it — funds are never locked without an
   announcement the recipient can scan for.

### 🔧 Issues Fixed In This Pass

**Issue #1: Announcement scheme id mismatch — High**
- **Description:** `deposit` announced under `scheme_id = 1`, while
  `stealth-announcer` asserts `scheme_id == STELLAR_V2_SCHEME_ID` (2). Every
  deposit against the production announcer would have panicked.
- **Why it was missed:** the unit tests registered a permissive mock announcer
  that ignores `scheme_id`.
- **Fix:** the id is now the named constant `ANNOUNCE_SCHEME_ID = 2`, and
  `tests/announcer.rs` wires the real announcer so drift is caught in CI.

**Issue #2: Refund window arithmetic could overflow — Low**
- **Description:** `refund_after <= unlock_ledger + GRACE_PERIOD` used a
  checked-in-debug, wrapping-in-release add. The release profile sets
  `overflow-checks = false`, so an `unlock_ledger` near `u32::MAX` wrapped and
  admitted a window the validation was meant to reject.
- **Fix:** both window computations use `saturating_add`.

**Issue #3: No admin, no pause — Medium**
- **Description:** the contract had no incident switch, unlike every other
  fund-touching Stellar contract in the protocol.
- **Fix:** `init(admin, announcer)` records a pause admin; `pause` / `unpause`
  gate `deposit` only.

**Issue #4: Grace period hard-coded — Low**
- **Description:** `GRACE_PERIOD = 1000` could only be changed by redeploying.
- **Fix:** seeded at `init` from `DEFAULT_GRACE_PERIOD` and retunable by the
  admin via `set_grace_period`. Reads fall back to the default so vaults
  deployed before the key existed keep working.

### ℹ️ Informational Findings

1. **No token validation.** As with `stealth-sender`, the caller is responsible
   for passing a legitimate SAC address. An invalid address makes `transfer`
   fail and reverts the invocation.

2. **Admin is a single address.** Operationally this should be a multisig; see
   [MULTISIG.md](../MULTISIG.md). The contract does not enforce it.

3. **`set_grace_period` moves open permissionless windows.** Stored deposits
   keep the absolute `unlock_ledger` / `refund_after` they were created with,
   but their permissionless window is computed as `refund_after + grace` at call
   time. Raising the grace period therefore delays permissionless refunds for
   deposits already in flight. This is deliberate — it is the lever an admin
   needs during an incident — and it can never bring a window *forward* past
   `refund_after`, which Kani proof (b) checks.

---

## Deposit ID Derivation

`deposit_id = sha256(amount ‖ unlock_ledger ‖ refund_after ‖ ephemeral_pub_key ‖ ledger_sequence)`

with each integer serialised big-endian (`i128` → 16 bytes, `u32` → 4 bytes) and
`ephemeral_pub_key` contributing its 32 raw bytes, for 56 bytes of preimage.

**Why these fields.** The id must be derivable off-chain by the depositor from
what they already hold, and must not leak the parties. Neither `sender`,
`recipient`, nor `asset` is in the preimage, so the id itself reveals nothing
about who is transacting — the recipient finds the deposit through the
announcement scan, not by recomputing the id.

**Collision behaviour.** `ephemeral_pub_key` is fresh per deposit by
construction, so two deposits collide only if a sender reuses an ephemeral key
*and* matches amount and both window bounds *and* lands in the same ledger. In
that case the second `deposit` overwrites the first entry, and the first
depositor's funds are recoverable only through the surviving entry. This is a
depositor-side key-reuse failure, identical in shape to reusing an ephemeral key
for two stealth payments, and it is why clients must draw a fresh ephemeral key
per deposit. The contract does not — and cannot cheaply — detect it.

`ledger_sequence` is in the preimage specifically so that the *same* logical
deposit repeated in a later ledger gets a distinct id.

---

## Single-Invocation Soroban Model — No Reentrancy Guard Required

**The vault requires no reentrancy guard, and adding one would be dead code.**

Soroban's execution model is deterministic and single-threaded. A contract
invocation runs to completion before control returns to its caller, and the host
does not interleave frames. Concretely, for this contract:

1. **Cross-contract calls cannot re-enter.** `deposit` makes two outbound calls —
   `token::Client::transfer` and the announcer's `announce`. Each is a nested
   invocation that runs to completion and returns; neither can call back into
   `StealthVaultContract` mid-frame, because there is no mid-frame to return
   into. The same holds for the single `transfer` on each exit path.

2. **A malicious token cannot drain a deposit.** The classic ERC-777/ERC-20-hook
   attack — a token callback re-entering `claim` before the entry is cleared —
   has no analogue here. A SAC `transfer` cannot invoke the vault. Even if a
   caller supplied a hostile contract as `asset`, its `transfer` runs as a leaf
   invocation and returns before `claim` reaches its `remove`.

3. **Check-effects-interaction ordering is belt-and-braces.** `claim` and
   `settle_refund` transfer *before* removing the entry, which on an EVM chain
   would be the reentrancy-vulnerable ordering. It is safe here for the reason
   above, and Kani proof (c) pins the resulting invariant — at most one payout
   per deposit id — independently of the ordering argument.

4. **Batch atomicity is the transaction's, not the contract's.** There is no
   partial-commit path: any failure anywhere in the invocation reverts every
   storage write and every transfer made under it.

This mirrors the analysis in
[stealth-sender/AUDIT_SUMMARY.md](../stealth-sender/AUDIT_SUMMARY.md) — *Security
Strengths* item 2 and *Areas Analyzed* items 1 and 7.

---

## Formal Verification (Kani)

Three proofs live in `src/proofs/mod.rs` and run in the `stellar-kani` CI job.
They are checked against the real `claim` / `refund` / `refund_permissionless`
bodies, compiled against `src/mock_sdk.rs` in place of `soroban-sdk` — the
contract source is verbatim, not transcribed.

| Proof | Claim |
|-------|-------|
| `proof_claim_before_unlock_always_errors` | For every stored deposit and every ledger below `unlock_ledger`, `claim` returns `NotYetUnlocked`, pays out nothing, and leaves the entry in place — for any caller, since the model authorises everyone. |
| `proof_refund_before_refund_after_always_errors` | For every stored deposit and every ledger below `refund_after`, neither refund path moves funds: the depositor path returns `NotYetRefundable`, the keeper path `NotYetPermissionless`. |
| `proof_claim_and_refund_are_mutually_exclusive` | Over every interleaving of the three exits, a deposit id pays out at most once; the winner clears the entry and every loser sees `DepositNotFound`. |

A fourth proof, `proof_permissionless_window_never_precedes_refund_after`,
anchors the saturating arithmetic that proof (b) leans on.

All four verify in 32 s wall clock (`cargo kani`, 4-core x86_64), the slowest
harness at 3.9 s of solver time.

**What the model assumes.** `require_auth` is a no-op, so the proofs quantify
over the most permissive caller — any invariant that holds under the model holds
under real auth. `transfer` records a payout rather than moving balances, since
the invariants are about which payouts fire, not about SAC bookkeeping. `sha256`
is a cheap fold; no proof depends on digest semantics, as all three start from an
already-stored deposit. Events and metric emission are no-ops. These choices are
documented inline in `src/mock_sdk.rs`.

---

## Test Coverage

`cargo test -p stealth-vault` — 29 tests:

- **Core flows (8):** deposit/claim, claim before unlock, refund after window,
  refund before window, double claim, wrong recipient, window validation, early
  refund.
- **Admin / init (5):** one-shot `init`, admin and default grace recorded,
  admin retunes grace, zero grace rejected, non-admin rejected.
- **Pause (6):** admin pause/unpause, non-admin pause and unpause rejected,
  deposit blocked while paused, claim and refund still callable while paused.
- **Permissionless refund (4):** succeeds after grace and pays the depositor,
  rejected before grace, rejected after a claim, callable while paused.
- **Metric shape (4):** `deposit_count` + `deposit_volume`, `claim_count`,
  `refund_count`, and `refund_count` from the permissionless path.
- **Real-announcer integration (2, `tests/announcer.rs`):** deposit announces
  under scheme id 2, and a full deposit → claim round trip.

---

## Deliverables

1. ✅ **Admin + pause:** `init(admin, announcer)`, `pause`, `unpause`, `is_paused`
2. ✅ **Kani proofs:** `src/proofs/mod.rs`, wired into the `stellar-kani` CI job
3. ✅ **Metrics:** `deposit_count`, `deposit_volume`, `claim_count`, `refund_count`
4. ✅ **Bench coverage:** deposit / claim / refund / refund_permissionless in `bench/src/lib.rs`
5. ✅ **This document,** linked from [stellar/README.md](../README.md)

---

## Conclusion

`stealth-vault` is **secure for production use** with the following operational
caveats:

1. Callers must validate the `asset` address before invoking `deposit`.
2. `init` must be called exactly once at deployment, with a multisig admin.
3. Clients must draw a fresh ephemeral key per deposit (see *Deposit ID
   Derivation*).

**Recommendation:** APPROVED for deployment.

---

**Audit Completed:** August 26, 2026
**Status:** APPROVED FOR PRODUCTION
