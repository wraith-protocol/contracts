# Mainnet Readiness Checklist — Stellar (Soroban)

> **Related issues:**
> - [#03](https://www.drips.network/wave/contributors/issues/03-audit-stealth-sender.md) — Stealth sender atomicity audit & rescue mechanism
> - [#20](https://www.drips.network/wave/contributors/issues/20-mainnet-readiness.md) — This document

## 1. Contract Audit Status

### stealth-sender Atomicity

| Item | Status | Reference |
|---|---|---|
| `send()` atomicity audit | ✅ Passed — no gap found | [POSTMORTEMS.md](./contracts/stellar/POSTMORTEMS.md#pm-001-atomicity-of-stealth-sendersend) |
| `batch_send()` atomicity audit | ✅ Passed — no gap found | [POSTMORTEMS.md](./contracts/stellar/POSTMORTEMS.md#pm-001-atomicity-of-stealth-sendersend) |
| Rescue tool available | ✅ Built | [scripts/rescue-stealth-funds.ts](./scripts/rescue-stealth-funds.ts) |

### Verified Invariants

1. `stealth-sender::send` is atomic under the Soroban execution model.
2. No code change to `stealth-sender` was required.
3. The rescue tool provides a safety net for non-contract failure modes (direct
   external transfers, operator error, theoretical chain reorg).

## 2. Contract Deployment Checklist

- [ ] `stealth-announcer` deployed and address recorded
- [ ] `stealth-registry` deployed and address recorded
- [ ] `stealth-sender` deployed, `init()` called with announcer address
- [ ] `wraith-names` deployed and address recorded
- [ ] All contract addresses published in README

## 3. Pre-Mainnet Verification

- [ ] `cargo test --workspace` passes for all Stellar contracts
- [ ] Rescue tool tested against a fixture stealth address
- [ ] All four contracts compile with `soroban-sdk` 22.0.0

## 4. Monitoring & Incident Response

| Scenario | Response |
|---|---|
| Missing announcement detected | Run `scripts/rescue-stealth-funds.ts` with sender's ephemeral key |
| Suspected atomicity breach | Investigate immediately; escalate to protocol team |
| Announcement event indexer down | Announcements are on-chain events; indexer can catch up from genesis |