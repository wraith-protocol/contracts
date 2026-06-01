# SAC Compatibility Audit — Wraith stealth-sender on Stellar

**Date:** 2026-06-01  
**Scope:** `stellar/stealth-sender` v0.1.0 (soroban-sdk 22.0.0)  
**Auditor:** Internal — Wraith Protocol  
**Status:** Draft

---

## Executive Summary

`stealth-sender` calls `token::Client::transfer()` and trusts the asset contract
entirely. It performs no pre-flight checks on token type, issuer flags, or
post-transfer balance. This is safe for permissionless assets (native XLM,
standard issued tokens, immutable-safe tokens) but creates three distinct
failure classes for restricted assets:

1. **Hard failure at send time** — AUTH_REQUIRED assets reject the transfer
   outright if the stealth address has not been pre-authorized. The transaction
   reverts atomically; no funds move and no announcement is emitted. This is a
   UX blocker, not a security issue.

2. **Post-receipt issuer action defeats unlinkability** — AUTH_REVOCABLE and
   AUTH_CLAWBACK_ENABLED assets allow the issuer to freeze or reverse a stealth
   balance after the announcement has been emitted. The recipient cannot
   withdraw. This is a direct security issue: the issuer can correlate the
   announcement with the clawback/freeze target.

3. **Amount mismatch** — Custom Soroban tokens with transfer fees credit less
   than `amount` to the stealth address. Scanners that verify receipt by
   checking the announced amount will see a discrepancy. This is a correctness
   issue.

**Recommendation:** stealth-sender should maintain an explicit allowlist of
supported asset configurations and reject incompatible tokens at send time.

---

## Asset Matrix

| Asset Variant | Transfer Outcome | Announcement Emitted | Post-Receipt Issuer Risk | Stealth Flow Correct | Verdict |
|---|---|---|---|---|---|
| Native XLM | ✅ Succeeds | ✅ Yes | ❌ None (no admin) | ✅ Yes | **SUPPORTED** |
| Standard issued (no flags) | ✅ Succeeds | ✅ Yes | ❌ None | ✅ Yes | **SUPPORTED** |
| AUTH_REQUIRED (not pre-authorized) | ❌ Fails (`BalanceDeauthorizedError`) | ❌ No | N/A | ❌ No | **BLOCKED** |
| AUTH_REQUIRED (pre-authorized) | ✅ Succeeds | ✅ Yes | ⚠️ Issuer can revoke | ⚠️ Conditional | **UNSUPPORTED** |
| AUTH_REVOCABLE | ✅ Succeeds | ✅ Yes | ⚠️ Issuer can freeze balance | ⚠️ Funds may be frozen | **UNSUPPORTED** |
| AUTH_CLAWBACK_ENABLED | ✅ Succeeds | ✅ Yes | 🔴 Issuer can reverse payment | 🔴 Unlinkability broken | **UNSUPPORTED** |
| AUTH_IMMUTABLE (no clawback, no auth-required) | ✅ Succeeds | ✅ Yes | ❌ None (flags frozen safe) | ✅ Yes | **SUPPORTED** |
| AUTH_IMMUTABLE + AUTH_REQUIRED | ❌ Fails permanently | ❌ No | N/A | ❌ No | **BLOCKED** |
| Custom Soroban token (fee hook) | ✅ Succeeds | ✅ Yes | ❌ None | ⚠️ Amount mismatch | **UNSUPPORTED** |

---

## Detailed Findings

### Finding 1 — AUTH_CLAWBACK_ENABLED defeats unlinkability (Critical)

**Severity:** Critical  
**Asset flags:** `AUTH_CLAWBACK_ENABLED`

The SAC `clawback()` function allows the issuer to burn tokens from any contract
balance without the holder's consent. The clawback flag is set at balance
creation time (when the stealth address first receives tokens) and cannot be
removed.

**Attack scenario:**
1. Sender calls `stealth-sender.send()` with a clawback-enabled token.
2. Transfer succeeds; announcement is emitted on-chain.
3. Issuer observes the announcement, identifies the stealth address.
4. Issuer calls `clawback(stealth_address, amount)`.
5. Stealth address balance is zeroed. Recipient's withdrawal fails.
6. The issuer now knows: (a) the stealth address received funds, (b) the
   recipient attempted to withdraw (or will attempt to), and (c) the sender's
   identity can be inferred from the announcement's ephemeral key.

This is a complete break of the unlinkability guarantee for any clawback-enabled
asset.

**Recommendation:** Reject clawback-enabled tokens in `stealth-sender`. Because
the SAC does not expose a read-only flag query in the `TokenInterface`, the
recommended approach is an explicit denylist/allowlist maintained by the
contract owner, or a governance-approved asset registry.

---

### Finding 2 — AUTH_REVOCABLE allows post-receipt balance freeze (High)

**Severity:** High  
**Asset flags:** `AUTH_REVOCABLE`

The issuer can call `set_authorized(stealth_address, false)` after the transfer
completes. The stealth address retains the balance in storage but cannot spend
it. The announcement has already been emitted.

**Impact:** Funds are permanently frozen at the stealth address. The recipient
cannot withdraw. Unlike clawback, the issuer does not recover the funds — they
are simply locked. This is a liveness failure (recipient loses access) rather
than a direct unlinkability break, but it is still a severe UX and trust issue.

**Recommendation:** Treat AUTH_REVOCABLE assets as unsupported. Document this
clearly. If a user sends a revocable asset, they should understand the issuer
can freeze the recipient's balance at any time.

---

### Finding 3 — AUTH_REQUIRED is a UX blocker (Medium)

**Severity:** Medium  
**Asset flags:** `AUTH_REQUIRED`

When the issuer has `AUTH_REQUIRED_FLAG` set, any contract address (including a
stealth address) must be explicitly authorized via `set_authorized` before it
can receive a balance. Since stealth addresses are derived on the fly and are
unknown to the issuer in advance, this makes stealth sends impossible without
issuer cooperation.

**Failure mode:** The `transfer()` call panics with `BalanceDeauthorizedError`
(SAC error code 11). The transaction reverts atomically — no funds move and no
announcement is emitted. This is safe (no partial state), but the send fails
entirely.

**Note on pre-authorization:** Even if the stealth address were pre-authorized
(e.g., via a separate flow), the issuer now knows the stealth address before the
payment, which defeats the purpose of stealth addressing.

**Recommendation:** Reject AUTH_REQUIRED assets. The pre-authorization
requirement is fundamentally incompatible with stealth flows.

---

### Finding 4 — AUTH_IMMUTABLE + AUTH_REQUIRED is permanently broken (Medium)

**Severity:** Medium  
**Asset flags:** `AUTH_IMMUTABLE | AUTH_REQUIRED`

`AUTH_IMMUTABLE` prevents the issuer from changing any flags. If `AUTH_REQUIRED`
was set at issuance, it can never be removed. Every transfer to an unknown
stealth address will fail permanently. There is no remediation path.

**Recommendation:** Same as Finding 3 — reject AUTH_REQUIRED assets. The
immutability makes this a permanent rather than temporary blocker.

---

### Finding 5 — Custom tokens with fee hooks cause amount mismatch (Low)

**Severity:** Low  
**Asset type:** Custom Soroban token (non-SAC) implementing `TokenInterface`

A custom token can deduct a fee during `transfer()`, crediting less than
`amount` to the recipient. The announcement records the requested `amount`, not
the net received amount. Scanners that verify receipt by checking
`balance(stealth_address) == announced_amount` will see a discrepancy.

**Impact:** Correctness issue for scanning. The recipient still receives funds
(just less than announced). No security issue unless the scanner uses the
announced amount to gate withdrawal.

**Recommendation:** Document that custom tokens with non-standard transfer
semantics may cause scanning discrepancies. Consider adding a post-transfer
balance check in `send()` and including the actual received amount in the
announcement metadata, or rejecting tokens that are not known-good SACs.

---

### Finding 6 — No atomic rollback if announcement fails (Low)

**Severity:** Low

`stealth-sender` calls `transfer()` first, then `announce()`. If the announcer
contract panics (e.g., it is upgraded or deregistered), the transfer has already
completed but no announcement is emitted. The recipient's funds are at the
stealth address but the recipient cannot discover them via scanning.

**Impact:** Funds are not lost (the stealth address holds them), but the
recipient has no way to discover the payment without out-of-band notification.

**Note:** In Soroban, a panic in any sub-call reverts the entire transaction, so
in practice this scenario requires the announcer to succeed but emit a malformed
event, or for the announcer to be replaced between the transfer and the
announcement. The risk is low but worth documenting.

**Recommendation:** No code change required. Document the dependency on the
announcer contract's liveness.

---

## Supported Asset Configurations

The following asset configurations are safe for use with Wraith stealth flows:

| Configuration | Rationale |
|---|---|
| Native XLM | No admin, no clawback, no auth requirement. |
| Standard issued asset (no flags) | No issuer restrictions on contract balances. |
| AUTH_IMMUTABLE with no clawback and no auth-required | Flags are frozen in a safe state; no issuer can add restrictions later. |

---

## Unsupported / Rejected Asset Configurations

The following configurations must be rejected or explicitly warned against:

| Configuration | Reason | Recommended Action |
|---|---|---|
| AUTH_CLAWBACK_ENABLED | Issuer can reverse payment post-receipt, breaking unlinkability. | **Reject at send time.** |
| AUTH_REVOCABLE | Issuer can freeze stealth address balance post-receipt. | **Reject at send time.** |
| AUTH_REQUIRED | Stealth address must be pre-authorized; incompatible with stealth. | **Reject at send time.** |
| AUTH_IMMUTABLE + AUTH_REQUIRED | Permanently broken; no remediation path. | **Reject at send time.** |
| Custom token with fee hooks | Amount mismatch between announced and received. | **Warn; document.** |

---

## Recommended Code Changes

### Option A — Asset allowlist (recommended)

Add an `allowed_tokens` set to `stealth-sender` storage, managed by the
contract owner. Only tokens on the allowlist can be used. This is the most
conservative approach and gives Wraith full control over which assets are
supported.

```rust
// In stealth-sender/src/lib.rs
pub fn allow_token(env: Env, owner: Address, token: Address) { ... }
pub fn deny_token(env: Env, owner: Address, token: Address) { ... }

fn assert_token_allowed(env: &Env, token: &Address) {
    if !env.storage().persistent().has(&DataKey::AllowedToken(token.clone())) {
        panic!("token not in Wraith allowlist");
    }
}
```

### Option B — Runtime flag check via StellarAssetClient

Use `token::StellarAssetClient` to query `authorized()` on the stealth address
before transferring. If the asset has AUTH_REQUIRED and the stealth address is
not authorized, fail early with a descriptive error rather than letting the
transfer panic.

This does not protect against clawback (there is no pre-flight check for
clawback capability in the SAC interface), so Option A is still required for
full protection.

### Option C — Documentation only (not recommended)

Document the restrictions and rely on users to only send supported assets. Given
the severity of Finding 1 (clawback breaks unlinkability), this is insufficient.

---

## Test Coverage

Adversarial tests are in `stealth-sender/tests/sac_compat.rs`. Mock contracts
are in `stealth-sender/tests/mocks/`.

| Test | Asset Variant | Expected Outcome |
|---|---|---|
| `native_xlm_send_succeeds` | Native XLM (standard mock) | Transfer + announcement succeed |
| `standard_issued_send_succeeds` | Standard issued | Transfer + announcement succeed |
| `auth_required_send_fails_without_authorization` | AUTH_REQUIRED | Transfer fails; no announcement |
| `auth_required_send_succeeds_when_pre_authorized` | AUTH_REQUIRED (pre-auth) | Transfer + announcement succeed |
| `auth_revocable_send_succeeds_then_issuer_freezes` | AUTH_REVOCABLE | Transfer succeeds; issuer freezes post-receipt |
| `clawback_send_succeeds_then_issuer_claws_back` | AUTH_CLAWBACK_ENABLED | Transfer succeeds; issuer claws back post-receipt |
| `immutable_safe_send_succeeds` | AUTH_IMMUTABLE (safe) | Transfer + announcement succeed |
| `immutable_auth_required_send_fails_permanently` | AUTH_IMMUTABLE + AUTH_REQUIRED | Transfer fails permanently; no announcement |
| `fee_token_send_succeeds_but_amount_mismatch` | Custom token with fee | Transfer succeeds; received < announced |

---

## Follow-up

- [ ] File docs follow-up issue: surface this matrix in `docs/contracts/stellar.mdx`.
- [ ] Implement Option A (asset allowlist) in `stealth-sender`.
- [ ] Add `SenderError::TokenNotAllowed` and `SenderError::TokenNotSupported` error variants.
- [ ] Consider adding a `check_token(token: Address) -> TokenSupport` read-only function for frontends to query before sending.
