# Operator Recovery Drill: 2026-09-29

**Issue:** [wraith-protocol/contracts#199](https://github.com/wraith-protocol/contracts/issues/199), Automate an operator recovery drill
**Owner:** Jerry ([@Jerryvic911](https://github.com/Jerryvic911))
**Network:** Stellar Futurenet ("Test SDF Future Network ; October 2022")
**Funds at risk:** None. All accounts and contracts are drill-only; every account is funded with test XLM (friendbot, or from the drill deployer).
**Reproduce:** `cd stellar && ./scripts/drill-operator-recovery.sh` (see [`docs/RECOVERY_DRILL.md`](../docs/RECOVERY_DRILL.md))

## Summary

This drill exercises the incident paths described in `stellar/PAUSE.md` and
`stellar/MULTISIG.md` together as one scripted procedure against a live
futurenet deployment: simulated operator key loss, pause, signer rotation,
**a real funds rescue**, and recovery. Raw output is in
`drills/2026-09-29T22-23-03Z-operator-recovery.log`.

**Update after maintainer review.** The first version of the rescue phase only
submitted an `announce` call, which emits an event and moves no funds, and the
standalone rescue tool was still broken. The rescue phase now runs the fixed
`scripts/rescue-stealth-funds.ts` itself and proves recovery of funds:

1. A real stealth account is funded with no announcement, so the payment is stuck.
2. The rescue tool checks the real balance and publishes the announcement
   through a real Soroban transaction.
3. The recipient finds the announcement through RPC `getEvents`, rederives the
   private key, and sweeps the funds to a destination account.
4. The script asserts balances before and after, and aborts the whole drill if
   any assertion fails.

**Phase numbering.** This report uses the script's numbering (Phase 1 to 6). An
earlier version of this report numbered the rescue as phase 6; it is now Phase 5
and recovery (unpause) is Phase 6.

## Deployment (this run)

| Contract | Contract ID |
|---|---|
| `stealth-announcer` | `CBJACIKMXNCZAJMMQYOQZVUX2B2XO6QGCK5FHHZ3NZ36O2LHBEGLQ7UU` |
| `stealth-sender` | `CAT57VVRLAURHMH4SYSGA33PRUPSPTTGZBCXX3WYAE4P5YEHQOCEBBDB` |

`stealth-sender` is built with the `drill-timelock` Cargo feature
(`ROTATION_TIMELOCK_SECS = 60` instead of 7 days) so rotation can be rehearsed in
one sitting. **This feature must never ship in a production build.**

Initial governance signer set is 3-of-5. `drill-deployer` is also the pause-admin.

| Alias | Address | Role |
|---|---|---|
| `drill-deployer` | `GBPV7NHTH2DJHQAEAXKSUJS3YK5V4OJRFWDQU5NSHVIWUWMXW2RJMTVA` | Pause-admin and signer |
| `drill-signer-1` | `GAGEHMDYONXQBDRN2HSAVM76277H3CQRGEP2LMAZ6WDD3D37Y44FQJGX` | Signer |
| `drill-signer-2` | `GD44S76CCB776TAPJZIRNXENR3NOTCXO42V6T47NHFHNY3Y7H25X7GVP` | Signer |
| `drill-signer-3` | `GAYDRMOO6ADFKHVMTYGFE36QY36JXOQGJM7ADV32LFVKJ2JMZ6CHSRMM` | Signer (not needed for quorum) |
| `drill-signer-4` | `GCEOH5QQI3GN7SCZM52NJJYDLWQIELWCRFDFAQHBXREIKRPJDXPDSGCQ` | Treated as lost or compromised; rotated out |
| `drill-signer-5-new` | `GDC4C5OP74AV4KZCPJJ4UT5OVWG6JYPPFAZD3EBJYVGBPW57L6RX4DYJ` | Rotated in |

## Timeline and evidence (all times UTC, 2026-09-29)

| Phase | Step | Start | End | Duration | Evidence |
|---|---|---|---|---|---|
| 1 | Simulate operator key loss | 22:24:20 | 22:24:20 | n/a | Declared in the script; no on-chain action. Marks incident start. |
| 2 | **Pause** | 22:24:20 | 22:24:34 | 14s | Tx `9751d3a0d6974cac3126464328959ff047e7b39f19d54e2630cef946301f10ca`; event `paused`; `is_paused` = `true` |
| 3 | **Propose** rotation | 22:24:34 | 22:24:40 | 6s | Tx `0fdb9e23902953ab755bfdbc68554706b4484f597a14e7f04216d9f77a1ac985` |
| 3 | **Approve** x2 (quorum) | 22:24:40 | 22:25:04 | 24s | Txs `61db8ab101ecaf503331c691ebca4ca689c11be3c3f5ee19f1bab53ec064fcf2`, `cf4e774cfd10f6ad534d4fbb94e854c21673f05b3996923c3d23f1fc8bf4e26a`; `pending_rotation` shows 3 approvals, threshold 3 |
| 4 | Wait for timelock (60s drill value) | 22:25:05 | 22:25:42 | 37s | Script sleeps until `executable_at` |
| 4 | **Execute** rotation | 22:25:42 | 22:25:52 | 10s | Tx `a2aedf08a1103e389d59a09f29fc04de2216c2e578d579ab0a349aaca0ca3c58`; event `SignersRotated`; `signers` no longer contains `drill-signer-4` |
| 5 | **Rescue** (real funds, on-chain) | 22:25:52 | 22:26:21 | 29s | See "Rescue phase" below |
| 6 | **Unpause** / recovery | 22:26:21 | 22:26:33 | 12s | Tx `5593c2402cb4d59ccd911d2684a1f55a58be700356ab8f44436e424bdc972c8b`; event `unpaused`; `is_paused` = `false` |

Setup transactions (not part of recovery time): `init`
`2ef31d69974facfc055e4be9feceb02e08c9a1c76ee2bcc923a612c62c1c32d9`,
`init_multisig`
`96a4c448bf471dc72e70dc60d117cbfc303e9a308da9b2e508622aef772b2dc6`.

**Recovery time**

- Pause confirmed **14 seconds** after incident start.
- Compromised signer removed **1m32s** after incident start (22:24:20 to 22:25:52).
- Funds rescued and swept in **17.1 seconds** (rescue phase, measured by the script).
- Fully recovered and unpaused **2m13s** after incident start (22:24:20 to 22:26:33), including a real funds rescue.

These times are unrealistically short on one point: the rotation timelock was 60
seconds. In production it is 7 days, so a real signer rotation takes at least a
week, during which the contract stays paused or exposed. Recovery time for a real
incident is dominated by that timelock, not by the mechanics measured here.

## Rescue phase: real funds recovered

Evidence file: `drills/evidence-real-rescue-1790720780879.json`.

| Step | Detail |
|---|---|
| Stuck stealth account (real `G...` address) | `GCD7NNDLUGPQ46PLZP7MZL3KJYBJ3DOFSUOZFAGA7J6PNNVSTE3GK5SX` |
| Funded, no announcement | Tx `87c4ed19fc011ea27050bf172b5b7d343ea858e94bd674897d6d342ea3dcf028`; balance `200000000` stroops (20 XLM) |
| Destination account | `GDNGFXE3TCZBMZSTXCQQZTBTQDXHHONLIFUMQ43P7AW434QC447LFJ4L`; balance before `50000000` stroops |
| Announcement via `rescue-stealth-funds.ts` | Real Soroban `announce` tx `199a6ec27a0672ebbad51d4cc855404a434bb8cae01f4a89e69e8d93fc5eee79`, scheme 2 |
| Recipient scan | Event found through RPC `getEvents`; recipient rederived the same stealth address from the announced ephemeral key |
| Sweep (account merge, signed with the derived key) | Tx `d399b4666603a3e8aa637ce0958477e01e96d4d5c6709ffb0542425c594a4d53`; fee `100` stroops |
| Balance check after | Stuck account no longer exists; destination balance `249999900` stroops |
| Result | Destination gained `199999900` stroops (`19.9999900` XLM) = stuck balance minus the sweep fee |

The script fails the drill unless: the address derived by the rescue tool equals
the address the sender derived, the announce event carries the expected stealth
address and ephemeral key, the recipient can rederive the address, the stuck
account is gone after the sweep, and the destination gained exactly the stuck
balance minus the fee.

## Findings

### `scripts/rescue-stealth-funds.ts` (fixed in this PR)

The tool was not incident-ready. These defects are fixed and covered by tests in
`scripts/tests/rescue-stealth-funds.test.ts`:

1. **Address format mismatch broke its own safety check.** It derived
   `stealth:<hex>` strings that no network can hold funds at, so the "funds
   already moved" guard could never fire. It now derives a real Ed25519 `G...`
   account, and the guard treats a missing account or a near-empty balance as
   "nothing left to rescue".
2. **No real on-chain broadcast.** The broadcast step was a local hash. It now
   builds, simulates, signs and submits a Soroban `announce` transaction and
   waits for success. The fee-paying key comes from `RESCUE_SOURCE_SECRET`; the
   tool never asks for a spending key.
3. **Missing entry point.** The CLI now runs when the file is executed. Without
   `--yes` it performs a dry run and does not broadcast.
4. **Hardcoded v1 scheme.** The default scheme is now 2 (the v2 announcer) and
   can be overridden with `--scheme-id`.
5. **Wrong default network passphrase.** The default said `September 2025`;
   Stellar's testnet passphrase is `September 2015`. Any real broadcast with the
   old default would have failed.
6. `commander` was imported but not declared in `package.json`; this PR adds it.

### Pause-admin has no rotation path

`stealth-sender` pause and unpause are gated by one `admin` address set at `init`.
Signer rotation covers the governance signer set only. If the pause-admin key is
the one lost or compromised, there is no on-chain remedy short of a contract
upgrade.

### Build requirement

`stellar contract build` refuses to build unless `overflow-checks = true` is set in
`[profile.release]`. This PR changes that value in `stellar/Cargo.toml`, which
applies to every contract in the workspace. Maintainers should review this change.

## Rollback decision points

- **Pause:** If `is_paused` is not `true` after the pause call, stop. Do not start
  a rotation on an unpaused contract. The script aborts here. If the pause-admin
  key itself is the compromised one, there is no fallback (see Findings).
- **Signer rotation, before execute:** Confirm `pending_rotation` shows quorum. If
  quorum cannot be reached because too many signers are unreachable, propose a
  rotation with a lower threshold using only reachable signers while quorum is
  still possible. Do not `cancel_rotate_signers` to "start clean": cancelling
  discards approvals and restarts the full timelock. If the wrong signer set was
  proposed, cancelling is correct, and it costs the timelock.
- **Signer rotation, after execute:** Read `signers`. If the compromised signer is
  still present, the rotation did not do its job; the script aborts. Keep the
  contract paused.
- **Rescue, before announcing:** Run the rescue tool without `--yes` first (dry
  run) and check the computed stealth address and balance. If the tool reports no
  account or an emptied balance, stop: the funds are gone or were never sent, and
  announcing would publish a link between the ephemeral key and the address for
  nothing. An announcement is public and cannot be withdrawn.
- **Rescue, announce failed:** No funds have moved and nothing was published.
  Check the fee-paying account balance, the announcer contract ID and the scheme
  id, then retry.
- **Rescue, announce succeeded but the sweep failed:** The funds are still at the
  stealth address and the announcement stays valid. Do not announce again; have
  the recipient retry the sweep.
- **Unpause:** If `is_paused` is still `true`, re-run `unpause` (idempotent).

## Limitations of this drill

- "Operator key loss" is declared by the script, not enacted. No key is destroyed
  and no failed-auth attempt is made.
- Recovery is confirmed by `is_paused` returning `false`. The drill does not
  execute a `send` afterward to prove sends work again.
- The stealth derivation in `scripts/stealth-derivation.ts` is a DKSAP-style
  Ed25519 construction, and the announcement view tag (first byte of SHA-256 of
  the shared point) is a convention chosen here. Neither has been checked against
  a published Wraith Stellar specification. If production wallets derive
  addresses differently, the rescue tool and this drill must be updated to match.
- The rescue sweeps a native XLM balance with an account merge. Issued assets
  (trustlines) and contract-held tokens are not exercised.
- The drill runs the rescue tool's announcement step and the recipient's sweep in
  one script with a generated recipient. It does not use a real wallet.

## Recommended follow-ups

1. Add a rotation path for the pause-admin.
2. Confirm the stealth derivation and view-tag convention against the Wraith
   Stellar spec (or replace them with the reference implementation) and update
   the rescue tool and drill if they differ.
3. Rehearse the remaining rotation paths listed in `stellar/MULTISIG.md`: the production 7-day timelock, cancel-and-repropose, `stealth-batch-sender` and `wraith-names`.
4. Extend the drill to send a token after unpause.
5. Extend the rescue drill to issued assets.