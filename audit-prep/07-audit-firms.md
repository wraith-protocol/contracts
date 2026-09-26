# Audit Firm Shortlist and Outreach Tracker

## Shortlist

The firms below are reasonable candidates for a Stellar/Soroban smart contract audit. Final selection should prioritize demonstrated Rust/Soroban experience, availability, ability to review privacy assumptions, and willingness to validate reproducible builds.

| Firm | Why consider | Suggested ask | Budgetary quote estimate |
| --- | --- | --- | --- |
| OtterSec | Strong Rust and non-EVM smart contract audit background; known for low-level protocol/security reviews. | Full audit of four in-scope Stellar contracts plus build attestation review. | USD 35k-90k depending on depth and schedule. |
| Zellic | Smart contract and cryptography-oriented reviews; suitable for authorization, signature, and privacy-adjacent logic. | Full code review with focused pass on `wraith-names` signatures/replay and sender asset policy. | USD 30k-80k. |
| Halborn | Broad blockchain audit practice with operational readiness and incident-response experience. | Mainnet-readiness audit covering contracts, governance, and deployment process. | USD 40k-100k. |
| Trail of Bits | Deep security engineering and formal review capability; strong fit for high-assurance Rust review if available. | Targeted high-assurance review of threat model, contracts, and reproducible build pipeline. | USD 75k-175k. |
| OpenZeppelin Security | Mature smart contract audit process and governance/security documentation review. | Independent contract and governance process audit, with Stellar support to be confirmed. | USD 50k-125k. |

Estimates are planning ranges only. Actual quotes depend on final LOC, audit duration, urgency, number of auditors, and whether remediation review is included.

## Outreach package

Send each firm:

- Link to repository and audit branch.
- This `audit-prep/` directory.
- `stellar/MAINNET_READINESS.md`.
- `stellar/GOVERNANCE.md`.
- All internal audit reports indexed in `03-internal-audits.md`.
- Requested timeline and desired start date.
- Request for initial quote, availability, methodology, deliverables, and remediation-review terms.

## Contact log

Maintainers should complete this table when outreach is sent. The acceptance criterion for issue #62 requires at least three firms to be contacted with the pack and quote estimates documented.

| Firm | Contact | Date sent | Status | Quote/estimate | Notes |
| --- | --- | --- | --- | --- | --- |
| OtterSec | TBD | Pending | Not sent | Planning range: USD 35k-90k | Send pack after maintainer threat-model review. |
| Zellic | TBD | Pending | Not sent | Planning range: USD 30k-80k | Send pack after maintainer threat-model review. |
| Halborn | TBD | Pending | Not sent | Planning range: USD 40k-100k | Send pack after maintainer threat-model review. |
| Trail of Bits | TBD | Pending | Not sent | Planning range: USD 75k-175k | Optional high-assurance candidate. |
| OpenZeppelin Security | TBD | Pending | Not sent | Planning range: USD 50k-125k | Confirm Stellar/Soroban availability. |

## Outreach email template

Subject: Wraith Protocol Stellar contracts audit request

Hello,

Wraith Protocol is preparing for mainnet deployment of its Stellar stealth-address contracts and is seeking an independent third-party audit.

Scope:

- `stellar/stealth-announcer`
- `stellar/stealth-registry`
- `stellar/stealth-sender`
- `stellar/wraith-names`
- Supporting tests, governance assumptions, SAC compatibility, and reproducible build attestation

We have assembled an audit pack in `audit-prep/` with scope, threat model, prior internal audits, coverage commands, deployment manifest, and reproducible build instructions.

Could you share:

- Earliest start date and expected duration
- Proposed methodology and deliverables
- Quote range and whether remediation review is included
- Relevant Stellar/Soroban or Rust smart contract audit experience
- Any pre-audit changes you recommend before scheduling

Thank you.
