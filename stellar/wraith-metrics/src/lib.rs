#![no_std]

use soroban_sdk::{contracttype, symbol_short, Symbol, Vec};

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
    pub dimensions: Vec<(Symbol, Symbol)>,
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
    dimensions: Vec<(Symbol, Symbol)>,
) {
    env.events().publish(
        (symbol_short!("metric"), contract, metric_name),
        (value, dimensions),
    );
}

/// Standard metric names
pub mod metric_names {
    use soroban_sdk::symbol_short;

    pub const REGISTER_COUNT: soroban_sdk::Symbol = symbol_short!("register_count");
    pub const REMOVE_COUNT: soroban_sdk::Symbol = symbol_short!("remove_count");
    pub const LOOKUP_COUNT: soroban_sdk::Symbol = symbol_short!("lookup_count");
    pub const SEND_COUNT: soroban_sdk::Symbol = symbol_short!("send_count");
    pub const SEND_VOLUME: soroban_sdk::Symbol = symbol_short!("send_volume");
    pub const BATCH_SEND_COUNT: soroban_sdk::Symbol = symbol_short!("batch_send_count");
    pub const BATCH_SEND_VOLUME: soroban_sdk::Symbol = symbol_short!("batch_send_volume");
    pub const BATCH_SIZE: soroban_sdk::Symbol = symbol_short!("batch_size");
    pub const ERROR_COUNT: soroban_sdk::Symbol = symbol_short!("error_count");
}

/// Standard contract identifiers
pub mod contract_ids {
    use soroban_sdk::symbol_short;

    pub const STEALTH_REGISTRY: soroban_sdk::Symbol = symbol_short!("stealth-registry");
    pub const STEALTH_SENDER: soroban_sdk::Symbol = symbol_short!("stealth-sender");
    pub const STEALTH_BATCH_SENDER: soroban_sdk::Symbol = symbol_short!("stealth-batch-sender");
    pub const STEALTH_ANNOUNCER: soroban_sdk::Symbol = symbol_short!("stealth-announcer");
}

/// Standard dimension names
pub mod dimension_names {
    use soroban_sdk::symbol_short;

    pub const SCHEME_ID: soroban_sdk::Symbol = symbol_short!("scheme_id");
    pub const TOKEN_ADDRESS: soroban_sdk::Symbol = symbol_short!("token_address");
    pub const ASSET_ADDRESS: soroban_sdk::Symbol = symbol_short!("asset_address");
    pub const ERROR_CODE: soroban_sdk::Symbol = symbol_short!("error_code");
}
