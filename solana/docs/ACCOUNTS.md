# Solana Program Account Documentation

This document describes every writable account in the Wraith Protocol Solana
programs, the authority model for each, which accounts can be reclaimed (rent
recovery), when, and by whom.  It is kept in sync with the adversarial test
suite in `tests/wraith-names-adversarial.ts`,
`tests/wraith-announcer-adversarial.ts`, and
`tests/wraith-sender-adversarial.ts`.

---

## wraith-names (`4JrrQh5aK7iLvx6MgtEQk7K7X3SsWfTLxVJu1jXEwNjD`)

### Account: `NameRecord`

| Field        | Type      | Description                                              |
|--------------|-----------|----------------------------------------------------------|
| `name`       | `String`  | The human-readable `.wraith` name (3-32 lowercase alnum/hyphen chars). |
| `meta_address` | `[u8; 64]` | 64-byte stealth meta-address: `spending_pub ‖ viewing_pub`. |
| `owner`      | `Pubkey`  | The wallet that registered the name.  Enforced by `has_one`. |
| `created_at` | `i64`     | Unix timestamp of registration.                          |

**PDA derivation:**

```
seeds = [b"name", name.as_bytes()]
program = wraith_names
```

The canonical bump is stored implicitly by Anchor.  Any instruction that passes
a PDA at a different address (wrong seed or wrong bump) is rejected with
`ConstraintSeeds` before any state is read or written.

### Instructions

#### `register(name, meta_address)`

| Account      | Writable | Signer | Constraint                     |
|--------------|----------|--------|--------------------------------|
| `name_record` | yes     | no     | `init`, seeds, bump            |
| `owner`      | yes      | **yes** | payer                         |
| `system_program` | no   | no     | —                              |

- `init` guarantees the PDA does not already exist; a second `register` for
  the same name fails with `AccountAlreadyInitialized`.
- The name is validated (length 3-32, lowercase alphanumeric or hyphens) inside
  the handler; invalid names are rejected before the account is created.

#### `update(new_meta_address)`

| Account      | Writable | Signer | Constraint                                   |
|--------------|----------|--------|----------------------------------------------|
| `name_record` | yes     | no     | `has_one = owner @ NotOwner`                 |
| `owner`      | no       | **yes** | must match `name_record.owner`              |

- `has_one = owner` is enforced at the **account-validation layer** (before the
  handler body runs).  An attacker who passes a different signer as `owner` is
  rejected with `ConstraintHasOne` (error 2001) or `NotOwner` (error 2003).
- There is no seeds constraint on `name_record` in this instruction; callers
  must supply the correct PDA themselves.  Supplying a different program-owned
  `NameRecord` whose `owner` field happens to match the signer would succeed,
  but only for accounts the signer legitimately owns.

#### `release()`

| Account      | Writable | Signer | Constraint                                   |
|--------------|----------|--------|----------------------------------------------|
| `name_record` | yes     | no     | `has_one = owner @ NotOwner`, `close = owner` |
| `owner`      | yes      | **yes** | receives rent lamports on close             |

- `has_one = owner` enforces that `name_record.owner == owner.key()`.
- `close = owner` transfers the full rent-exempt lamport balance to `owner`.
- Because `owner` **must be a signer**, the rent destination cannot be
  redirected to an arbitrary key — an attacker who is not the registered owner
  cannot supply their own key as `owner` and have it pass the `has_one` check.
- After the transaction the account is zeroed and no longer exists on-chain.

#### `resolve()`

| Account      | Writable | Signer | Constraint |
|--------------|----------|--------|------------|
| `name_record` | no      | no     | read-only  |

No state mutations.

### Rent-recovery summary

| Who can reclaim  | When                           | Destination |
|------------------|--------------------------------|-------------|
| Registered owner | Any time via `release`         | `owner` (the signer) |
| Anyone else      | Never                          | — |

---

## wraith-announcer (`9Ko7TuXHpLUH1ZsZWQEpeA9Tv7hX325ooWk5SD7Y9nuq`)

Intentionally stateless.  No on-chain accounts are created or modified.  Every
`announce` call emits a log event only.

| Account  | Writable | Signer | Constraint          |
|----------|----------|--------|---------------------|
| `caller` | yes      | **yes** | runtime signer check |

No lamports are transferred by any `announce` call.  No rent-recovery applies
(there is nothing to reclaim).

---

## wraith-sender (`E6J7GBSTjKbYANWjfTo5HfnXZ4Tg3LAasN7NrvCwn5Dq`)

Stateless except for the SOL/SPL transfer it performs as part of its purpose.
No PDAs are created.

### `send_sol(amount, scheme_id, stealth_address, ephemeral_pub_key, metadata)`

| Account          | Writable | Signer | Constraint                 |
|------------------|----------|--------|----------------------------|
| `sender`         | yes      | **yes** | source of lamports        |
| `stealth_account` | yes    | no     | destination of transfer   |
| `system_program` | no       | no     | —                          |

- `stealth_address` is an **instruction argument** used only as an event field.
  The on-chain lamport transfer targets `stealth_account` (the account in the
  accounts list), not the `stealth_address` argument.
- It is the caller's responsibility to ensure the two are consistent.

### `send_spl(amount, scheme_id, stealth_address, ephemeral_pub_key, metadata)`

| Account                | Writable | Signer | Constraint            |
|------------------------|----------|--------|-----------------------|
| `sender`               | yes      | **yes** | authority            |
| `sender_token_account` | yes      | no     | source token account |
| `stealth_token_account` | yes    | no     | destination          |
| `token_program`        | no       | no     | —                    |

No PDAs, no rent-recovery.

---

## Adversarial test coverage cross-reference

| Test ID | File                               | Scenario                                 |
|---------|------------------------------------|------------------------------------------|
| 1       | wraith-names-adversarial.ts        | Wrong seed → PDA mismatch                |
| 2       | wraith-names-adversarial.ts        | Wrong bump → PDA mismatch                |
| 3       | wraith-names-adversarial.ts        | Owner substitution on update             |
| 4       | wraith-names-adversarial.ts        | Owner substitution on release            |
| 5       | wraith-names-adversarial.ts        | Duplicate initialization                 |
| 6       | wraith-names-adversarial.ts        | Stale account reuse after close          |
| 7       | wraith-names-adversarial.ts        | Unauthorized close                       |
| 8       | wraith-names-adversarial.ts        | Rent-destination substitution            |
| 9       | wraith-names-adversarial.ts        | Replay update after close                |
| 10      | wraith-names-adversarial.ts        | Replay release after close               |
| 11      | wraith-names-adversarial.ts        | Legitimate rent recovery (positive)      |
| 12      | wraith-names-adversarial.ts        | Cross-name account substitution          |
| A1      | wraith-announcer-adversarial.ts    | No lamport transfer on announce          |
| A2      | wraith-announcer-adversarial.ts    | Unsigned caller rejected                 |
| A3      | wraith-announcer-adversarial.ts    | Large metadata accepted                  |
| A4      | wraith-announcer-adversarial.ts    | All-zeroes inputs accepted               |
| A5      | wraith-announcer-adversarial.ts    | Any signer may call (no allowlist)       |
| S1      | wraith-sender-adversarial.ts       | Zero-amount send                         |
| S2      | wraith-sender-adversarial.ts       | Unsigned sender rejected                 |
| S3      | wraith-sender-adversarial.ts       | Insufficient balance rejects cleanly     |
| S4      | wraith-sender-adversarial.ts       | No lamport mutation on failure           |
| S5      | wraith-sender-adversarial.ts       | stealth_account is actual transfer target |
