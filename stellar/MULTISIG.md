# Stellar Multisig Setup

Scripted, idempotent setup for the Wraith Protocol governance multisig on Stellar.
The `setup-multisig.sh` script configures signer weights and thresholds on a Stellar account in a single atomic transaction.

## Recommended Starting Config (3-of-5)

Per [GOVERNANCE.md](./GOVERNANCE.md), the admin account for `stealth-sender` and `wraith-names` should be a **3-of-5 Security Council multisig**.

| Role | Named Guardian | Stellar Key |
|---|---|---|
| Guardian 1 | @truthixify | `G…` (populate before mainnet) |
| Guardian 2 | @thebabalola | `G…` |
| Guardian 3 | @bbkenny | `G…` |
| Guardian 4 | @richiey1 | `G…` |
| Guardian 5 | @drips-wave | `G…` |

**Threshold**: 3 signatures required for all operations (low / med / high all set to 3).
**Master weight**: 0 (the base keypair is disabled; only the 5 named signers can act).

### Initial Setup

```bash
cd stellar/scripts

./setup-multisig.sh \
  --network mainnet \
  --account <ADMIN_ACCOUNT_G...> \
  --signers "G_TRUTHIXIFY,G_THEBABALOLA,G_BBKENNY,G_RICHIEY1,G_DRIPS_WAVE" \
  --threshold 3 \
  --dry-run   # review first

# If the plan looks correct, remove --dry-run to submit
./setup-multisig.sh \
  --network mainnet \
  --account <ADMIN_ACCOUNT_G...> \
  --signers "G_TRUTHIXIFY,G_THEBABALOLA,G_BBKENNY,G_RICHIEY1,G_DRIPS_WAVE" \
  --threshold 3 \
  --identity deployer
```

The script will verify the on-chain account state after submission and append a full audit log to `multisig-setup.log`.

---

## Recovery Procedure

### Adding a Signer

> Requires M-of-N current signers to co-sign (3 signatures for the 3-of-5 config).

1. Collect the new signer's `G...` public key.
2. Each co-signer builds and signs the `set_options` transaction:

```bash
stellar tx new set-options \
  --network mainnet \
  --source <ADMIN_ACCOUNT> \
  --signer "<NEW_SIGNER_KEY>:1" \
  --build-only \
  --xdr-out add-signer.xdr

# Each co-signer signs:
stellar tx sign \
  --network mainnet \
  --sign-with-key <COSIGNER_IDENTITY> \
  --xdr-in add-signer.xdr \
  --xdr-out add-signer.xdr
```

3. Once 3 signatures are collected, submit:

```bash
stellar tx submit --network mainnet --xdr-in add-signer.xdr
```

4. After adding, if the new total signers changes the desired M-of-N ratio, also update thresholds (see below).

### Removing a Signer

1. Build the `set_options` transaction setting the departing signer's weight to 0:

```bash
stellar tx new set-options \
  --network mainnet \
  --source <ADMIN_ACCOUNT> \
  --signer "<DEPARTING_KEY>:0" \
  --build-only \
  --xdr-out remove-signer.xdr
```

2. Collect M co-signatures (same process as above).
3. Submit. **Important**: if removing a signer drops N below M, lower the threshold first in the same transaction, otherwise the account becomes permanently locked.

### Updating Threshold Only

```bash
stellar tx new set-options \
  --network mainnet \
  --source <ADMIN_ACCOUNT> \
  --low-threshold <NEW_T> \
  --med-threshold <NEW_T> \
  --high-threshold <NEW_T> \
  --build-only \
  --xdr-out update-threshold.xdr
```

Collect M signatures, then submit.

### Emergency: Account Locked (threshold unreachable)

If fewer than M signers are available and the account is locked:

1. Contact remaining signers immediately — any M-of-N subset can still recover.
2. If keys are lost and recovery is impossible, the account is permanently locked. This is a **non-recoverable** state for the admin account itself, but deployed contracts remain operational (they only call `require_auth` on their stored admin address; the contracts themselves are unaffected).
3. To mitigate: keep an offline copy of all guardian keys in separate secure locations.

---

## Audit Trail

Every run of `setup-multisig.sh` appends a timestamped log to `multisig-setup.log` (or `--log-file` path):

```
[2026-06-26T11:53:00Z] [INFO] === Multisig Setup Plan ===
[2026-06-26T11:53:00Z] [INFO] Network:       futurenet
[2026-06-26T11:53:00Z] [INFO] Account:       GABC...
[2026-06-26T11:53:00Z] [INFO] Signers (N=5):
[2026-06-26T11:53:00Z] [INFO]   GA1... (weight=1)
...
[2026-06-26T11:53:01Z] [INFO] Transaction built.
[2026-06-26T11:53:02Z] [INFO] Submitting transaction...
[2026-06-26T11:53:03Z] [INFO] ✓ High threshold confirmed: 3
[2026-06-26T11:53:03Z] [INFO] Signers found on-chain: 5 (expected 5)
[2026-06-26T11:53:03Z] [INFO] === Setup complete. Audit log: multisig-setup.log ===
```

Commit `multisig-setup.log` to your ops runbook repo (not this repository) after each governance change.

---

## On-Chain Signer Rotation (`stealth-sender`, `wraith-names`)

The account-level Stellar multisig above governs the *admin key* that submits
transactions. Separately, `stealth-sender` and `wraith-names` — the two
contracts GOVERNANCE.md marks **Timelock + Multisig Upgradable** — each keep
their own on-chain governance signer set and quorum threshold, used to
authorise a `rotate_signers` flow without a contract redeploy (issue #104).

This is intentionally a second, independent layer: the Stellar account
multisig above controls *who can submit transactions at all*; the contract's
own signer set controls *quorum + timelock for signer rotation specifically*,
enforced by the contract itself regardless of which account submitted the
call.

### Flow

1. **Propose** — any current signer proposes a new signer set + threshold.
   Their approval is recorded automatically. Only one rotation may be
   pending at a time.
   ```bash
   stellar contract invoke \
     --network futurenet \
     --id <CONTRACT_ID> \
     --source <SIGNER_IDENTITY> \
     -- propose_rotate_signers \
     --caller <SIGNER_G...> \
     --new_signers '["G_NEW1","G_NEW2","G_NEW3","G_NEW4","G_NEW5"]' \
     --new_threshold 3
   ```
   Rejected with `InvalidThreshold` if `new_threshold` is `0` or greater than
   `new_signers.len()` — a quorum that could never be reached.

2. **Approve** — remaining current signers add their approval until quorum
   (the *current* threshold) is met:
   ```bash
   stellar contract invoke \
     --network futurenet \
     --id <CONTRACT_ID> \
     --source <SIGNER_IDENTITY> \
     -- approve_rotate_signers \
     --caller <SIGNER_G...>
   ```

3. **Wait out the timelock** — 7 days from the `propose` call
   (`ROTATION_TIMELOCK_SECS`), mirroring the upgrade timelock in
   GOVERNANCE.md.

4. **Execute** — once quorum is met and the timelock has elapsed, any
   current signer executes the rotation. This swaps in the new signer set
   and threshold, clears the proposal, and emits `SignersRotated(new_signers,
   old_threshold, new_threshold)`:
   ```bash
   stellar contract invoke \
     --network futurenet \
     --id <CONTRACT_ID> \
     --source <SIGNER_IDENTITY> \
     -- execute_rotate_signers \
     --caller <SIGNER_G...>
   ```
   Fails with `QuorumNotMet` or `TimelockNotElapsed` if either condition
   isn't satisfied yet.

5. **Cancel (optional)** — any current signer can abort a pending rotation
   before execution. This fully clears the proposal (including collected
   approvals), so a fresh `propose_rotate_signers` can start immediately —
   no leftover state blocks it:
   ```bash
   stellar contract invoke \
     --network futurenet \
     --id <CONTRACT_ID> \
     --source <SIGNER_IDENTITY> \
     -- cancel_rotate_signers \
     --caller <SIGNER_G...>
   ```

### Inspecting state

```bash
stellar contract invoke --network futurenet --id <CONTRACT_ID> --source <ANY_IDENTITY> -- signers
stellar contract invoke --network futurenet --id <CONTRACT_ID> --source <ANY_IDENTITY> -- threshold
stellar contract invoke --network futurenet --id <CONTRACT_ID> --source <ANY_IDENTITY> -- pending_rotation
```

### Futurenet rehearsal

> **Status: not yet rehearsed.** The flow above is covered by unit tests in
> `stealth-sender/src/lib.rs` and `wraith-names/src/lib.rs` (quorum + timelock
> enforcement, invalid-threshold rejection, and cancelled-mid-rotation state
> cleanup), but has not been exercised against a live futurenet deployment.
> Before relying on this runbook for a mainnet rotation, a maintainer with
> futurenet deploy access should walk through propose → approve (x2) →
> advance ledger time past the 7-day timelock → execute, and separately
> propose → approve → cancel → propose again, confirming the CLI commands
> above match the deployed contract interface, then update this section with
> the confirmed transcript.

## Script Reference

```
./setup-multisig.sh [OPTIONS]

Required:
  --network    <testnet|futurenet|mainnet>
  --account    <G... account to configure>
  --signers    <comma-separated G... signer addresses>
  --threshold  <integer M, must be ≤ number of signers>

Optional:
  --dry-run    Print plan without submitting any transaction
  --identity   <stellar identity name>  (default: $STELLAR_IDENTITY or "default")
  --log-file   <path>                   (default: multisig-setup.log)
```

Exit codes: `0` = success / dry-run OK, `1` = validation error or submission failure.
