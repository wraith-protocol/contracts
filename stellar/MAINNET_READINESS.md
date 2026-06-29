# Stellar Mainnet Readiness Checklist

This document tracks the requirements and status for launching the Stellar Smart Contracts on mainnet. This is a living document and serves as the source of truth for the go/no-go decision.

## Code Quality

- [ ] **Contract Audits**: All four contract audits completed with zero unresolved Critical or High findings.
  - [Audit #01](#01)
  - [Audit #02](#02)
  - [Audit #03](#03)
  - [Audit #04](#04)
- [ ] **Property-based Fuzz Tests**: Fuzz tests running nightly with no failures for 30 days. [#05](#05)
- [ ] **Wasm Size Budget**: Size budget under threshold for every contract. [#17](#17)
- [ ] **SAC Compatibility Matrix**: Signed off for current protocol version. [#15](#15)
- [ ] **Reproducible Build Attestation**: CI pipeline verified and reproducible. [#18](#18)

---

## Operations

- [x] **Deployment Script**: Tested on Futurenet; dry-run performed for Mainnet. [#07](#07) — completed in [#54](https://github.com/wraith-protocol/contracts/issues/54); see `stellar/deployments/futurenet.json` and `stellar/DEPLOYMENT.md`.
- [ ] **Upgrade Governance**: Decision finalized and signers configured in technical controls. [#11](#11)
- [ ] **Multisig Hardware Setup**: Verified by independent third party.
- [ ] **Incident Response Runbook**: Documented and distributed to on-call team.
- [ ] **On-call Rotation**: Defined and active.

---

## Ecosystem

- [ ] **SDK Stability**: Tagged at semver-stable release.
- [ ] **Documentation**: Docs reflect mainnet contract addresses and latest interface.
- [ ] **Demo UI**: Deployable against mainnet (toggle enabled on Stellar wallet).
- [ ] **Spectre Agent**: Verified end-to-end on a small mainnet test transaction.

---

## Communication

- [ ] **Blog Post**: Preview published to stakeholders.
- [ ] **Status Page**: Monitoring mainnet endpoints and contract health.
- [ ] **Security Contact**: `security@usewraith.xyz` email actively monitored.
- [ ] **Bounty Program**: Live with payment infrastructure ready.

---

## Risk Acknowledgments

- **Known Limitations of v1**:
  - Social recovery (Postponed for v1.1) - Rationale: Prioritizing core asset safety over complex recovery UX.
  - Vault contract refinements - Rationale: Current implementation meets security requirements; gas optimizations planned for v2.

---

## Sign-off

- [ ] **Maintainer Consensus**: Recorded in internal governance repo.
- [ ] **Go/No-Go Meeting**: Date-stamped meeting minutes committed.

---

*Last Updated: 2026-05-28*
