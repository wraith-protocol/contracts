# Flake Handling

Integration tests against futurenet are inherently flaky due to:
- RPC timeouts and rate limits
- Friendbot funding delays
- Ledger close timing

## Mitigations
- CI job uses `continue-on-error: true`
- Tests retry RPC calls up to 3 times with exponential backoff
- Each test is independent — no shared state between scenarios
- `workflow_dispatch` allows manual re-runs on transient failures

## Triage
If >3 consecutive weekly runs fail, file a bug against the RPC endpoint
before assuming a contract regression.

## Per-contract flake notes

### announcer (`announcer_announce_through_chaos`, `announcer_multiple_schemes_through_chaos`)
No on-chain state read before the call; purely write-once.
Flakes are almost always `Http500` or `Timeout` on the RPC ingestion path.
Retry up to 3 times before filing a bug.

### registry (`registry_register_and_lookup_through_chaos`, `registry_wrong_length_rejected_through_chaos`)
The `stealth_meta_address_of` read can return `EmptyResponse` if the ledger
entry TTL has expired between write and read.  On futurenet this is rare but
possible during high load.  Retry once; if the second attempt also returns
empty, extend the entry TTL before re-registering.

### sender (`sender_send_eth_through_chaos`, `sender_batch_send_through_chaos`, `sender_announcer_lifecycle_through_chaos`)
Cross-contract calls (sender → announcer) can hit `WrongLedger` if the
announcer contract instance entry was bumped between the two calls.  This is
non-retryable by policy; treat as transient and re-run the test.

### names (`names_register_and_resolve_through_chaos`, `names_update_through_chaos`, `names_release_and_reregister_through_chaos`, `names_duplicate_rejected_through_chaos`, `names_reverse_lookup_through_chaos`)
Name storage is persistent; `WrongLedger` is the most common flake mode when
the network is under load.  The reverse-lookup test is the most sensitive
because it performs two reads.  If both reads fail, consider that a network
issue rather than a contract regression.

### vault (`vault_deposit_and_claim_through_chaos`, `vault_deposit_and_refund_through_chaos`)
`deposit` makes a cross-contract call to the announcer and a token transfer in
the same transaction.  `Timeout` flakes during the deposit step are the most
common; retry once with doubled backoff.  `claim` and `refund` are safe to
retry unconditionally — the contract guards against double-claim /
double-refund with `DepositNotFound`.  If `claim` returns `NotYetUnlocked`,
the test ledger sequence was not advanced correctly; this is a test bug, not a
network flake.

### splitter (`splitter_create_and_fund_through_chaos`)
`create_split` is idempotent on the same inputs (same split_id returned for
same beneficiaries + salt), so Http500 retries are safe.  `fund_split` is NOT
idempotent: check `get_split` `total_funded` before retrying to avoid
double-funding.  Vector-length mismatches between stealth addresses and
beneficiaries surface as `SplitNotFound`; these are always test-input bugs.

### batch-sender (`batch_sender_batch_send_through_chaos`)
All-or-nothing semantics mean that a `WrongLedger` bail is final — the
caller must re-build the batch and resubmit after confirming the current
ledger state.  An `Http500` or `Timeout` retry is safe only if the
transaction was not yet included; check recipient balances before retrying
to avoid double-sends.

### governance (`governance_propose_vote_execute_through_chaos`)
`propose` is not idempotent: each retry creates a new proposal.  Query
`get_proposal` before retrying to confirm the first attempt did not land.
`vote` is idempotent for a given (proposal_id, voter) pair — the contract
rejects duplicates with `AlreadyVoted`, so Http500 retries are safe.
`execute` is also idempotent; the contract rejects re-execution with
`AlreadyExecuted`.  Timelock flakes: if `execute` returns `TimelockNotElapsed`,
the test did not advance the ledger far enough; this is a test-setup issue,
not a network flake.
