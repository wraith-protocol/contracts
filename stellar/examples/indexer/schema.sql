-- Watermarks for reorg handling
CREATE TABLE IF NOT EXISTS watermark (
    id SERIAL PRIMARY KEY,
    contract_address VARCHAR NOT NULL,
    latest_ledger INTEGER NOT NULL,
    cursor TEXT NOT NULL,
    updated_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    UNIQUE(contract_address)
);

-- Stealth Announcer events
CREATE TABLE IF NOT EXISTS announcements (
    id SERIAL PRIMARY KEY,
    ledger INTEGER NOT NULL,
    transaction_hash VARCHAR NOT NULL,
    contract_address VARCHAR NOT NULL,
    scheme_id INTEGER NOT NULL,
    stealth_address VARCHAR NOT NULL,
    ephemeral_pub_key BYTEA NOT NULL,
    metadata BYTEA,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_announcements_ledger ON announcements(ledger);
CREATE INDEX IF NOT EXISTS idx_announcements_stealth_address ON announcements(stealth_address);

-- Stealth Registry events
CREATE TABLE IF NOT EXISTS registries (
    id SERIAL PRIMARY KEY,
    ledger INTEGER NOT NULL,
    transaction_hash VARCHAR NOT NULL,
    contract_address VARCHAR NOT NULL,
    registrant VARCHAR NOT NULL,
    scheme_id INTEGER NOT NULL,
    stealth_meta_address BYTEA NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_registries_ledger ON registries(ledger);
CREATE INDEX IF NOT EXISTS idx_registries_registrant ON registries(registrant);
CREATE INDEX IF NOT EXISTS idx_registries_scheme_id ON registries(scheme_id);

-- Wraith Names events
CREATE TABLE IF NOT EXISTS names (
    id SERIAL PRIMARY KEY,
    ledger INTEGER NOT NULL,
    transaction_hash VARCHAR NOT NULL,
    contract_address VARCHAR NOT NULL,
    name VARCHAR NOT NULL,
    owner VARCHAR NOT NULL,
    stealth_meta_address BYTEA NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_names_ledger ON names(ledger);
CREATE INDEX IF NOT EXISTS idx_names_name ON names(name);
CREATE INDEX IF NOT EXISTS idx_names_owner ON names(owner);
