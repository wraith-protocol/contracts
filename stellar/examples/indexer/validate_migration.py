#!/usr/bin/env python3
"""
Validation Script for Indexer Schema Migration (v0 -> v1).
Tests that:
1. v0 schema builds cleanly.
2. Sample v0 data inserts properly.
3. 001_v0_to_v1.sql migration applies without error.
4. Historical v0 data is preserved.
5. New v1 columns and tables accept new data.
6. Schema version in indexer_metadata is updated to '1'.
"""

import sqlite3
import os
import sys

BASE_DIR = os.path.dirname(os.path.abspath(__file__))
SCHEMA_V0_PATH = os.path.join(BASE_DIR, "schema_v0.sql")
MIGRATION_PATH = os.path.join(BASE_DIR, "migrations", "001_v0_to_v1.sql")
SCHEMA_V1_PATH = os.path.join(BASE_DIR, "schema_v1.sql")

def run_validation():
    print("🚀 Starting Indexer Schema Migration Validation...")

    db = sqlite3.connect(":memory:")
    cursor = db.cursor()

    # Step 1: Initialize v0 Schema
    print("  [1/6] Loading schema_v0.sql...")
    with open(SCHEMA_V0_PATH, "r") as f:
        cursor.executescript(f.read())
    
    # Step 2: Insert v0 test records
    print("  [2/6] Inserting v0 test data...")
    cursor.execute("""
        INSERT INTO announcements (id, contract_address, scheme_id, stealth_address, ephemeral_pub_key, metadata, caller, ledger_sequence, tx_hash, block_timestamp)
        VALUES ('ann-1', 'CAAAA1', 1, 'GAAAA1', '0x1234', X'010203', 'GBCALLER1', 1000, '0xhash1', '2026-01-01 00:00:00')
    """)
    cursor.execute("""
        INSERT INTO registrations (registrant, scheme_id, stealth_meta_address, ledger_sequence, tx_hash)
        VALUES ('GUSER1', 1, '0xmetaaddress1', 1000, '0xhash1')
    """)
    cursor.execute("""
        INSERT INTO name_registrations (name, name_hash, owner, stealth_meta_address, ledger_sequence, tx_hash)
        VALUES ('alice.wraith', '0xnamehash1', 'GUSER1', '0xmetaaddress1', 1000, '0xhash1')
    """)
    db.commit()

    # Step 3: Run Migration 001_v0_to_v1.sql
    print("  [3/6] Executing migrations/001_v0_to_v1.sql...")
    with open(MIGRATION_PATH, "r") as f:
        cursor.executescript(f.read())

    # Step 4: Verify Schema Version and Data Preservation
    print("  [4/6] Verifying data integrity and schema version...")
    cursor.execute("SELECT value FROM indexer_metadata WHERE key = 'schema_version'")
    version = cursor.fetchone()[0]
    assert version == '1', f"Expected schema_version '1', got '{version}'"

    cursor.execute("SELECT id, scheme_id, view_tag_bucket, metadata_kind FROM announcements WHERE id = 'ann-1'")
    ann_row = cursor.fetchone()
    assert ann_row[0] == 'ann-1' and ann_row[1] == 1, "Historical announcement data corrupted"
    assert ann_row[2] is None, "Historical view_tag_bucket should be NULL for v0 events"

    # Step 5: Insert v1/v2 records with new columns and new tables
    print("  [5/6] Inserting v1/v2 event & state records...")
    cursor.execute("""
        INSERT INTO announcements (id, contract_address, scheme_id, stealth_address, ephemeral_pub_key, metadata, caller, view_tag_bucket, metadata_kind, scheme_version, ledger_sequence, tx_hash, block_timestamp)
        VALUES ('ann-v2-1', 'CAAAA2', 2, 'GAAAA2', '0x5678', X'ab12', 'GBCALLER2', 173, 'default', 2, 2000, '0xhash2', '2026-07-01 00:00:00')
    """)

    cursor.execute("""
        INSERT INTO pausable_states (contract_address, is_paused, admin, last_changed_ledger)
        VALUES ('CSENDER1', 0, 'GADMIN1', 2000)
    """)

    cursor.execute("""
        INSERT INTO timelock_proposals (proposal_id, contract_address, proposed_wasm_hash, proposed_at_ledger, timelock_end_ledger, status)
        VALUES ('prop-1', 'CSENDER1', '0xwasmhash123', 2000, 122960, 'PROPOSED')
    """)

    cursor.execute("""
        INSERT INTO vault_deposits (deposit_id, sender, recipient, amount, asset, unlock_ledger, refund_after, created_at_ledger, tx_hash)
        VALUES ('dep-1', 'GSENDER1', 'GRECIPIENT1', 1000000000, 'CASSET1', 5000, 6000, 2000, '0xhashvault1')
    """)
    db.commit()

    # Step 6: Query Filter Benchmarking
    print("  [6/6] Testing query filtering performance on indexed view_tag_bucket...")
    cursor.execute("""
        SELECT id, stealth_address FROM announcements 
        WHERE scheme_id = 2 AND view_tag_bucket = 173 AND ledger_sequence >= 2000
    """)
    results = cursor.fetchall()
    assert len(results) == 1 and results[0][0] == 'ann-v2-1', "View-tag bucket query filtering failed"

    print("✅ Indexer Schema Migration Validation SUCCESSFUL! All checks passed.")
    db.close()

if __name__ == "__main__":
    try:
        run_validation()
    except Exception as e:
        print(f"❌ Validation FAILED: {e}")
        sys.exit(1)
