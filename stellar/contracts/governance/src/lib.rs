// SPDX-License-Identifier: MIT
//
// Wraith Protocol Governance Contract — PROOF OF CONCEPT
//
// THIS IS NOT PRODUCTION READY.
// See GOVERNANCE.md for design decisions, known limitations, and the
// upgrade path to a production-grade system.
//
// Flow: propose -> vote -> execute
//  1. Anyone with a token balance creates a proposal describing an action.
//  2. Token holders vote for or against during a fixed voting window.
//  3. After voting ends + timelock delay, anyone can execute if:
//       - total votes >= quorum (absolute token threshold)
//       - for_votes > against_votes

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, token, Address, Bytes, Env,
    IntoVal, String, Symbol, Val,
};
use wraith_metrics::{contract_ids, dimension_names, emit_metric, metric_names};

// ---------------------------------------------------------------------------
// TTL constants (matching the Wraith convention)
// ---------------------------------------------------------------------------

const TTL_THRESHOLD: u32 = 17280; // ~1 day
const TTL_EXTEND_TO: u32 = 518400; // ~30 days

// ---------------------------------------------------------------------------
// Storage keys
// ---------------------------------------------------------------------------

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Admin,
    Token,
    Quorum,
    VotingPeriod,
    Timelock,
    NextProposalId,
    Proposal(u32),
    Vote(u32, Address),
}

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// A governance proposal.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    /// Sequential proposal ID.
    pub id: u32,
    /// Address that created the proposal.
    pub proposer: Address,
    /// Contract to call on execution.
    pub target: Address,
    /// Function to call on the target.
    pub function: Symbol,
    /// Raw argument bytes forwarded to the target function.
    pub args: Bytes,
    /// Human-readable description.
    pub description: String,
    /// Ledger at which voting opens.
    pub start_ledger: u32,
    /// Ledger at which voting closes.
    pub end_ledger: u32,
    /// Total tokens voted in favour.
    pub for_votes: i128,
    /// Total tokens voted against.
    pub against_votes: i128,
    /// Whether the proposal has been executed.
    pub executed: bool,
    /// Whether the proposal has been cancelled.
    pub cancelled: bool,
}

/// A single vote cast by a token holder.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Vote {
    /// true = for, false = against.
    pub support: bool,
    /// Voting weight (token balance at vote time).
    pub weight: i128,
}

/// Read-only governance configuration.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GovernanceConfig {
    pub token: Address,
    pub quorum: i128,
    pub voting_period: u32,
    pub timelock: u32,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum GovernanceError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    NotAdmin = 3,
    ProposalNotFound = 4,
    AlreadyVoted = 5,
    VotingNotActive = 6,
    VotingStillActive = 7,
    QuorumNotMet = 8,
    ProposalDefeated = 9,
    TimelockNotElapsed = 10,
    AlreadyExecuted = 11,
    AlreadyCancelled = 12,
    ExecutionFailed = 13,
    NoVotingPower = 14,
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct GovernanceContract;

#[contractimpl]
impl GovernanceContract {
    // ---- admin / config ---------------------------------------------------

    /// Initialise the governance contract.
    ///
    /// Must be called exactly once. After init the admin can create and cancel
    /// proposals — in production this role should be renounced or replaced by
    /// the governance process itself.
    ///
    /// # Arguments
    /// * `admin`          — Address with super-admin powers (PoC only).
    /// * `token`          — SAC token used for voting weight.
    /// * `quorum`         — Absolute minimum total tokens required for a valid vote.
    /// * `voting_period`  — Duration of voting window in ledgers.
    /// * `timelock`       — Delay after voting closes before execution (ledgers).
    pub fn init(
        env: Env,
        admin: Address,
        token: Address,
        quorum: i128,
        voting_period: u32,
        timelock: u32,
    ) -> Result<(), GovernanceError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(GovernanceError::AlreadyInitialized);
        }

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Token, &token);
        env.storage().instance().set(&DataKey::Quorum, &quorum);
        env.storage()
            .instance()
            .set(&DataKey::VotingPeriod, &voting_period);
        env.storage().instance().set(&DataKey::Timelock, &timelock);
        env.storage()
            .instance()
            .set(&DataKey::NextProposalId, &1u32);
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);

        Ok(())
    }

    /// Return the current governance configuration.
    pub fn get_config(env: Env) -> Result<GovernanceConfig, GovernanceError> {
        if !env.storage().instance().has(&DataKey::Token) {
            return Err(GovernanceError::NotInitialized);
        }
        Ok(GovernanceConfig {
            token: env.storage().instance().get(&DataKey::Token).unwrap(),
            quorum: env.storage().instance().get(&DataKey::Quorum).unwrap(),
            voting_period: env
                .storage()
                .instance()
                .get(&DataKey::VotingPeriod)
                .unwrap(),
            timelock: env.storage().instance().get(&DataKey::Timelock).unwrap(),
        })
    }

    // ---- proposals --------------------------------------------------------

    /// Create a new governance proposal.
    ///
    /// Anyone may propose. The proposer must authorise the call.
    ///
    /// # Arguments
    /// * `proposer`    — Address creating the proposal (must auth).
    /// * `target`      — Contract to call on execution.
    /// * `function`    — Function name to invoke on the target.
    /// * `args`        — Raw argument bytes forwarded to the target.
    /// * `description` — Human-readable proposal description.
    ///
    /// # Returns
    /// The new proposal ID.
    pub fn propose(
        env: Env,
        proposer: Address,
        target: Address,
        function: Symbol,
        args: Bytes,
        description: String,
    ) -> Result<u32, GovernanceError> {
        proposer.require_auth();

        if !env.storage().instance().has(&DataKey::Token) {
            return Err(GovernanceError::NotInitialized);
        }

        let current_ledger = env.ledger().sequence();
        let voting_period: u32 = env
            .storage()
            .instance()
            .get(&DataKey::VotingPeriod)
            .unwrap();
        let id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextProposalId)
            .unwrap();

        let proposal = Proposal {
            id,
            proposer: proposer.clone(),
            target,
            function,
            args,
            description: description.clone(),
            start_ledger: current_ledger,
            end_ledger: current_ledger + voting_period,
            for_votes: 0,
            against_votes: 0,
            executed: false,
            cancelled: false,
        };

        env.storage()
            .instance()
            .set(&DataKey::Proposal(id), &proposal);
        env.storage()
            .instance()
            .set(&DataKey::NextProposalId, &(id + 1));
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);

        env.events()
            .publish((symbol_short!("propose"), id), (proposer, description));

        // Emit metric event.
        emit_metric(
            &env,
            contract_ids::GOVERNANCE,
            metric_names::PROPOSAL_COUNT,
            1,
            soroban_sdk::vec![&env, (dimension_names::PROPOSAL_ID, id.into_val(&env))],
        );

        Ok(id)
    }

    /// Return a proposal by ID.
    pub fn get_proposal(env: Env, proposal_id: u32) -> Result<Proposal, GovernanceError> {
        env.storage()
            .instance()
            .get(&DataKey::Proposal(proposal_id))
            .ok_or(GovernanceError::ProposalNotFound)
    }

    // ---- voting -----------------------------------------------------------

    /// Cast a vote on an active proposal.
    ///
    /// Voting weight equals the voter's token balance at the time the vote is
    /// cast. Each address may vote once per proposal.
    ///
    /// # Arguments
    /// * `voter`       — Address casting the vote (must auth).
    /// * `proposal_id` — Target proposal.
    /// * `support`     — `true` = for, `false` = against.
    pub fn vote(
        env: Env,
        voter: Address,
        proposal_id: u32,
        support: bool,
    ) -> Result<(), GovernanceError> {
        voter.require_auth();

        let proposal_key = DataKey::Proposal(proposal_id);
        let mut proposal: Proposal = env
            .storage()
            .instance()
            .get(&proposal_key)
            .ok_or(GovernanceError::ProposalNotFound)?;

        if proposal.executed {
            return Err(GovernanceError::AlreadyExecuted);
        }
        if proposal.cancelled {
            return Err(GovernanceError::AlreadyCancelled);
        }

        let current_ledger = env.ledger().sequence();
        if current_ledger < proposal.start_ledger || current_ledger > proposal.end_ledger {
            return Err(GovernanceError::VotingNotActive);
        }

        let vote_key = DataKey::Vote(proposal_id, voter.clone());
        if env.storage().persistent().has(&vote_key) {
            return Err(GovernanceError::AlreadyVoted);
        }

        let token: Address = env.storage().instance().get(&DataKey::Token).unwrap();
        let balance = token::Client::new(&env, &token).balance(&voter);
        if balance <= 0 {
            return Err(GovernanceError::NoVotingPower);
        }

        if support {
            proposal.for_votes += balance;
        } else {
            proposal.against_votes += balance;
        }

        env.storage().instance().set(&proposal_key, &proposal);

        let vote = Vote {
            support,
            weight: balance,
        };
        env.storage().persistent().set(&vote_key, &vote);
        env.storage()
            .persistent()
            .extend_ttl(&vote_key, TTL_THRESHOLD, TTL_EXTEND_TO);
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);

        env.events().publish(
            (symbol_short!("vote"), proposal_id),
            (voter, support, balance),
        );

        // Emit metric event.
        emit_metric(
            &env,
            contract_ids::GOVERNANCE,
            metric_names::VOTE_COUNT,
            1,
            soroban_sdk::vec![
                &env,
                (dimension_names::PROPOSAL_ID, proposal_id.into_val(&env)),
                (dimension_names::SUPPORT, support.into_val(&env)),
            ],
        );

        Ok(())
    }

    /// Return the vote record for a given voter on a proposal.
    pub fn get_vote(env: Env, proposal_id: u32, voter: Address) -> Result<Vote, GovernanceError> {
        env.storage()
            .persistent()
            .get(&DataKey::Vote(proposal_id, voter))
            .ok_or(GovernanceError::ProposalNotFound)
    }

    // ---- execution --------------------------------------------------------

    /// Execute a proposal that has passed.
    ///
    /// Conditions (all must hold):
    ///   1. Voting window has closed.
    ///   2. Timelock delay has elapsed since voting closed.
    ///   3. Total votes cast >= quorum.
    ///   4. `for_votes > against_votes`.
    ///   5. Not already executed or cancelled.
    pub fn execute(env: Env, proposal_id: u32) -> Result<(), GovernanceError> {
        let proposal_key = DataKey::Proposal(proposal_id);
        let proposal: Proposal = env
            .storage()
            .instance()
            .get(&proposal_key)
            .ok_or(GovernanceError::ProposalNotFound)?;

        if proposal.executed {
            return Err(GovernanceError::AlreadyExecuted);
        }
        if proposal.cancelled {
            return Err(GovernanceError::AlreadyCancelled);
        }

        let current_ledger = env.ledger().sequence();

        // Condition 1: voting must have ended.
        if current_ledger <= proposal.end_ledger {
            return Err(GovernanceError::VotingStillActive);
        }

        // Condition 2: timelock must have elapsed.
        let timelock: u32 = env.storage().instance().get(&DataKey::Timelock).unwrap();
        if current_ledger < proposal.end_ledger + timelock {
            return Err(GovernanceError::TimelockNotElapsed);
        }

        // Condition 3: quorum.
        let quorum: i128 = env.storage().instance().get(&DataKey::Quorum).unwrap();
        let total_votes = proposal.for_votes + proposal.against_votes;
        if total_votes < quorum {
            return Err(GovernanceError::QuorumNotMet);
        }

        // Condition 4: majority.
        if proposal.for_votes <= proposal.against_votes {
            return Err(GovernanceError::ProposalDefeated);
        }

        // Execute — forward the stored args as a single Bytes argument.
        let args_val: Val = proposal.args.into_val(&env);
        let _: () = env.invoke_contract(
            &proposal.target,
            &proposal.function,
            soroban_sdk::vec![&env, args_val],
        );

        // Mark executed.
        let mut executed_proposal = proposal;
        executed_proposal.executed = true;
        env.storage()
            .instance()
            .set(&proposal_key, &executed_proposal);
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);

        env.events()
            .publish((symbol_short!("execute"), proposal_id), ());

        // Emit metric event.
        emit_metric(
            &env,
            contract_ids::GOVERNANCE,
            metric_names::EXECUTION_COUNT,
            1,
            soroban_sdk::vec![
                &env,
                (dimension_names::PROPOSAL_ID, proposal_id.into_val(&env))
            ],
        );

        Ok(())
    }

    // ---- cancellation -----------------------------------------------------

    /// Cancel a proposal.
    ///
    /// Rules (PoC):
    ///   - During voting: only the admin may cancel.
    ///   - After voting, without quorum: anyone may cancel (failed proposal).
    ///   - After voting, with quorum: only the admin may cancel (emergency
    ///     override — a production system would remove this power).
    pub fn cancel(env: Env, proposal_id: u32) -> Result<(), GovernanceError> {
        let proposal_key = DataKey::Proposal(proposal_id);
        let mut proposal: Proposal = env
            .storage()
            .instance()
            .get(&proposal_key)
            .ok_or(GovernanceError::ProposalNotFound)?;

        if proposal.executed {
            return Err(GovernanceError::AlreadyExecuted);
        }
        if proposal.cancelled {
            return Err(GovernanceError::AlreadyCancelled);
        }

        let current_ledger = env.ledger().sequence();
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();

        if current_ledger <= proposal.end_ledger {
            // During voting — only admin.
            admin.require_auth();
        } else {
            // After voting.
            let quorum: i128 = env.storage().instance().get(&DataKey::Quorum).unwrap();
            let total_votes = proposal.for_votes + proposal.against_votes;
            if total_votes >= quorum {
                // Met quorum — only admin can emergency-cancel.
                admin.require_auth();
            }
            // Without quorum anyone can cancel (no auth gate needed).
        }

        proposal.cancelled = true;
        env.storage().instance().set(&proposal_key, &proposal);
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);

        env.events()
            .publish((symbol_short!("cancel"), proposal_id), ());

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{
        contract, contractimpl, symbol_short,
        testutils::{Address as _, Events, Ledger},
        Bytes, Env, String,
    };

    /// A minimal contract that the governance can call during execution tests.
    #[contract]
    pub struct MockTarget;

    #[contractimpl]
    impl MockTarget {
        /// Store a value. Accepts raw Bytes — in production the governance args
        /// would need to match the target function's actual signature.
        pub fn set_value(env: Env, value: Bytes) {
            env.storage()
                .instance()
                .set(&symbol_short!("value"), &value);
        }

        /// Retrieve the stored value.
        pub fn get_value(env: Env) -> Bytes {
            env.storage()
                .instance()
                .get(&symbol_short!("value"))
                .unwrap_or(Bytes::new(&env))
        }
    }

    /// Deploy the governance contract, a mock token, and a mock target.
    /// Returns (env, governance_client, token_id, target_id, admin).
    fn setup_env() -> (
        Env,
        GovernanceContractClient<'static>,
        Address,
        Address,
        Address,
    ) {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);

        // Deploy mock token (SAC v2).
        let token_admin = Address::generate(&env);
        let token_id = env
            .register_stellar_asset_contract_v2(token_admin)
            .address();

        // Deploy governance contract.
        let gov_id = env.register(GovernanceContract, ());
        let gov_client = GovernanceContractClient::new(&env, &gov_id);

        // Deploy mock target.
        let target_id = env.register(MockTarget, ());

        // Initialise governance: quorum = 100, voting_period = 50, timelock = 10.
        gov_client.init(&admin, &token_id, &100i128, &50u32, &10u32);

        (env, gov_client, token_id, target_id, admin)
    }

    // ---- happy path -------------------------------------------------------

    #[test]
    fn test_happy_path_propose_vote_execute() {
        let (env, gov, token_id, target_id, _admin) = setup_env();
        let token_admin_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);

        let voter = Address::generate(&env);
        token_admin_client.mint(&voter, &200);

        let target = target_id.clone();
        let function = symbol_short!("set_value");
        let args = Bytes::from_slice(&env, b"hello-governance");
        let description = String::from_str(&env, "Test proposal");
        let proposer = Address::generate(&env);

        let pid = gov.propose(&proposer, &target, &function, &args, &description);
        let p = gov.get_proposal(&pid);
        assert!(!p.executed);
        assert!(!p.cancelled);

        gov.vote(&voter, &pid, &true);

        env.ledger().with_mut(|li| {
            li.sequence_number = p.end_ledger + 20;
        });

        gov.execute(&pid);

        let p2 = gov.get_proposal(&pid);
        assert!(p2.executed);

        let target_client = MockTargetClient::new(&env, &target_id);
        let stored = target_client.get_value();
        assert_eq!(stored, Bytes::from_slice(&env, b"hello-governance"));
    }

    // ---- failed quorum ----------------------------------------------------

    #[test]
    fn test_failed_quorum_cancels() {
        let (env, gov, _token_id, _target_id, _admin) = setup_env();

        let proposer = Address::generate(&env);
        let target = Address::generate(&env);
        let function = symbol_short!("set_value");
        let args = Bytes::from_slice(&env, b"data");
        let description = String::from_str(&env, "Quorum test");

        let pid = gov.propose(&proposer, &target, &function, &args, &description);
        let p = gov.get_proposal(&pid);

        env.ledger().with_mut(|li| {
            li.sequence_number = p.end_ledger + 20;
        });

        let exec_result = gov.try_execute(&pid);
        assert_eq!(exec_result, Err(Ok(GovernanceError::QuorumNotMet)));

        gov.cancel(&pid);

        let p2 = gov.get_proposal(&pid);
        assert!(p2.cancelled);
    }

    // ---- proposal defeated ------------------------------------------------

    #[test]
    fn test_proposal_defeated_cannot_execute() {
        let (env, gov, token_id, _target_id, _admin) = setup_env();
        let token_admin_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);

        let voter = Address::generate(&env);
        token_admin_client.mint(&voter, &200);

        let proposer = Address::generate(&env);
        let target = Address::generate(&env);
        let function = symbol_short!("set_value");
        let args = Bytes::from_slice(&env, b"lost");
        let description = String::from_str(&env, "Will be defeated");

        let pid = gov.propose(&proposer, &target, &function, &args, &description);
        let p = gov.get_proposal(&pid);

        gov.vote(&voter, &pid, &false);

        env.ledger().with_mut(|li| {
            li.sequence_number = p.end_ledger + 20;
        });

        let exec_result = gov.try_execute(&pid);
        assert_eq!(exec_result, Err(Ok(GovernanceError::ProposalDefeated)));
    }

    // ---- double-vote rejection -------------------------------------------

    #[test]
    fn test_double_vote_rejected() {
        let (env, gov, token_id, _target_id, _admin) = setup_env();
        let token_admin_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);

        let voter = Address::generate(&env);
        token_admin_client.mint(&voter, &200);

        let proposer = Address::generate(&env);
        let pid = gov.propose(
            &proposer,
            &Address::generate(&env),
            &symbol_short!("set_value"),
            &Bytes::from_slice(&env, b""),
            &String::from_str(&env, ""),
        );

        gov.vote(&voter, &pid, &true);

        let second = gov.try_vote(&voter, &pid, &true);
        assert_eq!(second, Err(Ok(GovernanceError::AlreadyVoted)));
    }

    // ---- voting window enforcement ---------------------------------------

    #[test]
    fn test_vote_after_window_rejected() {
        let (env, gov, _token_id, _target_id, _admin) = setup_env();
        let voter = Address::generate(&env);

        let pid = gov.propose(
            &Address::generate(&env),
            &Address::generate(&env),
            &symbol_short!("set_value"),
            &Bytes::from_slice(&env, b""),
            &String::from_str(&env, ""),
        );

        let p = gov.get_proposal(&pid);
        env.ledger().with_mut(|li| {
            li.sequence_number = p.end_ledger + 1;
        });

        let result = gov.try_vote(&voter, &pid, &true);
        assert_eq!(result, Err(Ok(GovernanceError::VotingNotActive)));
    }

    // ---- no voting power --------------------------------------------------

    #[test]
    fn test_vote_with_zero_balance_rejected() {
        let (env, gov, _token_id, _target_id, _admin) = setup_env();

        let voter = Address::generate(&env);
        let proposer = Address::generate(&env);

        let pid = gov.propose(
            &proposer,
            &Address::generate(&env),
            &symbol_short!("set_value"),
            &Bytes::from_slice(&env, b""),
            &String::from_str(&env, ""),
        );

        let result = gov.try_vote(&voter, &pid, &true);
        assert_eq!(result, Err(Ok(GovernanceError::NoVotingPower)));
    }

    // ---- get_vote returns stored record -----------------------------------

    #[test]
    fn test_get_vote_returns_record() {
        let (env, gov, token_id, _target_id, _admin) = setup_env();
        let token_admin_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);

        let voter = Address::generate(&env);
        token_admin_client.mint(&voter, &500);

        let pid = gov.propose(
            &Address::generate(&env),
            &Address::generate(&env),
            &symbol_short!("set_value"),
            &Bytes::from_slice(&env, b""),
            &String::from_str(&env, ""),
        );

        gov.vote(&voter, &pid, &true);

        let record = gov.get_vote(&pid, &voter);
        assert!(record.support);
        assert_eq!(record.weight, 500);
    }

    // ---- admin cancel during voting window --------------------------------

    #[test]
    fn test_admin_cancels_during_voting() {
        let (env, gov, _token_id, _target_id, _admin_two) = setup_env();
        let _ = &env;

        let pid = gov.propose(
            &Address::generate(&env),
            &Address::generate(&env),
            &symbol_short!("set_value"),
            &Bytes::from_slice(&env, b""),
            &String::from_str(&env, "Admin cancel test"),
        );

        gov.cancel(&pid);
        let p = gov.get_proposal(&pid);
        assert!(p.cancelled);
    }

    // ---- timelock enforcement --------------------------------------------

    #[test]
    fn test_execute_before_timelock_rejected() {
        let (env, gov, token_id, _target_id, _admin) = setup_env();
        let token_admin_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_id);

        let voter = Address::generate(&env);
        token_admin_client.mint(&voter, &200);

        let pid = gov.propose(
            &Address::generate(&env),
            &Address::generate(&env),
            &symbol_short!("set_value"),
            &Bytes::from_slice(&env, b""),
            &String::from_str(&env, ""),
        );
        let p = gov.get_proposal(&pid);

        gov.vote(&voter, &pid, &true);

        env.ledger().with_mut(|li| {
            li.sequence_number = p.end_ledger + 5;
        });

        let result = gov.try_execute(&pid);
        assert_eq!(result, Err(Ok(GovernanceError::TimelockNotElapsed)));
    }

    // ---- get_config -------------------------------------------------------

    #[test]
    fn test_get_config() {
        let (env, gov, token_id, _target_id, _admin) = setup_env();

        let cfg = gov.get_config();
        let _ = &env;
        assert_eq!(cfg.token, token_id);
        assert_eq!(cfg.quorum, 100);
        assert_eq!(cfg.voting_period, 50);
        assert_eq!(cfg.timelock, 10);
    }

    // ---- double-init rejected ---------------------------------------------

    #[test]
    fn test_double_init_rejected() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let token_id = Address::generate(&env);

        let gov_id = env.register(GovernanceContract, ());
        let gov = GovernanceContractClient::new(&env, &gov_id);

        gov.init(&admin, &token_id, &100, &50, &10);
        let second = gov.try_init(&admin, &token_id, &100, &50, &10);
        assert_eq!(second, Err(Ok(GovernanceError::AlreadyInitialized)));
    }

    // ---- metric event shape ------------------------------------------------

    /// Return the single `("metric", "gov", name)` event in the buffer as
    /// `(metric_name, value, dimensions)`.
    fn only_metric_event(env: &Env) -> (Symbol, i128, soroban_sdk::Vec<(Symbol, Val)>) {
        let metric_topic: Val = symbol_short!("metric").into_val(env);
        let mut found = None;

        for (_, topics, data) in env.events().all().iter() {
            let first: Option<Val> = topics.first();
            if first.map(|t| t.shallow_eq(&metric_topic)) != Some(true) {
                continue;
            }

            assert!(found.is_none(), "expected exactly one metric event");
            assert_eq!(
                topics.len(),
                3,
                "metric topics are (metric, contract, name)"
            );

            let emitting: Symbol = topics.get(1).unwrap().into_val(env);
            assert_eq!(emitting, contract_ids::GOVERNANCE);

            let metric_name: Symbol = topics.get(2).unwrap().into_val(env);
            let (value, dimensions): (i128, soroban_sdk::Vec<(Symbol, Val)>) = data.into_val(env);
            found = Some((metric_name, value, dimensions));
        }

        found.expect("no metric event emitted")
    }

    #[test]
    fn test_propose_emits_proposal_count_metric() {
        let (env, gov, _token_id, target_id, _admin) = setup_env();

        let pid = gov.propose(
            &Address::generate(&env),
            &target_id,
            &symbol_short!("set_value"),
            &Bytes::from_slice(&env, b"data"),
            &String::from_str(&env, "Metric proposal"),
        );

        let (metric_name, value, dimensions) = only_metric_event(&env);
        assert_eq!(metric_name, metric_names::PROPOSAL_COUNT);
        assert_eq!(value, 1);
        assert_eq!(dimensions.len(), 1);
        assert_eq!(dimensions.get(0).unwrap().0, dimension_names::PROPOSAL_ID);
        let emitted_id: u32 = dimensions.get(0).unwrap().1.into_val(&env);
        assert_eq!(emitted_id, pid);
    }

    #[test]
    fn test_vote_emits_vote_count_metric() {
        let (env, gov, token_id, target_id, _admin) = setup_env();
        let voter = Address::generate(&env);
        soroban_sdk::token::StellarAssetClient::new(&env, &token_id).mint(&voter, &200);

        let pid = gov.propose(
            &Address::generate(&env),
            &target_id,
            &symbol_short!("set_value"),
            &Bytes::from_slice(&env, b"data"),
            &String::from_str(&env, "Metric proposal"),
        );

        gov.vote(&voter, &pid, &false);

        let (metric_name, value, dimensions) = only_metric_event(&env);
        assert_eq!(metric_name, metric_names::VOTE_COUNT);
        assert_eq!(value, 1);
        assert_eq!(dimensions.len(), 2);
        assert_eq!(dimensions.get(0).unwrap().0, dimension_names::PROPOSAL_ID);
        assert_eq!(dimensions.get(1).unwrap().0, dimension_names::SUPPORT);
        let support: bool = dimensions.get(1).unwrap().1.into_val(&env);
        assert!(!support);
    }

    #[test]
    fn test_execute_emits_execution_count_metric() {
        let (env, gov, token_id, target_id, _admin) = setup_env();

        let voter = Address::generate(&env);
        soroban_sdk::token::StellarAssetClient::new(&env, &token_id).mint(&voter, &200);

        let pid = gov.propose(
            &Address::generate(&env),
            &target_id,
            &symbol_short!("set_value"),
            &Bytes::from_slice(&env, b"metric-exec"),
            &String::from_str(&env, "Metric proposal"),
        );
        gov.vote(&voter, &pid, &true);

        let proposal = gov.get_proposal(&pid);
        env.ledger().with_mut(|li| {
            li.sequence_number = proposal.end_ledger + 20;
        });

        gov.execute(&pid);

        let (metric_name, value, dimensions) = only_metric_event(&env);
        assert_eq!(metric_name, metric_names::EXECUTION_COUNT);
        assert_eq!(value, 1);
        assert_eq!(dimensions.len(), 1);
        assert_eq!(dimensions.get(0).unwrap().0, dimension_names::PROPOSAL_ID);
    }
}
