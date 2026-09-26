-- Reference Indexer Consolidated Schema v1 for Wraith Protocol Smart Contracts
-- Compatible with PostgreSQL and SQLite

CREATE TABLE IF NOT EXISTS indexer_metadata (
    key VARCHAR(64) PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO indexer_metadata (key, value) VALUES ('schema_version', '1')
ON CONFLICT(key) DO UPDATE SET value = '1', updated_at = CURRENT_TIMESTAMP;

-- v1/v2 Announcement Events
-- Topics: ("announce", scheme_id, view_tag_bucket, metadata_kind)
-- Data: (stealth_address, ephemeral_pub_key, metadata)
CREATE TABLE IF NOT EXISTS announcements (
    id VARCHAR(128) PRIMARY KEY,
    contract_address VARCHAR(56) NOT NULL,
    scheme_id INTEGER NOT NULL,
    stealth_address VARCHAR(56) NOT NULL,
    ephemeral_pub_key VARCHAR(66) NOT NULL,
    metadata BLOB,
    caller VARCHAR(56),
    view_tag_bucket INTEGER DEFAULT NULL,
    metadata_kind VARCHAR(32) DEFAULT 'default',
    scheme_version INTEGER DEFAULT 1,
    ledger_sequence INTEGER NOT NULL,
    tx_hash VARCHAR(64) NOT NULL,
    block_timestamp TIMESTAMP NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_v1_announcements_lookup 
ON announcements(scheme_id, stealth_address);

CREATE INDEX idx_v1_announcements_view_tag 
ON announcements(scheme_id, view_tag_bucket, ledger_sequence);

-- v1 Meta-Address Registrations
CREATE TABLE IF NOT EXISTS registrations (
    registrant VARCHAR(56) NOT NULL,
    scheme_id INTEGER NOT NULL,
    stealth_meta_address VARCHAR(130) NOT NULL,
    storage_type VARCHAR(16) DEFAULT 'persistent',
    ttl_expiry_ledger INTEGER DEFAULT 0,
    is_active BOOLEAN DEFAULT TRUE,
    ledger_sequence INTEGER NOT NULL,
    tx_hash VARCHAR(64) NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (registrant, scheme_id)
);

CREATE INDEX idx_v1_registrations_ttl 
ON registrations(ttl_expiry_ledger);

-- v1 Wraith Names Registration & Subdomains
CREATE TABLE IF NOT EXISTS name_registrations (
    name VARCHAR(255) NOT NULL,
    name_hash VARCHAR(64) PRIMARY KEY,
    owner VARCHAR(56) NOT NULL,
    stealth_meta_address VARCHAR(130) NOT NULL,
    parent_hash VARCHAR(64) DEFAULT NULL,
    guardians TEXT DEFAULT NULL,
    recovery_status VARCHAR(32) DEFAULT 'NONE',
    is_paused BOOLEAN DEFAULT FALSE,
    ledger_sequence INTEGER NOT NULL,
    tx_hash VARCHAR(64) NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_v1_names_owner ON name_registrations(owner);
CREATE INDEX idx_v1_names_parent ON name_registrations(parent_hash);

-- Stealth Payment Splitter
CREATE TABLE IF NOT EXISTS splitter_splits (
    split_id VARCHAR(64) PRIMARY KEY,
    creator VARCHAR(56) NOT NULL,
    asset VARCHAR(56) NOT NULL,
    salt VARCHAR(64) NOT NULL,
    total_funded BIGINT DEFAULT 0,
    created_at_ledger INTEGER NOT NULL,
    tx_hash VARCHAR(64) NOT NULL
);

-- Pausable Circuit-Breaker State Tracking
CREATE TABLE IF NOT EXISTS pausable_states (
    contract_address VARCHAR(56) PRIMARY KEY,
    is_paused BOOLEAN NOT NULL DEFAULT FALSE,
    admin VARCHAR(56) NOT NULL,
    last_changed_ledger INTEGER NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Governance Upgrade Timelock Proposals
CREATE TABLE IF NOT EXISTS timelock_proposals (
    proposal_id VARCHAR(64) PRIMARY KEY,
    contract_address VARCHAR(56) NOT NULL,
    proposed_wasm_hash VARCHAR(64) NOT NULL,
    proposed_at_ledger INTEGER NOT NULL,
    timelock_end_ledger INTEGER NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'PROPOSED',
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_v1_timelock_status 
ON timelock_proposals(contract_address, status);

-- Rate Limit Records
CREATE TABLE IF NOT EXISTS rate_limit_records (
    id VARCHAR(128) PRIMARY KEY,
    contract_address VARCHAR(56) NOT NULL,
    sender VARCHAR(56) NOT NULL,
    window_start_ledger INTEGER NOT NULL,
    tx_count INTEGER NOT NULL DEFAULT 1,
    window_limit INTEGER NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_v1_rate_limit_sender 
ON rate_limit_records(contract_address, sender, window_start_ledger);

-- Time-Locked Vault Deposits
CREATE TABLE IF NOT EXISTS vault_deposits (
    deposit_id VARCHAR(64) PRIMARY KEY,
    sender VARCHAR(56) NOT NULL,
    recipient VARCHAR(56) NOT NULL,
    amount NUMERIC(38, 0) NOT NULL,
    asset VARCHAR(56) NOT NULL,
    unlock_ledger INTEGER NOT NULL,
    refund_after INTEGER NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'LOCKED',
    created_at_ledger INTEGER NOT NULL,
    tx_hash VARCHAR(64) NOT NULL
);

CREATE INDEX idx_v1_vault_recipient 
ON vault_deposits(recipient, status);

-- Asset Policy Rules
CREATE TABLE IF NOT EXISTS asset_policy_rules (
    policy_address VARCHAR(56) NOT NULL,
    asset_address VARCHAR(56) NOT NULL,
    is_allowed BOOLEAN NOT NULL DEFAULT TRUE,
    updated_at_ledger INTEGER NOT NULL,
    PRIMARY KEY (policy_address, asset_address)
);
