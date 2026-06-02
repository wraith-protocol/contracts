# Security Audit: Stealth-Sender Soroban Contract
**Date:** May 30, 2026  
**Auditor:** Security Review  
**Scope:** `stealth-sender/src/lib.rs` and atomic coupling with `stealth-announcer`  
**Severity Matrix:** Critical, High, Medium, Low, Informational

---

## Executive Summary

The stealth-sender contract implements atomic "transfer asset + emit announcement" operations for stealth payments on Stellar. The contract interoperates with Stellar Asset Contracts (SAC) and must handle both native XLM and arbitrary issued assets without loss of funds.

**Overall Assessment:** PASS with recommendations.

The contract's core design is sound:
- Atomic coupling between token transfer and announcement prevents the critical failure mode (funds moved without announcement).
- Auth model correctly requires sender authorization via `require_auth()`.
- Batch operations maintain all-or-nothing semantics through Soroban's transaction model.
- No reentrancy vectors identified due to Soroban's execution model.

**Critical Issues:** None identified.  
**High Issues:** None identified.  
**Medium Issues:** 1 (init re-initialization risk).  
**Low Issues:** 2 (informational).

---

## Detailed Findings

### 1. Token Contract Trust & Reentrancy Analysis

**Status:** ✅ PASS

**Finding:**
The contract calls `token::Client::new(&env, &token).transfer(...)` without validating the token contract address. However, reentrancy is not a concern in Soroban due to its execution model:

- **Soroban's Execution Model:** Soroban uses a single-threaded, deterministic execution environment. Cross-contract calls (CPI) are serialized and cannot interleave with the calling contract's execution.
- **No Callback Vectors:** A malicious SAC cannot call back into `stealth-sender` during the `transfer()` call. The SAC's execution completes atomically before control returns.
- **Token Validation:** The contract does not validate that `token` is a legitimate SAC. However, this is by design—the caller is responsible for providing a valid token address. If an invalid address is provided, the `transfer()` call will fail, and the entire transaction reverts (no partial state).

**Recommendation:**
Document that callers must validate the token address before invoking `send()` or `batch_send()`. Consider adding an optional registry lookup in a future version if token validation becomes a concern.

**Test Coverage:** See `test_malicious_token_reentry_attempt()` in audit test suite.

---

### 2. Native Asset vs. Issued Asset Divergence

**Status:** ✅ PASS

**Finding:**
The contract uses the same `token::Client::transfer()` interface for both native XLM and issued assets. Soroban's token interface abstracts this difference:

- **Native XLM:** Handled by the native token contract (address `CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4`).
- **Issued Assets:** Handled by individual SAC instances.

Both paths use identical code:
```rust
token_client.transfer(&sender, &stealth_address, &amount);
```

**Success Path:** Both native and issued assets transfer correctly.

**Failure Path:** If `transfer()` fails (insufficient balance, frozen account, etc.), the entire transaction reverts. No asymmetry exists where fees are paid but funds don't reach the stealth address.

**Test Coverage:** See `test_native_vs_issued_asset_parity()` in audit test suite.

---

### 3. Batch Send Atomicity

**Status:** ✅ PASS

**Finding:**
The `batch_send()` function processes multiple transfers in a loop:

```rust
for i in 0..len {
    token_client.transfer(&sender, &stealth_address, &amount);
    announcer_client::announce(...);
}
```

**Atomicity Guarantee:** Soroban transactions are atomic. If any operation fails (e.g., a transfer in the middle of the batch), the entire transaction reverts, including all prior transfers and announcements. This is enforced by the Soroban runtime.

**Verification:** If a transfer fails mid-batch, the transaction reverts, and no partial state is committed.

**Test Coverage:** See `test_batch_send_atomicity_mid_batch_failure()` in audit test suite.

---

### 4. Announcer Call Coupling

**Status:** ✅ PASS

**Finding:**
The contract invokes the announcer atomically after each transfer:

```rust
token_client.transfer(&sender, &stealth_address, &amount);
announcer_client::announce(&env, &announcer, ...);
```

**Panic Behavior:** If the announcer panics (or returns an error), the entire transaction reverts. Funds are not moved without an announcement.

**Coupling Guarantee:** The transfer and announcement are coupled within the same transaction. If the announcement fails, the transfer is rolled back.

**Potential Issue:** The announcer contract is pure event-emission with no storage or access control. It cannot panic under normal conditions. However, if the announcer address is invalid or points to a non-existent contract, the `invoke_contract` call will fail, and the transaction reverts.

**Test Coverage:** See `test_announcer_panic_reverts_transfer()` in audit test suite.

---

### 5. Fee & Refund Flows

**Status:** ✅ PASS

**Finding:**
Soroban does not have a built-in fee mechanism like Ethereum's `msg.value`. Instead:

- **Resource Fees:** Paid by the transaction submitter upfront.
- **No Partial Failure:** If a transaction fails, all state changes are reverted, and fees are not refunded.

The contract does not handle fees or refunds explicitly. The caller pays Soroban resource fees, and if the transaction fails, those fees are lost (standard Soroban behavior).

**Asymmetry Check:** No asymmetry exists where the caller pays fees but funds don't reach the stealth address. Either the entire transaction succeeds (funds transferred, announcement emitted), or it fails entirely (all state reverted).

**Test Coverage:** Covered by general transaction atomicity tests.

---

### 6. Auth Caching & `require_auth_for_args`

**Status:** ✅ PASS

**Finding:**
The contract uses `sender.require_auth()` to verify that the sender has authorized the operation:

```rust
pub fn send(env: Env, sender: Address, ...) -> Result<(), SenderError> {
    sender.require_auth();
    // ... transfer and announce
}
```

**Auth Model:**
- `require_auth()` checks that the `sender` address has signed the current transaction.
- The auth context is established at the transaction level and is valid for the entire contract invocation.
- No auth caching issues exist because Soroban's auth model is stateless per transaction.

**Token Spender Authorization:**
- The contract does not need explicit approval from the sender for the token transfer. Instead, the contract itself is the spender, and the sender's `require_auth()` implicitly authorizes the transfer.
- This is the standard Soroban pattern and is secure.

**Verification:** The auth context covers the sender correctly. The token transfer is authorized by the sender's signature.

**Test Coverage:** See `test_auth_required_for_send()` in audit test suite.

---

### 7. Reentrancy via CPI / Nested Contract Calls

**Status:** ✅ PASS

**Finding:**
The contract calls the announcer via `env.invoke_contract()`:

```rust
let _: () = env.invoke_contract(
    announcer,
    &soroban_sdk::symbol_short!("announce"),
    soroban_sdk::vec![...],
);
```

**Reentrancy Analysis:**
- **Soroban's Execution Model:** CPI calls are serialized. The announcer contract executes to completion before control returns to the sender contract.
- **No Callback Vectors:** The announcer cannot call back into the sender during its execution.
- **Nested Calls:** Even if the announcer calls another contract, that contract cannot call back into the sender (no circular call chains).

**Conclusion:** Reentrancy is not possible in Soroban due to its deterministic, single-threaded execution model.

**Test Coverage:** See `test_nested_contract_calls_no_reentrancy()` in audit test suite.

---

### 8. Init / Upgrade Story

**Status:** ⚠️ MEDIUM SEVERITY

**Finding:**
The `init()` function stores the announcer address:

```rust
pub fn init(env: Env, announcer: Address) -> Result<(), SenderError> {
    if env.storage().instance().has(&DataKey::Announcer) {
        return Err(SenderError::AlreadyInitialized);
    }
    env.storage().instance().set(&DataKey::Announcer, &announcer);
    Ok(())
}
```

**Issue:** The check `has(&DataKey::Announcer)` prevents re-initialization, but only if the key exists. If the contract is deployed and `init()` is never called, the contract is in an uninitialized state. Subsequent calls to `send()` or `batch_send()` will fail with `NotInitialized`.

**Risk:** If `init()` is not called during deployment, the contract is unusable. This is not a security issue but an operational risk.

**Recommendation:**
1. Document that `init()` must be called exactly once during deployment.
2. Consider using a constructor pattern or a factory contract to ensure `init()` is called atomically with deployment.
3. Alternatively, use a lazy-initialization pattern where the announcer is set on first use (less safe but more forgiving).

**Test Coverage:** See `test_init_one_shot_semantics()` in audit test suite.

---

### 9. Informational Findings

#### 9.1 No Explicit Validation of Stealth Address

**Status:** ℹ️ INFORMATIONAL

The contract does not validate that `stealth_address` is a valid Stellar address. However, this is acceptable because:
- The caller is responsible for generating a valid stealth address.
- If an invalid address is provided, the token transfer will fail, and the transaction reverts.
- No funds are lost.

#### 9.2 Metadata Size Unbounded

**Status:** ℹ️ INFORMATIONAL

The `metadata` field in the announcement is unbounded. Large metadata could increase transaction costs. However, this is a caller concern, not a contract issue.

---

## Recommendations

### Immediate Actions
1. ✅ Document that callers must validate the token address.
2. ✅ Document that `init()` must be called exactly once during deployment.
3. ✅ Add comprehensive tests for adversarial scenarios (see audit test suite).

### Future Enhancements
1. Consider a token registry or whitelist for additional safety.
2. Consider a factory contract pattern to ensure `init()` is called atomically.
3. Add event logging for successful transfers (optional, for indexing).

---

## Test Coverage

The audit test suite (`tests/audit.rs`) includes:

1. **Malicious Token Reentrancy:** Attempts to reenter the sender during transfer.
2. **Batch Atomicity:** Injects a failing transfer mid-batch and verifies rollback.
3. **Announcer Panic:** Verifies that funds are not moved if the announcer fails.
4. **Auth Requirements:** Verifies that unauthorized senders are rejected.
5. **Native vs. Issued Assets:** Verifies parity between XLM and issued assets.
6. **Init One-Shot:** Verifies that `init()` cannot be called twice.
7. **Nested Contract Calls:** Verifies no reentrancy via nested calls.

All tests pass. See `tests/audit.rs` for details.

---

## Conclusion

The stealth-sender contract is **secure for production use** with the following caveats:

1. Callers must validate the token address before invoking `send()` or `batch_send()`.
2. The contract must be initialized with `init()` exactly once during deployment.
3. The contract relies on Soroban's atomic transaction model for safety. Any changes to the execution model could affect security.

**Recommendation:** APPROVED for deployment with the above caveats documented.

---

## Appendix: Severity Matrix

| Severity | Definition |
|----------|-----------|
| **Critical** | Funds can be lost or stolen; contract is unusable. |
| **High** | Significant security issue; requires immediate fix. |
| **Medium** | Operational or design issue; should be addressed. |
| **Low** | Minor issue; nice to have. |
| **Informational** | Observation; no action required. |

---

**Audit Date:** May 30, 2026  
**Status:** COMPLETE
