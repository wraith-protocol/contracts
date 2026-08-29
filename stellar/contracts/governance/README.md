# Governance Contract (`contracts/governance`)

**⚠️ THIS IS NOT PRODUCTION READY — PROOF OF CONCEPT ONLY**

This governance contract is a proof of concept for token-weighted on-chain governance. See [GOVERNANCE.md](../../GOVERNANCE.md) for design decisions, known limitations, and the upgrade path to a production-grade system.

## Purpose

Token-weighted governance for protocol upgrades and parameter changes. This PoC implements a basic propose → vote → execute flow with quorum requirements and timelock delays.

## Flow

1. **Propose** — Anyone with a token balance creates a proposal describing an action
2. **Vote** — Token holders vote for or against during a fixed voting window
3. **Execute** — After voting ends + timelock delay, anyone can execute if:
   - Total votes >= quorum (absolute token threshold)
   - `for_votes > against_votes`

## Entrypoints

| Function | Description | Authorization |
|----------|-------------|----------------|
| `init(env, admin, token, quorum, voting_period, timelock)` | Initialize governance contract | None (one-time initialization) |
| `get_config(env)` | Return current governance configuration | None (read-only) |
| `propose(env, proposer, target, function, args, description)` | Create a new governance proposal | `proposer` must authorize |
| `get_proposal(env, proposal_id)` | Return a proposal by ID | None (read-only) |
| `vote(env, voter, proposal_id, support)` | Cast a vote on an active proposal | `voter` must authorize |
| `get_vote(env, proposal_id, voter)` | Return the vote record for a voter on a proposal | None (read-only) |
| `execute(env, proposal_id)` | Execute a proposal that has passed | None (permissionless) |
| `cancel(env, proposal_id)` | Cancel a proposal | Admin or permissionless (see rules) |

### `init`

Initialize the governance contract.

**Parameters:**
- `admin: Address` — Address with super-admin powers (PoC only)
- `token: Address` — SAC token used for voting weight
- `quorum: i128` — Absolute minimum total tokens required for a valid vote
- `voting_period: u32` — Duration of voting window in ledgers
- `timelock: u32` — Delay after voting closes before execution (ledgers)

**Returns:** `Result<(), GovernanceError>`

**Errors:**
- `AlreadyInitialized` — Contract already initialized

### `get_config`

Return the current governance configuration.

**Returns:** `Result<GovernanceConfig, GovernanceError>`

**GovernanceConfig struct:**
```rust
pub struct GovernanceConfig {
    pub token: Address,
    pub quorum: i128,
    pub voting_period: u32,
    pub timelock: u32,
}
```

### `propose`

Create a new governance proposal.

**Parameters:**
- `proposer: Address` — Address creating the proposal (must authorize)
- `target: Address` — Contract to call on execution
- `function: Symbol` — Function name to invoke on the target
- `args: Bytes` — Raw argument bytes forwarded to the target
- `description: String` — Human-readable proposal description

**Returns:** `Result<u32, GovernanceError>` — The new proposal ID

**Events emitted:**
- `("propose", proposal_id)` with `(proposer, description)`

**Errors:**
- `NotInitialized` — Contract not initialized

### `get_proposal`

Return a proposal by ID.

**Parameters:**
- `proposal_id: u32` — Proposal ID

**Returns:** `Result<Proposal, GovernanceError>`

**Proposal struct:**
```rust
pub struct Proposal {
    pub id: u32,
    pub proposer: Address,
    pub target: Address,
    pub function: Symbol,
    pub args: Bytes,
    pub description: String,
    pub start_ledger: u32,
    pub end_ledger: u32,
    pub for_votes: i128,
    pub against_votes: i128,
    pub executed: bool,
    pub cancelled: bool,
}
```

### `vote`

Cast a vote on an active proposal.

**Parameters:**
- `voter: Address` — Address casting the vote (must authorize)
- `proposal_id: u32` — Target proposal
- `support: bool` — `true` = for, `false` = against

**Returns:** `Result<(), GovernanceError>`

**Voting weight:** Equals the voter's token balance at the time the vote is cast. Each address may vote once per proposal.

**Events emitted:**
- `("vote", proposal_id)` with `(voter, support, balance)`

**Errors:**
- `ProposalNotFound` — Proposal does not exist
- `AlreadyExecuted` — Proposal already executed
- `AlreadyCancelled` — Proposal already cancelled
- `VotingNotActive` — Not within voting window
- `AlreadyVoted` — Address already voted on this proposal
- `NoVotingPower` — Voter has zero token balance

### `get_vote`

Return the vote record for a given voter on a proposal.

**Parameters:**
- `proposal_id: u32` — Proposal ID
- `voter: Address` — Voter address

**Returns:** `Result<Vote, GovernanceError>`

**Vote struct:**
```rust
pub struct Vote {
    pub support: bool,   // true = for, false = against
    pub weight: i128,     // Voting weight (token balance at vote time)
}
```

### `execute`

Execute a proposal that has passed.

**Conditions (all must hold):**
1. Voting window has closed
2. Timelock delay has elapsed since voting closed
3. Total votes cast >= quorum
4. `for_votes > against_votes`
5. Not already executed or cancelled

**Parameters:**
- `proposal_id: u32` — Proposal ID

**Returns:** `Result<(), GovernanceError>`

**Events emitted:**
- `("execute", proposal_id)` with `()`

**Errors:**
- `ProposalNotFound` — Proposal does not exist
- `AlreadyExecuted` — Proposal already executed
- `AlreadyCancelled` — Proposal already cancelled
- `VotingStillActive` — Voting window still open
- `TimelockNotElapsed` — Timelock delay has not elapsed
- `QuorumNotMet` — Total votes < quorum
- `ProposalDefeated` — `for_votes <= against_votes`
- `ExecutionFailed` — Target contract call failed

### `cancel`

Cancel a proposal.

**Rules (PoC):**
- During voting: only the admin may cancel
- After voting, without quorum: anyone may cancel (failed proposal)
- After voting, with quorum: only the admin may cancel (emergency override — a production system would remove this power)

**Parameters:**
- `proposal_id: u32` — Proposal ID

**Returns:** `Result<(), GovernanceError>`

**Events emitted:**
- `("cancel", proposal_id)` with `()`

**Errors:**
- `ProposalNotFound` — Proposal does not exist
- `AlreadyExecuted` — Proposal already executed
- `AlreadyCancelled` — Proposal already cancelled

## Error Variants

| Error Code | Description |
|------------|-------------|
| `AlreadyInitialized = 1` | Contract already initialized |
| `NotInitialized = 2` | Contract not initialized |
| `NotAdmin = 3` | Caller is not admin |
| `ProposalNotFound = 4` | Proposal does not exist |
| `AlreadyVoted = 5` | Address already voted on this proposal |
| `VotingNotActive = 6` | Not within voting window |
| `VotingStillActive = 7` | Voting window still open |
| `QuorumNotMet = 8` | Total votes < quorum |
| `ProposalDefeated = 9` | `for_votes <= against_votes` |
| `TimelockNotElapsed = 10` | Timelock delay has not elapsed |
| `AlreadyExecuted = 11` | Proposal already executed |
| `AlreadyCancelled = 12` | Proposal already cancelled |
| `ExecutionFailed = 13` | Target contract call failed |
| `NoVotingPower = 14` | Voter has zero token balance |

## Event Topics

| Topic | Data | Description |
|-------|------|-------------|
| `("propose", proposal_id)` | `(proposer, description)` | Proposal created |
| `("vote", proposal_id)` | `(voter, support, balance)` | Vote cast |
| `("execute", proposal_id)` | `()` | Proposal executed |
| `("cancel", proposal_id)` | `()` | Proposal cancelled |

## Storage Layout

### Instance Storage
- `DataKey::Admin: Address` — Admin address
- `DataKey::Token: Address` — Voting token address
- `DataKey::Quorum: i128` — Quorum threshold
- `DataKey::VotingPeriod: u32` — Voting period duration
- `DataKey::Timelock: u32` — Timelock delay
- `DataKey::NextProposalId: u32` — Next proposal ID counter
- `DataKey::Proposal(proposal_id): Proposal` — Proposal entries

### Persistent Storage
- `DataKey::Vote(proposal_id, voter): Vote` — Individual vote records

**TTL Strategy:**
- Instance storage: Extended to `TTL_EXTEND_TO` (518400 ledgers, ~30 days) on every write
- Vote storage: Extended to `TTL_EXTEND_TO` on creation

## Pause / Admin / Metrics Posture

| Feature | Status |
|---------|--------|
| Pausable | No — no pause mechanism implemented |
| Admin | Yes — admin has super-admin powers (PoC only, should be removed in production) |
| Metrics | No — no metric events emitted |

## Related Docs

- [PAUSE.md](../../PAUSE.md) — Pause posture documentation
- [MULTISIG.md](../../MULTISIG.md) — Multisig setup documentation (for admin key)
- [METRICS.md](../../METRICS.md) — Metrics standard documentation
- [GOVERNANCE.md](../../GOVERNANCE.md) — Full governance design documentation with production upgrade path

## Constants

- `TTL_THRESHOLD: u32 = 17280` — ~1 day, TTL extension threshold
- `TTL_EXTEND_TO: u32 = 518400` — ~30 days, TTL extension target

## Known Limitations (PoC)

This is a proof of concept with known limitations:
- Admin has emergency cancel power even after quorum is met (production should remove this)
- No delegation mechanism
- No vote replay protection across upgrades
- Raw Bytes args require manual encoding/decoding (production would use structured types)
- No proposal types or validation (any target/function can be called)

See [GOVERNANCE.md](../../GOVERNANCE.md) for the full list and upgrade path.

## Testing

```bash
cargo test -p governance
```

Tests cover:
- Happy path: propose → vote → execute
- Failed quorum cancellation
- Proposal defeat (majority against)
- Double-vote rejection
- Voting window enforcement
- No voting power rejection
- Vote record retrieval
- Admin cancel during voting window
- Timelock enforcement
- Config retrieval
- Double-init rejection

Error codes are tracked in the [Stellar error catalog](../../ERRORS.md#governance).