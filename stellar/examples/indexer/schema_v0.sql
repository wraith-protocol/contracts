-- Reference Indexer Schema v0 for Wraith Protocol Smart Contracts
-- Compatible with PostgreSQL and SQLite

CREATE TABLE IF NOT EXISTS indexer_metadata (
    key VARCHAR(64) PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO indexer_metadata (key, value) VALUES ('schema_version', '0');

-- v0 Announcement Events
-- Topics: ("announce", scheme_id, stealth_address)
-- Data: (caller_contract, ephemeral_pub_key, metadata)
CREATE TABLE IF NOT EXISTS announcements (
    id VARCHAR(128) PRIMARY KEY,
    contract_address VARCHAR(56) NOT NULL,
    scheme_id INTEGER NOT NULL,
    stealth_address VARCHAR(56) NOT NULL,
    ephemeral_pub_key VARCHAR(66) NOT NULL,
    metadata BLOB,
    caller VARCHAR(56),
    ledger_sequence INTEGER NOT NULL,
    tx_hash VARCHAR(64) NOT NULL,
    block_timestamp TIMESTAMP NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_v0_announcements_lookup 
ON announcements(scheme_id, stealth_address);

-- v0 Meta-Address Registration Events
-- Topics: ("register", registrant, scheme_id)
CREATE TABLE IF NOT EXISTS registrations (
    registrant VARCHAR(56) NOT NULL,
    scheme_id INTEGER NOT NULL,
    stealth_meta_address VARCHAR(130) NOT NULL,
    ledger_sequence INTEGER NOT NULL,
    tx_hash VARCHAR(64) NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (registrant, scheme_id)
);

-- v0 Wraith Names Registration & Resolution
-- Topics: (event_type, name_hash)
CREATE TABLE IF NOT EXISTS name_registrations (
    name VARCHAR(255) NOT NULL,
    name_hash VARCHAR(64) PRIMARY KEY,
    owner VARCHAR(56) NOT NULL,
    stealth_meta_address VARCHAR(130) NOT NULL,
    ledger_sequence INTEGER NOT NULL,
    tx_hash VARCHAR(64) NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_v0_names_owner ON name_registrations(owner);

-- v0 Stealth Payment Splitter
CREATE TABLE IF NOT EXISTS splitter_splits (
    split_id VARCHAR(64) PRIMARY KEY,
    creator VARCHAR(56) NOT NULL,
    asset VARCHAR(56) NOT NULL,
    salt VARCHAR(64) NOT NULL,
    total_funded BIGINT DEFAULT 0,
    created_at_ledger INTEGER NOT NULL,
    tx_hash VARCHAR(64) NOT NULL
);
