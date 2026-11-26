# Stellar Contract Governance & Migration Plan

This document records the current authorization model and proposed governance
configuration for Wraith Protocol smart contracts on Stellar. A governance
mechanism is not an upgrade path unless the contract exposes and authorizes an
upgrade entry point.

## Mainnet Initial Configuration Recommendation
If we have to ship mainnet next week, the governance configuration will be:
- **Admin**: 3-of-5 Multisig managed by a Security Council.
- **Named Guardians**: @truthixify, @thebabalola, @bbkenny, @richiey1, and @drips-wave.
- **Time-lock Duration**: 7 days for any contract upgrades (for upgradable contracts).

## Per-Contract Upgradability Decisions

### 1. `stealth-announcer`
- **Decision**: Frozen (No upgrade path).
- **Reasoning**: This is a simple, trust-minimizing contract that merely emits events. To build trust, the most-watched and foundational contract should be immutable.

### 2. `stealth-registry`
- **Decision**: Frozen (No upgrade path).
- **Reasoning**: It holds the user's meta-address mapping and scheme keys. Keeping this frozen ensures users that their keys cannot be arbitrarily altered or censored.

### 3. `stealth-sender`
- **Decision**: Timelock + Multisig Upgradable.
- **Reasoning**: This contract holds complex logic to process stealth payments. If a critical bug is found, the admin needs a path to upgrade the logic to prevent loss of funds, but with a 7-day timelock so users have a chance to review the upgrade.
- **Protocol Fee Governance**: The optional protocol fee (`fee_recipient` and `fee_basis_points`) is set at initialization. The fee is capped at 50 bps (0.5%) by contract invariant to prevent exorbitant charges. Deploying fee configuration changes requires contract upgrades controlled by the multisig governance.

### 4. `wraith-names`
- **Current implementation**: No contract-upgrade or authority-renunciation
  entry point. The contract is not upgradeable through its interface.
- **Governance that exists**: A configurable on-chain signer set and quorum
  protect signer-set rotation and premium-auction admin rotation. Both use a
  7-day timelock. This governance does not authorize WASM upgrades and does not
  control the separate pause admin.
- **Initialization caveat**: `init_multisig(signers, threshold)` is a
  permissionless, one-time initializer; its first caller chooses the signer
  set and threshold. Deployment must initialize it before relying on that
  governance set. The implementation does not enforce the recommended 3-of-5
  configuration.

## Versioning & Migration
When the protocol needs breaking changes or new data shapes that an upgrade cannot support directly:
- **Convention**: We use `scheme_id` bumps rather than just upgrading contract logic or redeploying for everything.
- **Example**: The event-topic redesign for the Stellar announcer (#26) will use `scheme_id=2`. Old integrations will continue to listen to `scheme_id=1`, while new ones will use `scheme_id=2`. New scheme IDs can be deployed and registered without breaking existing users.

## Pause Mechanism Trade-off
Implementing a "Pause" (circuit breaker) mechanism presents a fundamental trade-off:
- **Pause Capability**: Introduces a centralization vector where the multi-sig could censor or halt transactions.
- **No Pause Capability**: A zero-day exploit in the `stealth-sender` could be catastrophic, allowing attackers to exploit the protocol before an upgrade can deploy.

**Decision: Implement Pause for `stealth-sender`**
For an early-stage protocol, "no pause" is a reckless vector. We will implement a pause functionality controlled by the 3-of-5 multisig to halt sends during an emergency. To mitigate censorship risks, the `stealth-registry` and pure event emission will remain unpauseable.

## EVM Comparison
To maintain ecosystem consistency, here is how our Stellar decisions map to their EVM equivalents:

| Stellar Governance Decision | EVM Equivalent / Concept |
| --- | --- |
| Admin as 3-of-5 Multisig | Gnosis Safe (Safe{Wallet}) |
| Frozen Contracts (Announcer, Registry) | Immutable Contracts (No Proxy) |
| Timelock + Multisig (Sender only) | Proxy Admin with TimelockController |
| Scheme ID Bumps | Contract Registry Versioning |
| Pause Mechanism | `Pausable` (OpenZeppelin) |

## Wraith Names governance tests

`stellar/wraith-names/tests/upgrade_auth.rs` covers the implemented signer-set
rotation lifecycle, despite the historical test filename. It exercises
proposal, distinct-signer quorum, the 7-day delay, execution, cancellation,
duplicate-approval and post-execution replay rejection, unauthorized callers,
and signer replacement.

Wraith Names has no upgrade or renunciation entry point, so there is no
upgrade proposal to execute and no upgrade authority to renounce. The tests do
not simulate either behavior. The on-chain signer set is configurable; 3-of-5
is a deployment recommendation, not an enforced contract default.

Run the focused suite:

```bash
cd stellar
cargo test -p wraith-names --test upgrade_auth
```

The workspace test suite also runs these tests in CI.

---

## On-Chain Governance PoC (`contracts/governance/`)

**Status: PROOF OF CONCEPT — NOT PRODUCTION READY**

`contracts/governance/` implements a minimal on-chain governance flow (propose → vote → execute) gated by token-weighted quorum. It was built to validate the architectural shape before committing the roadmap to replacing the multisig upgrade authority.

### How it works

1. **Init** — deployer sets the voting token address, absolute quorum threshold, voting period (ledgers), and timelock delay (ledgers).
2. **Propose** — any token holder creates a proposal specifying a target contract, function symbol, and raw argument bytes.
3. **Vote** — token holders vote for or against. Weight = token balance at vote time. One vote per address per proposal.
4. **Execute** — after the voting window closes and the timelock elapses, anyone may execute if:
   - Total votes cast ≥ quorum
   - `for_votes > against_votes`
5. **Cancel** — admin can cancel anytime; anyone can cancel a failed-quorum proposal after voting ends.

### Design decisions

| Decision | Choice | Rationale |
|---|---|---|
| **Quorum model** | Absolute token threshold | Simpler to reason about than percentage-based; avoids supply-dependent edge cases. The PoC uses a fixed `i128`. |
| **Voting weight** | Snapshot at vote time | No delegation, no checkpointing. A voter's balance is read when `vote()` is called. This is the simplest correct approach for a PoC. |
| **Proposal args** | Raw `Bytes` forwarded to target | Keeps the governance contract agnostic to the target function signature. The execution call passes the bytes as a single argument — the target must accept `Bytes`. A production system would need proper ABI encoding/decoding. |
| **Timelock** | Ledger-count delay after voting ends | Prevents instant execution; gives token holders time to exit if they disagree with a passed proposal. |
| **Admin role** | Hardcoded single address | Deliberately centralised for the PoC. A production system must either remove this role or make it a governance-controlled multisig. |
| **Event scheme** | `(action, proposal_id)` as topics | Matches the Wraith Stellar event convention. |

### Design decisions rejected

| Rejected | Why |
|---|---|
| **Percentage-based quorum** | Adds `total_supply` dependency; the token contract may not expose supply in a cheap way. Absolute threshold is simpler and sufficient for the PoC. |
| **Vote delegation** | Adds complexity (delegation chain, delegation expiry) with no immediate benefit for proving the flow shape. Can be added later. |
| **Multiple proposal types** | The PoC only supports `target.function(args)`. In production we'd want typed actions (upgrade contract, set fee, pause, etc.) with domain-specific validation. |
| **On-chain discussion / rationale** | Out of scope — off-chain forums are a better fit for discussion. |
| **Quadratic voting** | Interesting but overengineered for a PoC. Token-weighted voting is the simplest baseline. |
| **ZKP-based private voting** | Desirable for privacy, but orthogonal to proving the governance flow. |
| **Governor Bravo fork** | The EVM Governor pattern (Compound/Zora) is battle-tested but relies on EVM-specific primitives (delegatecall, storage slots). The Soroban equivalent would look very different. We chose a native Soroban design from scratch. |

### Known limitations (PoC)

- **Admin is a single address** — the very centralisation this PoC aims to eventually replace. The contract's `cancel` function gives the admin emergency override even for passed proposals.
- **No vote-weight checkpointing** — a voter can transfer tokens after voting and still have their original weight counted. A production system would snapshot at proposal start.
- **No proposal cancellation by proposer** — only admin can cancel during voting. The proposer should be able to withdraw their own proposal.
- **Raw `Bytes` arguments** — the execution call is a single-argument `invoke_contract` with no type safety. A production system would use typed action structs.
- **No minimum proposal threshold** — anyone can propose. In production, a token minimum (e.g. 1% of quorum) prevents spam.
- **No upgrade authority integration** — this contract does not hold upgrade keys for `stealth-sender`, `wraith-names`, or any other contract. Any such integration remains future work; Wraith Names currently has no contract-upgrade entry point.
- **No emergency pause** — the PoC has no circuit breaker. Production governance should include a pausable module.
- **Single token** — only one voting token is supported. Production may need multi-token or NFT-weighted voting.

### Path to production

1. Remove the single-admin role and replace with governance-controlled upgrade authority.
2. Snapshot voting weights at proposal creation time.
3. Add typed action structs with domain-specific validation for each contract.
4. Add minimum proposal threshold.
5. Integrate with the `pausable` module.
6. Add support for multiple voting tokens / delegation.
7. Formal audit before mainnet deployment.

### Running the PoC tests

```bash
cd stellar
cargo test --package governance
```

## Implementation Checklist
The following should be filed as follow-up issues with the `Stellar Wave` label:
- [ ] Create issue: "Implement 3-of-5 multisig on Stellar for mainnet admin"
- [ ] Create issue: "Implement 7-day Timelock for upgrades in `stealth-sender` and `wraith-names`"
- [ ] Create issue: "Implement Pause mechanism in `stealth-sender`"
- [ ] Create issue: "Ensure `stealth-announcer` and `stealth-registry` have no upgrade paths (Frozen)"
- [x] Create issue #57: "Upgrade authority enforcement test suite" ✅ COMPLETED
