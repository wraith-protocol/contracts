# Wraith Protocol Stellar Contracts - Third-Party Audit Package

## Overview

This directory contains the complete audit preparation package for Wraith Protocol's Stellar smart contracts. The package is designed for independent third-party security auditors and includes all necessary documentation, test coverage, threat models, and prior audit reports.

**Target Audience:** External security audit firms  
**Prepared:** June 2026  
**Version:** 1.0.0  
**Status:** Ready for distribution

## Package Contents

| Document | Purpose | Location |
|---|---|---|
| **Scope Definition** | In-scope contracts, versions, assumptions | [SCOPE.md](./SCOPE.md) |
| **Threat Model** | STRIDE-based threat analysis | [THREAT_MODEL.md](./THREAT_MODEL.md) |
| **Internal Audits Index** | Summary of prior audits | [INTERNAL_AUDITS.md](./INTERNAL_AUDITS.md) |
| **Test Coverage Report** | Comprehensive test documentation | [TEST_COVERAGE.md](./TEST_COVERAGE.md) |
| **Reproducible Builds** | Build verification instructions | [REPRODUCIBLE_BUILD.md](./REPRODUCIBLE_BUILD.md) |
| **Deployment Manifest** | Planned deployment configuration | [DEPLOYMENT_MANIFEST.md](./DEPLOYMENT_MANIFEST.md) |
| **Audit Firms Shortlist** | Recommended audit providers | [AUDIT_FIRMS.md](./AUDIT_FIRMS.md) |

## Quick Start for Auditors

### 1. Understand the Scope
Start with [SCOPE.md](./SCOPE.md) to understand which contracts are in scope and what assumptions apply.

### 2. Review Threat Model
Read [THREAT_MODEL.md](./THREAT_MODEL.md) to understand the security model and attack vectors we've identified.

### 3. Review Prior Audits
Check [INTERNAL_AUDITS.md](./INTERNAL_AUDITS.md) to see what has already been audited internally and any known issues.

### 4. Examine Test Coverage
Review [TEST_COVERAGE.md](./TEST_COVERAGE.md) to understand our testing strategy and coverage levels.

### 5. Verify Reproducible Builds
Follow [REPRODUCIBLE_BUILD.md](./REPRODUCIBLE_BUILD.md) to verify you can reproduce our contract builds.

### 6. Review Deployment Plan
Read [DEPLOYMENT_MANIFEST.md](./DEPLOYMENT_MANIFEST.md) to understand the planned mainnet deployment configuration.

## Repository Structure

```
contracts/
├── audit-prep/              # THIS DIRECTORY - Complete audit package
│   ├── README.md
│   ├── SCOPE.md
│   ├── THREAT_MODEL.md
│   ├── INTERNAL_AUDITS.md
│   ├── TEST_COVERAGE.md
│   ├── REPRODUCIBLE_BUILD.md
│   ├── DEPLOYMENT_MANIFEST.md
│   └── AUDIT_FIRMS.md
├── stellar/                 # Stellar/Soroban contracts (IN SCOPE)
│   ├── stealth-announcer/   # Event emission contract
│   ├── stealth-registry/    # Meta-address registry
│   ├── stealth-sender/      # Atomic send + announce
│   ├── wraith-names/        # Name resolution
│   ├── stealth-splitter/    # Payment splitting (OPTIONAL)
│   ├── stealth-batch-sender/# Batch operations (OPTIONAL)
│   ├── wraith-metrics/      # Metrics library
│   └── shared/              # Shared utilities
├── evm/                     # EVM contracts (OUT OF SCOPE)
├── solana/                  # Solana contracts (OUT OF SCOPE)
└── ckb/                     # CKB contracts (OUT OF SCOPE)
```

## Key Contracts (In Scope)

### Core Contracts (MUST AUDIT)

1. **stealth-announcer** (~100 LOC)
   - Pure event emitter for stealth address announcements
   - **Status:** Frozen (no upgrade path)
   - **Blast Radius:** Low (no assets, no storage)

2. **stealth-registry** (~150 LOC)
   - Maps addresses to stealth meta-addresses
   - **Status:** Frozen (no upgrade path)
   - **Blast Radius:** Medium (user privacy if compromised)

3. **stealth-sender** (~200 LOC)
   - Atomic token transfer + announcement
   - **Status:** Upgradeable (timelock + multisig)
   - **Blast Radius:** High (handles user assets)

4. **wraith-names** (~400 LOC)
   - Name → meta-address resolution with guardians/recovery
   - **Status:** Upgradeable (timelock + multisig)
   - **Blast Radius:** Medium (name ownership disputes)

### Optional Contracts (SHOULD AUDIT)

5. **stealth-splitter** (~300 LOC)
   - Payment splitting across multiple recipients
   - **Status:** Upgradeable
   - **Blast Radius:** High (handles user assets)

6. **stealth-batch-sender** (~150 LOC)
   - Optimized batch operations
   - **Status:** Upgradeable
   - **Blast Radius:** High (handles user assets)

## Audit Timeline

**Recommended Duration:** 2-3 weeks  
**Delivery Format:** Markdown report following [Smart Contract Security Field Guide](https://scsfg.io/)

### Suggested Milestones

- **Week 1:** Initial review + automated analysis
- **Week 2:** Manual review + test harness development
- **Week 3:** Report writing + remediation review

## Contact Information

**Project:** Wraith Protocol  
**Repository:** https://github.com/wraith-protocol/contracts  
**Security Contact:** security@usewraith.xyz  
**Project Documentation:** https://docs.usewraith.xyz

**Audit Coordinator:** [TBD]  
**Technical Contact:** [TBD]

## Budget Expectations

Based on scope (4 core contracts + 2 optional, ~1,500 LOC total):

- **Small Firm:** $15,000 - $30,000
- **Medium Firm:** $30,000 - $60,000
- **Top-Tier Firm:** $60,000 - $120,000

See [AUDIT_FIRMS.md](./AUDIT_FIRMS.md) for specific quotes.

## Out of Scope

The following are **explicitly out of scope** for this audit:

- EVM contracts (Solidity, separate audit planned)
- Solana contracts (Rust/Anchor, separate audit planned)
- CKB contracts (Rust/RISC-V, experimental)
- Off-chain components (SDK, indexers, wallets)
- Stellar network itself
- Cryptographic primitives (rely on Soroban SDK)

## Acceptance Criteria

For the audit to be considered complete:

- [ ] All 4 core contracts reviewed
- [ ] At least 2 optional contracts reviewed
- [ ] Critical and High findings documented with PoC
- [ ] Medium and Low findings documented
- [ ] Gas optimization opportunities identified
- [ ] Code quality recommendations provided
- [ ] Final report delivered in markdown format
- [ ] Remediation verification (if findings exist)

## Questions?

If you have questions about this audit package:

1. Open an issue: https://github.com/wraith-protocol/contracts/issues
2. Email: security@usewraith.xyz
3. Review FAQ: [SCOPE.md#FAQ](./SCOPE.md#faq)

---

**Last Updated:** 2026-06-26  
**Package Version:** 1.0.0  
**Prepared by:** Wraith Protocol Security Team
