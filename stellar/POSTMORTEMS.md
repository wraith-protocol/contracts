# Stellar Contract Postmortems

## PM-001: Atomicity of `stealth-sender::send`

**Date:** 2026-05-27  
**Auditor:** Wraith Protocol Team  
**Related Issue:** [#03 - Stealth Sender Atomicity & Rescue Mechanism](https://www.drips.network/wave/contributors/issues/03-audit-stealth-sender.md)

### Summary

Audited `stealth-sender::send` and `stealth-sender::batch_send` for atomicity —
whether a token transfer can succeed without a matching on-chain announcement,
leaving funds at a stealth address that the recipient cannot discover.

### Finding: ✅ Atomicity Holds

After thorough analysis of the Soroban execution model and the current contract
implementation, **no atomicity gap exists today.** The contract is safe.

#### How Soroban Guarantees Atomicity

In Soroban, a single contract invocation (plus all its sub-contract calls) forms
an atomic unit. Every cross-contract call made via `env.invoke_contract` executes
within the same transaction context. If **any** sub-call panics, **all** state
changes across the entire invocation are rolled back.

The `send` function's execution path:

```
send()
  ├── sender.require_auth()          ← auth check (panics if unauthorized)
  ├── storage.get(Announcer)         ← reads announcer address (panics if uninit)
  ├── token_client.transfer()        ← sub-contract call to SAC token
  │   └── On failure: panic → entire send() reverts → transfer rolled back
  └── announcer_client::announce()   ← sub-contract call to announcer
      └── On failure: panic → entire send() reverts → transfer rolled back
```

Both the token transfer and the announcement execute as sub-contract invocations
within the same Soroban host call. If either fails, the whole invocation reverts.

#### Edge Cases Considered

| Edge Case | Analysis | Verdict |
|---|---|---|
| **Malicious token contract** | Token contract panics after debiting but before crediting | Soroban rollback covers both; no partial state |
| **Announcer contract bug** | Announcer's `announce` panics (e.g., out of gas, storage error) | Event emission in announcer cannot panic; it has no storage and no branching |
| **Wrong announcer address** | `env.invoke_contract` with non-existent address | Panics → entire send reverts |
| **`batch_send` partial failure** | Transfer `i` succeeds, announce `i` fails | Soroban rolls back all transfers in the batch |
| **External direct transfer** | User sends tokens directly to stealth address without calling `send()` | Not a contract bug; this is what the rescue tool (PM-001/R) addresses |
| **Chain reorg** | Stellar ledger reorg drops the announcement tx but keeps the transfer | Theoretical; Stellar consensus makes deep reorgs extremely unlikely. Rescue tool covers this. |

### No Code Change Required

Because atomicity holds, we do **not** modify the `stealth-sender` contract.
The invariant is documented here and referenced from `MAINNET_READINESS.md`.

### Related Rescue Mechanism

See the companion rescue tool `scripts/rescue-stealth-funds.ts` and its
documentation in `scripts/README.md`, which covers the hypothetical cases
where announcements are missing despite successful transfers (operator error,
external direct sends, or edge-case chain reorgs).

---

## PM-001/R: Rescue Tool Design Rationale

While the contract is atomic, funds can still land at a stealth address without
an announcement through non-contract paths:

1. **Direct external transfer:** Someone sends tokens to a stealth address
   without using `stealth-sender::send`.
2. **Operator error:** A UI or script sends the transfer but fails to call
   `send()` on the sender contract.
3. **Chain reorg (theoretical):** A deep Stellar ledger reorg drops the
   announcement transaction but keeps the transfer (extremely unlikely with
   Stellar consensus, but formally possible).

The rescue tool (`scripts/rescue-stealth-funds.ts`) addresses these scenarios.
It does **not** modify any contract or require a contract upgrade.

**Trust model:** The sender must still possess the ephemeral private key used
spending key — only the ephemeral key material.

---

## PM-002: Reproducible Build Workflow Rot

**Date:** 2026-07-24  
**Auditor:** Wraith Protocol Team  
**Related Commits:** `35bf3fc`, `a319969`, `9abf33a`, `b621b46`

### Summary
The reproducible-build verification CI pipeline failed due to bit-rot in our Dockerfile and toolchain pins. Specifically, a base image digest rotation for `bookworm-slim`, coupled with Rust MSRV (Minimum Supported Rust Version) creep for Edition 2024 dependencies in the `soroban-sdk`, broke the reproducible environment.

### Timeline & Root Cause
1. **Rust MSRV Creep:** Dependencies in the Soroban SDK ecosystem bumped their MSRV to 1.86.0 to support Rust 2024 edition features, breaking our pinned older Rust version in the reproducible build container.
2. **Base Image Digest Rotation:** The `debian:bookworm-slim` base image digest was hard-pinned. Upstream registries rotated/pruned the old digest, preventing the Docker build from pulling the base layer.
3. **CI Breakage:** The combination of these issues caused the reproducible-build verification step to fail on all new PRs.

### Contributing Factors
- **Strict Pinning:** Pinning by digest instead of tag for the OS base image provided security but introduced fragility when upstream registries pruned old digests.
- **Uncoupled Toolchains:** The reproducible build Dockerfile used a hardcoded Rust version instead of reading from a centralized source of truth, meaning it drifted out of sync.

### Fix
- Unpinned the `bookworm-slim` digest and switched to a stable tag (`35bf3fc`).
- Bumped the pinned Rust toolchain to `1.86.0` to support the updated dependencies (`a319969`).
- Corrected volume mount and output paths in the Dockerfile (`b621b46`).
- Made the reproducible-build verification non-blocking temporarily while resolving the issue to unblock developers (`9abf33a`).

### Prevention (Next-Time)
- **Centralized Toolchain Config:** The reproducible build container should read the Rust version directly from the project's `rust-toolchain.toml` rather than hardcoding it in the Dockerfile.
- **Automated Base Image Updates:** Use Dependabot or Renovate to automatically open PRs when base image digests are rotated, ensuring we test and bump them before they disappear.
- **Scheduled CI Runs:** Run the reproducible-build pipeline on a nightly cron schedule. This would have caught the base image digest rot immediately, rather than surprising a developer on an unrelated feature PR.