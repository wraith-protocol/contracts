-- SQL Migration: Indexer Schema v0 to v1
-- Migration for Wraith Protocol Stellar Smart Contracts (v0 -> v1 cut)
-- Validated against SQLite and PostgreSQL reference indexers

BEGIN TRANSACTION;

-- 1. Update Announcements Schema for v2 Topic Layout
-- Introduces view_tag_bucket (topic 2) and metadata_kind (topic 3)
ALTER TABLE announcements ADD COLUMN view_tag_bucket INTEGER DEFAULT NULL;
ALTER TABLE announcements ADD COLUMN metadata_kind VARCHAR(32) DEFAULT 'default';
ALTER TABLE announcements ADD COLUMN scheme_version INTEGER DEFAULT 1;

-- Fast index for v2 announcer queries: filter by scheme_id and view_tag_bucket
CREATE INDEX IF NOT EXISTS idx_v1_announcements_view_tag 
ON announcements(scheme_id, view_tag_bucket, ledger_sequence);

-- 2. Update Meta-Address Registration Schema
-- Adds storage persistence metadata and TTL tracking
ALTER TABLE registrations ADD COLUMN storage_type VARCHAR(16) DEFAULT 'persistent';
ALTER TABLE registrations ADD COLUMN ttl_expiry_ledger INTEGER DEFAULT 0;
ALTER TABLE registrations ADD COLUMN is_active BOOLEAN DEFAULT TRUE;

CREATE INDEX IF NOT EXISTS idx_v1_registrations_ttl 
ON registrations(ttl_expiry_ledger);

-- 3. Update Wraith Names Schema
-- Adds hierarchical subdomain (parent_hash), multi-sig recovery, and guardian metadata
ALTER TABLE name_registrations ADD COLUMN parent_hash VARCHAR(64) DEFAULT NULL;
ALTER TABLE name_registrations ADD COLUMN guardians TEXT DEFAULT NULL;
ALTER TABLE name_registrations ADD COLUMN recovery_status VARCHAR(32) DEFAULT 'NONE';
ALTER TABLE name_registrations ADD COLUMN is_paused BOOLEAN DEFAULT FALSE;

CREATE INDEX IF NOT EXISTS idx_v1_names_parent 
ON name_registrations(parent_hash);

-- 4. Pausable Circuit-Breaker State Tracking Table
CREATE TABLE IF NOT EXISTS pausable_states (
    contract_address VARCHAR(56) PRIMARY KEY,
    is_paused BOOLEAN NOT NULL DEFAULT FALSE,
    admin VARCHAR(56) NOT NULL,
    last_changed_ledger INTEGER NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- 5. Governance Upgrade Timelock Proposals Table
CREATE TABLE IF NOT EXISTS timelock_proposals (
    proposal_id VARCHAR(64) PRIMARY KEY,
    contract_address VARCHAR(56) NOT NULL,
    proposed_wasm_hash VARCHAR(64) NOT NULL,
    proposed_at_ledger INTEGER NOT NULL,
    timelock_end_ledger INTEGER NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'PROPOSED', -- PROPOSED, EXECUTED, CANCELLED
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_v1_timelock_status 
ON timelock_proposals(contract_address, status);

-- 6. Rate Limit Observability & Event Records Table
CREATE TABLE IF NOT EXISTS rate_limit_records (
    id VARCHAR(128) PRIMARY KEY,
    contract_address VARCHAR(56) NOT NULL,
    sender VARCHAR(56) NOT NULL,
    window_start_ledger INTEGER NOT NULL,
    tx_count INTEGER NOT NULL DEFAULT 1,
    window_limit INTEGER NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_v1_rate_limit_sender 
ON rate_limit_records(contract_address, sender, window_start_ledger);

-- 7. Time-Locked Stealth Vault Deposits Table
CREATE TABLE IF NOT EXISTS vault_deposits (
    deposit_id VARCHAR(64) PRIMARY KEY,
    sender VARCHAR(56) NOT NULL,
    recipient VARCHAR(56) NOT NULL,
    amount NUMERIC(38, 0) NOT NULL,
    asset VARCHAR(56) NOT NULL,
    unlock_ledger INTEGER NOT NULL,
    refund_after INTEGER NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'LOCKED', -- LOCKED, CLAIMED, REFUNDED
    created_at_ledger INTEGER NOT NULL,
    tx_hash VARCHAR(64) NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_v1_vault_recipient 
ON vault_deposits(recipient, status);

-- 8. Asset Policy Allowlist Rules Table
CREATE TABLE IF NOT EXISTS asset_policy_rules (
    policy_address VARCHAR(56) NOT NULL,
    asset_address VARCHAR(56) NOT NULL,
    is_allowed BOOLEAN NOT NULL DEFAULT TRUE,
    updated_at_ledger INTEGER NOT NULL,
    PRIMARY KEY (policy_address, asset_address)
);

-- 9. Update Schema Version Tracker
UPDATE indexer_metadata 
SET value = '1', updated_at = CURRENT_TIMESTAMP 
WHERE key = 'schema_version';

COMMIT;
