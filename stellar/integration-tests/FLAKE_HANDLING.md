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