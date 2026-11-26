# Stellar Upgrade and Governance Authorization Tests

This document describes what the current Stellar contract interfaces
authorize. A multisig or timelock is not an upgrade path unless the contract
implements an upgrade entry point and applies that authorization to it.

## Wraith Names

Wraith Names exposes no contract-upgrade or authority-renunciation entry point.
Its on-chain multisig supports signer-set rotation and premium-auction admin
rotation only. The signer set and threshold are configurable through the
permissionless, one-time `init_multisig(signers, threshold)` entry point; the
first caller chooses both. The contract does not enforce the recommended
3-of-5 configuration.

`wraith-names/tests/upgrade_auth.rs` retains its historical filename, but
tests the actual signer-set rotation interface:

- Current signers only may propose, approve, execute, or cancel.
- The proposer is automatically counted once; duplicate approval is rejected.
- Execution requires the current quorum and the seven-day delay from proposal.
- Successful execution replaces the signer set and threshold, emits
  `SignersRotated`, and consumes the proposal.
- Cancellation clears the proposal and approvals; execution after cancellation
  and replay after successful execution are rejected.

There is no supported renunciation path to test because no upgrade authority
exists. These tests make no claim that Wraith Names can be upgraded or frozen
by renouncing an admin.

Run the focused tests:

```bash
cd stellar
cargo test -p wraith-names --test upgrade_auth
```

The full Wraith Names suite can be run with:

```bash
cargo test -p wraith-names
```

See [GOVERNANCE.md](./GOVERNANCE.md), [MULTISIG.md](./MULTISIG.md), and the
[authorization matrix](../AUTHORIZATION_MATRIX.md) for the corresponding
authority lifecycle and deployment caveat.
