# Stellar Contract Upgrade Governance & Migration Plan

This document outlines the strategy for upgrading Wraith Protocol smart contracts on the Stellar network, ensuring security, transparency, and minimal disruption to users.

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

### 4. `wraith-names`
- **Decision**: Timelock + Multisig Upgradable (eventually renounceable).
- **Reasoning**: Handles naming logic and resolution. We may need to add complex resolution upgrades or support new scheme IDs. Once fully matured, we can renounce the upgrade capability.

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
| Timelock + Multisig (Sender, Names) | Proxy Admin with TimelockController |
| Scheme ID Bumps | Contract Registry Versioning |
| Pause Mechanism | `Pausable` (OpenZeppelin) |

## Implementation Checklist
The following should be filed as follow-up issues with the `Stellar Wave` label:
- [ ] Create issue: "Implement 3-of-5 multisig on Stellar for mainnet admin"
- [ ] Create issue: "Implement 7-day Timelock for upgrades in `stealth-sender` and `wraith-names`"
- [ ] Create issue: "Implement Pause mechanism in `stealth-sender`"
- [ ] Create issue: "Ensure `stealth-announcer` and `stealth-registry` have no upgrade paths (Frozen)"
