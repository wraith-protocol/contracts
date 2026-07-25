# Threat Model: Wraith Protocol Stellar Contracts

## Overview

This document presents a comprehensive threat analysis of the Wraith Protocol smart contracts on Stellar using the STRIDE methodology (Spoofing, Tampering, Repudiation, Information Disclosure, Denial of Service, Elevation of Privilege).

**Target System:** Wraith Protocol Stellar Smart Contracts  
**Analysis Date:** June 2026  
**Methodology:** STRIDE + Attack Trees  
**Scope:** Core contracts (announcer, registry, sender, names)

## Trust Boundaries

### Boundary 1: User ↔ Contract
- **Entry Points:** All public contract functions
- **Auth Mechanism:** Soroban `require_auth()`
- **Trust Level:** Zero trust (user is adversarial)

### Boundary 2: Contract ↔ Contract
- **Entry Points:** `invoke_contract()` calls
- **Auth Mechanism:** Caller identity + authorization context
- **Trust Level:** Depends on called contract (SAC = trusted, user contracts = untrusted)

### Boundary 3: Contract ↔ Storage
- **Entry Points:** Storage read/write operations
- **Auth Mechanism:** Contract instance isolation
- **Trust Level:** Trusted (Soroban runtime enforced)

### Boundary 4: Admin ↔ Upgradeable Contracts
- **Entry Points:** Upgrade functions (future implementation)
- **Auth Mechanism:** Multisig (3-of-5) + Timelock (7 days)
- **Trust Level:** Semi-trusted (mitigated by timelock + transparency)

## STRIDE Analysis by Contract

---

## 1. stealth-announcer

### Spoofing
**Threat:** Impersonate another user when emitting announcements.

**Mitigations:**
- ✅ Contract emits caller address in event data
- ✅ No identity checks (permissionless by design)
- ✅ Recipient can validate sender off-chain

**Residual Risk:** **Low** - Spoofing is detectable and doesn't affect security

---

### Tampering
**Threat:** Alter announcement data (stealth address, ephemeral key, metadata).

**Mitigations:**
- ✅ No storage (events are immutable once emitted)
- ✅ Soroban ensures atomic event emission
- ✅ Network-level integrity (Stellar consensus)

**Residual Risk:** **None** - Cannot tamper with emitted events

---

### Repudiation
**Threat:** Deny having made an announcement.

**Mitigations:**
- ✅ Events include caller address
- ✅ Transaction hash provides proof
- ✅ Ledger provides timestamp

**Residual Risk:** **None** - All announcements are non-repudiable

---

### Information Disclosure
**Threat:** Leak sensitive information (link sender to recipient).

**Mitigations:**
- ✅ Announcements are public by design
- ✅ Stealth address breaks on-chain linkability
- ✅ No private data stored

**Residual Risk:** **Low** - Privacy relies on cryptographic protocol, not smart contract

---

### Denial of Service
**Threat:** Prevent legitimate announcements from being emitted.

**Mitigations:**
- ✅ Permissionless (no access control to DOS)
- ✅ No storage writes (cannot exhaust storage)
- ✅ Gas costs are minimal

**Attack Scenarios:**
- Spam announcements: Mitigated by gas costs
- Contract upgrade to censor: **Not possible (frozen contract)**

**Residual Risk:** **Low** - Network-level DOS only

---

### Elevation of Privilege
**Threat:** Gain admin rights or upgrade frozen contract.

**Mitigations:**
- ✅ No admin role exists
- ✅ No upgrade mechanism
- ✅ Contract is frozen permanently

**Residual Risk:** **None** - No privilege to elevate

---

## 2. stealth-registry

### Spoofing
**Threat:** Register keys on behalf of another user.

**Mitigations:**
- ✅ `registrant.require_auth()` enforced
- ✅ Only registrant can modify their own keys
- ✅ No admin override

**Residual Risk:** **None** - Soroban auth prevents spoofing

---

### Tampering
**Threat:** Alter another user's registered keys.

**Mitigations:**
- ✅ Storage isolation by (registrant, scheme_id)
- ✅ Authorization check on all write operations
- ✅ No admin can modify user data (frozen contract)

**Residual Risk:** **None** - User sovereignty guaranteed

---

### Repudiation
**Threat:** Deny having registered keys.

**Mitigations:**
- ✅ Registration events emitted
- ✅ Transaction signatures provide proof
- ✅ On-chain storage provides evidence

**Residual Risk:** **None** - All registrations are non-repudiable

---

### Information Disclosure
**Threat:** Leak user stealth meta-addresses.

**Mitigations:**
- ⚠️ **Meta-addresses are public by design**
- ✅ Storage is accessible to anyone (intentional)
- ✅ No private keys stored (only public keys)

**Residual Risk:** **Medium** - Privacy model assumes meta-addresses are public

---

### Denial of Service
**Threat:** Prevent users from registering keys.

**Mitigations:**
- ✅ Permissionless registration
- ✅ No rate limiting (gas costs provide economic DOS protection)
- ✅ TTL management prevents storage bloat

**Attack Scenarios:**
- Register many keys to exhaust storage: Mitigated by storage rent
- Front-run registration: Not profitable (user chooses their own keys)

**Residual Risk:** **Low** - Economic incentives prevent DOS

---

### Elevation of Privilege
**Threat:** Gain ability to censor or alter registrations.

**Mitigations:**
- ✅ No admin role exists
- ✅ No upgrade mechanism
- ✅ Contract is frozen permanently

**Residual Risk:** **None** - No privilege to elevate

---

## 3. stealth-sender

### Spoofing
**Threat:** Send tokens from another user's account.

**Mitigations:**
- ✅ `sender.require_auth()` enforced
- ✅ Token transfer requires sender authorization
- ✅ SAC propagates authorization correctly

**Residual Risk:** **None** - Soroban auth prevents spoofing

---

### Tampering
**Threat:** Alter transfer amount or recipient.

**Mitigations:**
- ✅ Parameters are verified before transfer
- ✅ Atomic execution prevents race conditions
- ✅ No storage of transfer details (pass-through)

**Attack Scenarios:**
- Malicious token contract: ⚠️ **Documented risk** (caller must validate token address)
- Announcer address manipulation: ✅ Mitigated (init one-shot semantics)

**Residual Risk:** **Low** - Malicious token risk is documented

---

### Repudiation
**Threat:** Deny having sent tokens.

**Mitigations:**
- ✅ Transaction signatures provide proof
- ✅ Announcement events provide public record
- ✅ Token transfer emits SAC events

**Residual Risk:** **None** - All transfers are non-repudiable

---

### Information Disclosure
**Threat:** Leak sender-recipient linkage.

**Mitigations:**
- ✅ Stealth address cryptography prevents linkage
- ✅ Contract doesn't log sender-recipient mappings
- ✅ Announcements don't reveal recipient identity

**Residual Risk:** **Low** - Privacy relies on cryptographic protocol

---

### Denial of Service
**Threat:** Prevent legitimate transfers.

**Mitigations:**
- ✅ Permissionless (no access control to DOS)
- ✅ Admin can pause (future feature) but not censor specific users
- ✅ Gas costs prevent spam

**Attack Scenarios:**
- Init re-initialization: ⚠️ **Medium severity** (operational, not security)
- Announcer contract failure: ✅ Mitigated (transaction reverts, no partial state)

**Residual Risk:** **Low** - Admin pause is transparent and time-limited

---

### Elevation of Privilege
**Threat:** Gain admin rights to steal funds or censor transfers.

**Mitigations:**
- ✅ Admin role is multisig (3-of-5)
- ✅ Upgrade requires 7-day timelock
- ✅ Admin cannot directly transfer user funds
- ✅ Pause mechanism (if implemented) is emergency-only

**Attack Scenarios:**
- Malicious upgrade: ✅ Mitigated by timelock (users have 7 days to exit)
- Admin key compromise: ✅ Mitigated by multisig threshold

**Residual Risk:** **Medium** - Admin trust required, mitigated by safeguards

---

## 4. wraith-names

### Spoofing
**Threat:** Register name on behalf of another user.

**Mitigations:**
- ✅ `owner.require_auth()` enforced
- ✅ Signature verification for on-behalf operations
- ✅ Replay protection (nonce-based)

**Residual Risk:** **None** - Auth prevents spoofing

---

### Tampering
**Threat:** Alter name resolution or steal names.

**Mitigations:**
- ✅ Only owner can update name
- ✅ Guardians can initiate recovery (with delay)
- ✅ Recovery requires threshold signatures
- ✅ 7-day delay gives owner time to respond

**Attack Scenarios:**
- Guardian collusion: ⚠️ **Medium severity** (mitigated by threshold + delay)
- Signature malleability: ✅ Mitigated (standard Ed25519)

**Residual Risk:** **Medium** - Guardian trust required, mitigated by threshold

---

### Repudiation
**Threat:** Deny name ownership or transfers.

**Mitigations:**
- ✅ All operations emit events
- ✅ Signature provides cryptographic proof
- ✅ On-chain storage provides evidence

**Residual Risk:** **None** - All operations are non-repudiable

---

### Information Disclosure
**Threat:** Link names to stealth addresses.

**Mitigations:**
- ⚠️ **Names and meta-addresses are public by design**
- ✅ No private keys stored
- ✅ Reverse lookup is intentional feature

**Residual Risk:** **Medium** - Privacy model assumes names are public

---

### Denial of Service
**Threat:** Prevent name registration or resolution.

**Mitigations:**
- ✅ Permissionless registration (first-come-first-served)
- ✅ Name uniqueness prevents squatting disputes
- ✅ Resolution is read-only (no DOS vector)

**Attack Scenarios:**
- Name squatting: ⚠️ **Accepted risk** (first-come-first-served model)
- Front-running registration: ⚠️ **Possible** (use private mempool or higher gas)

**Residual Risk:** **Medium** - Name squatting is economic problem, not security

---

### Elevation of Privilege
**Threat:** Gain admin rights or bypass recovery safeguards.

**Mitigations:**
- ✅ Admin role is multisig (3-of-5)
- ✅ Upgrade requires 7-day timelock
- ✅ Admin cannot directly transfer names
- ✅ Recovery delay is hardcoded (100,000 ledgers)

**Attack Scenarios:**
- Malicious upgrade: ✅ Mitigated by timelock
- Guardian threshold bypass: ✅ Prevented by smart contract logic

**Residual Risk:** **Medium** - Admin trust required, mitigated by safeguards

---

## Cross-Contract Attack Scenarios

### Attack 1: Announcer Failure During Transfer
**Threat:** Funds transferred but announcement not emitted (recipient can't detect payment).

**Mitigations:**
- ✅ Atomic execution (transfer + announcement or revert)
- ✅ Test coverage: `test_announcer_panic_reverts_transfer()`

**Residual Risk:** **None** - Atomicity guaranteed by Soroban

---

### Attack 2: Reentrancy via Malicious Token
**Threat:** Token contract re-enters sender during transfer.

**Mitigations:**
- ✅ Soroban prevents reentrancy (single-threaded execution)
- ✅ Test coverage: `test_malicious_token_reentry_attempt()`

**Residual Risk:** **None** - Reentrancy impossible in Soroban

---

### Attack 3: Upgrade to Malicious Logic
**Threat:** Admin upgrades contract to steal funds.

**Mitigations:**
- ✅ 7-day timelock (users can exit)
- ✅ Upgrade events emitted (transparency)
- ✅ Multisig prevents single-point-of-failure

**Residual Risk:** **Medium** - Users must monitor upgrades and exit if malicious

---

### Attack 4: Front-Running Name Registration
**Threat:** Attacker observes pending name registration and front-runs it.

**Mitigations:**
- ⚠️ **Possible attack** (Stellar has public mempool)
- ✅ Use private RPC endpoint
- ✅ Higher fee priority

**Residual Risk:** **Medium** - Economic problem, not security vulnerability

---

### Attack 5: Storage Rent Exhaustion
**Threat:** User data expires due to unpaid storage rent.

**Mitigations:**
- ✅ TTL extension on access
- ✅ Users can pay rent preemptively
- ⚠️ User responsibility to maintain rent

**Residual Risk:** **Low** - User education + keeper bots

---

## Attack Tree Summary

```
┌─ Steal User Funds
│  ├─ Direct Transfer [BLOCKED: No admin fund transfer]
│  ├─ Malicious Upgrade [MITIGATED: 7-day timelock]
│  ├─ Reentrancy [BLOCKED: Soroban prevents]
│  └─ Malicious Token [DOCUMENTED: Caller validates]
│
┌─ Censor Transactions
│  ├─ Frozen Contracts [BLOCKED: No admin]
│  ├─ Upgradeable Contracts [MITIGATED: Transparent pause]
│  └─ Network-Level [OUT OF SCOPE]
│
┌─ Steal Name Ownership
│  ├─ Direct Transfer [BLOCKED: Owner auth required]
│  ├─ Guardian Collusion [MITIGATED: Threshold + delay]
│  └─ Malicious Upgrade [MITIGATED: Timelock]
│
┌─ Break Privacy
│  ├─ Link Sender-Recipient [BLOCKED: Stealth addresses]
│  ├─ Reveal Meta-Addresses [ACCEPTED: Public by design]
│  └─ Timing Analysis [OUT OF SCOPE: Network-level]
│
└─ Denial of Service
   ├─ Spam Transactions [MITIGATED: Gas costs]
   ├─ Storage Exhaustion [MITIGATED: Storage rent]
   └─ Contract Pause [MITIGATED: Time-limited, transparent]
```

## Risk Assessment Matrix

| Threat | Likelihood | Impact | Residual Risk | Mitigation |
|---|---|---|---|---|
| **Fund Theft (Direct)** | Very Low | Critical | **Low** | No admin fund access |
| **Fund Theft (Upgrade)** | Low | Critical | **Medium** | 7-day timelock |
| **Reentrancy** | Very Low | Critical | **None** | Soroban prevents |
| **Malicious Token** | Medium | High | **Low** | Documented, caller validates |
| **Name Theft (Direct)** | Very Low | Medium | **None** | Auth required |
| **Name Theft (Guardians)** | Low | Medium | **Medium** | Threshold + delay |
| **Censor Transactions** | Low | Medium | **Medium** | Transparent pause |
| **Break Privacy** | Medium | Low | **Low** | Cryptographic protocol |
| **DOS (Spam)** | Medium | Low | **Low** | Gas costs |
| **DOS (Storage)** | Low | Low | **Low** | Storage rent |
| **Front-Run Names** | Medium | Low | **Medium** | Economic problem |

## Recommendations

### For Deployment

1. **Monitor Upgrade Proposals**
   - Users should subscribe to upgrade events
   - Community should review upgrade code during 7-day timelock

2. **Validate Token Addresses**
   - UI should whitelist known tokens
   - SDK should provide token validation helpers

3. **Guardian Selection**
   - Choose geographically distributed guardians
   - Use hardware security modules (HSMs) for guardian keys

4. **Storage Rent Monitoring**
   - Deploy keeper bots to extend TTL for critical data
   - UI should alert users when rent is due

### For Future Audits

1. **Admin Infrastructure**
   - Audit multisig implementation
   - Audit timelock mechanism
   - Verify upgrade event emission

2. **Pause Mechanism**
   - Audit pause/unpause logic
   - Verify time limits enforced
   - Test emergency response procedures

3. **Economic Analysis**
   - Game-theoretic analysis of name squatting
   - Storage rent sustainability modeling

## Conclusion

The Wraith Protocol contracts exhibit a strong security posture with multiple layers of defense:

1. **Soroban Runtime:** Prevents entire classes of vulnerabilities (reentrancy, integer overflow, memory safety)
2. **Authorization Framework:** Enforces user consent for all sensitive operations
3. **Governance Model:** Balances immutability (frozen contracts) with upgradeability (timelock + multisig)
4. **Atomic Transactions:** Ensures consistency (transfer + announcement)

**Primary Risks:**
- Malicious upgrades (mitigated by timelock)
- Guardian collusion (mitigated by threshold + delay)
- Malicious tokens (documented, user responsibility)

**Residual Risk Level:** **Medium** - Acceptable for mainnet with proper monitoring and user education.

---

**Last Updated:** 2026-06-26  
**Threat Model Version:** 1.0.0  
**Next Review:** After Phase 2 implementation (admin infrastructure)
