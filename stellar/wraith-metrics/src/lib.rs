#![no_std]

use soroban_sdk::{contracttype, symbol_short, Symbol, Val, Vec};

/// Wraith Protocol standard metric event schema.
///
/// All Wraith contracts emit metric events using this structure to enable
/// standardized off-chain observability and monitoring.
#[contracttype]
#[derive(Clone)]
pub struct WraithMetricEvent {
    /// Contract identifier (e.g., "stealth-registry", "stealth-sender")
    pub contract: Symbol,
    /// Metric name (e.g., "register_count", "send_volume")
    pub metric_name: Symbol,
    /// Numeric value of the metric
    pub value: i128,
    /// Optional dimensions for filtering/grouping (e.g., token_address, scheme_id)
    pub dimensions: Vec<(Symbol, Val)>,
}

/// Helper function to emit a metric event.
///
/// # Arguments
/// * `env` - The Soroban environment
/// * `contract` - Contract identifier
/// * `metric_name` - Metric name
/// * `value` - Metric value
/// * `dimensions` - Optional dimensions
pub fn emit_metric(
    env: &soroban_sdk::Env,
    contract: Symbol,
    metric_name: Symbol,
    value: i128,
    dimensions: Vec<(Symbol, Val)>,
) {
    env.events().publish(
        (symbol_short!("metric"), contract, metric_name),
        (value, dimensions),
    );
}

/// Standard metric names
pub mod metric_names {
    use soroban_sdk::symbol_short;

    pub const REGISTER_COUNT: soroban_sdk::Symbol = symbol_short!("reg_cnt");
    pub const REMOVE_COUNT: soroban_sdk::Symbol = symbol_short!("rem_cnt");
    pub const LOOKUP_COUNT: soroban_sdk::Symbol = symbol_short!("lkp_cnt");
    pub const SEND_COUNT: soroban_sdk::Symbol = symbol_short!("send_cnt");
    pub const SEND_VOLUME: soroban_sdk::Symbol = symbol_short!("send_vol");
    pub const BATCH_SEND_COUNT: soroban_sdk::Symbol = symbol_short!("bat_send");
    pub const BATCH_SEND_VOLUME: soroban_sdk::Symbol = symbol_short!("bat_vol");
    pub const BATCH_SIZE: soroban_sdk::Symbol = symbol_short!("bat_size");
    pub const ERROR_COUNT: soroban_sdk::Symbol = symbol_short!("err_cnt");

    // wraith-names
    pub const RENEW_COUNT: soroban_sdk::Symbol = symbol_short!("renew_cnt");
    pub const RELEASE_COUNT: soroban_sdk::Symbol = symbol_short!("rel_cnt");
    pub const RESOLVE_HIT_COUNT: soroban_sdk::Symbol = symbol_short!("res_hit");
    pub const RESOLVE_MISS_COUNT: soroban_sdk::Symbol = symbol_short!("res_miss");

    // stealth-splitter
    pub const CREATE_COUNT: soroban_sdk::Symbol = symbol_short!("crt_cnt");
    pub const FUND_COUNT: soroban_sdk::Symbol = symbol_short!("fund_cnt");
    pub const FUND_VOLUME: soroban_sdk::Symbol = symbol_short!("fund_vol");
    pub const BENEFICIARIES_PER_SPLIT: soroban_sdk::Symbol = symbol_short!("benef_cnt");

    // stealth-vault
    pub const DEPOSIT_COUNT: soroban_sdk::Symbol = symbol_short!("dep_cnt");
    pub const DEPOSIT_VOLUME: soroban_sdk::Symbol = symbol_short!("dep_vol");
    pub const CLAIM_COUNT: soroban_sdk::Symbol = symbol_short!("clm_cnt");
    pub const REFUND_COUNT: soroban_sdk::Symbol = symbol_short!("rfnd_cnt");

    // governance
    pub const PROPOSAL_COUNT: soroban_sdk::Symbol = symbol_short!("prop_cnt");
    pub const VOTE_COUNT: soroban_sdk::Symbol = symbol_short!("vote_cnt");
    pub const EXECUTION_COUNT: soroban_sdk::Symbol = symbol_short!("exec_cnt");
}

/// Standard contract identifiers
pub mod contract_ids {
    use soroban_sdk::symbol_short;

    pub const STEALTH_REGISTRY: soroban_sdk::Symbol = symbol_short!("st_reg");
    pub const STEALTH_SENDER: soroban_sdk::Symbol = symbol_short!("st_send");
    pub const STEALTH_BATCH_SENDER: soroban_sdk::Symbol = symbol_short!("st_bat_sd");
    pub const STEALTH_ANNOUNCER: soroban_sdk::Symbol = symbol_short!("st_ann");
    pub const WRAITH_NAMES: soroban_sdk::Symbol = symbol_short!("wr_names");
    pub const STEALTH_SPLITTER: soroban_sdk::Symbol = symbol_short!("st_split");
    pub const STEALTH_VAULT: soroban_sdk::Symbol = symbol_short!("st_vault");
    pub const GOVERNANCE: soroban_sdk::Symbol = symbol_short!("gov");
}

/// Standard dimension names
pub mod dimension_names {
    use soroban_sdk::symbol_short;

    pub const SCHEME_ID: soroban_sdk::Symbol = symbol_short!("scheme_id");
    pub const TOKEN_ADDRESS: soroban_sdk::Symbol = symbol_short!("tok_addr");
    pub const ASSET_ADDRESS: soroban_sdk::Symbol = symbol_short!("ast_addr");
    pub const ERROR_CODE: soroban_sdk::Symbol = symbol_short!("err_code");
    pub const PROPOSAL_ID: soroban_sdk::Symbol = symbol_short!("prop_id");
    pub const SUPPORT: soroban_sdk::Symbol = symbol_short!("support");
}
