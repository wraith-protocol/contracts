# Stellar Contract Upgrade Governance & Migration Plan

This document outlines the strategy for upgrading Wraith Protocol smart contracts on the Stellar network, ensuring security, transparency, and minimal disruption to users.

## 1. Governance Model

### 1.1 Administrative Authority
All upgradeable contracts (currently `stealth-sender`) will have an `admin` address. In the initial phase, this will be a multi-sig account managed by the core team.

### 1.2 Upgrade Mechanism
Soroban contracts can be upgraded by replacing the contract's WASM hash.
- **Action**: `env.deployer().update_current_contract_wasm(new_wasm_hash)`
- **Restriction**: This action MUST be restricted to the `admin` account via `admin.require_auth()`.

### 1.3 Governance Transition
As the protocol matures, the `admin` role will transition from a team multi-sig to a Decentralized Autonomous Organization (DAO) or a time-locked governance contract.

## 2. Upgrade Procedure

### 2.1 Preparation
1. **Security Audit**: Any new WASM code must undergo a security audit.
2. **Testnet Validation**: Deploy and test the new WASM on `testnet` and `futurenet`.
3. **WASM Upload**: Upload the new WASM to the network to obtain the new WASM hash.

### 2.2 Execution
1. **Announcement**: Notify the community via official channels at least 72 hours before execution (on mainnet).
2. **Invoke Upgrade**: The `admin` invokes the `upgrade` function on the contract with the new WASM hash.
3. **Verification**: Verify the upgrade by checking the contract's WASM hash on-chain.

## 3. Migration Plan (v1 to v2)

### 3.1 Registry Data
The `stealth-registry` uses `persistent` storage. Upgrading the contract logic will NOT affect existing registrations as long as the `DataKey` structure remains compatible.

### 3.2 Sender State
The `stealth-sender` stores the `announcer` contract ID. An upgrade function should allow the `admin` to update this ID if the announcer is ever replaced.

### 3.3 Emergency Pause
A circuit breaker (pause) mechanism should be implemented in the `stealth-sender` to allow the `admin` to stop new transactions in case of a critical vulnerability discovery.

## 4. Implementation Checklist for v2
- [ ] Add `upgrade(new_wasm_hash)` function to all core contracts.
- [ ] Implement `pause()` and `unpause()` in `stealth-sender`.
- [ ] Add `set_admin(new_admin)` for governance handovers.
- [ ] Ensure all state-changing administrative functions emit `GovernanceEvent`.
