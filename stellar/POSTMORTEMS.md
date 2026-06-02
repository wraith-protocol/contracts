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
to derive the stealth address. The tool never requests the sender's long-term
spending key — only the ephemeral key material.