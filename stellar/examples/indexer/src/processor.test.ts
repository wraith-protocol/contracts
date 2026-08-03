import { describe, it, expect, beforeAll, afterAll, beforeEach } from 'vitest';
import pg from 'pg';
import fs from 'fs/promises';
import path from 'path';
import { fileURLToPath } from 'url';
import {
    getClient,
    setWatermark,
    rollbackToLedger,
    insertAnnouncement,
    getWatermark,
} from './db.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const TEST_DB_URL = 'postgres://wraith:wraith@localhost:5432/wraith_indexer_test';
const pool = new pg.Pool({ connectionString: TEST_DB_URL });

async function setupTestDatabase() {
    const client = await pool.connect();
    try {
        const schemaPath = path.join(__dirname, '../../schema.sql');
        const schema = await fs.readFile(schemaPath, 'utf-8');
        await client.query(schema);
    } finally {
        client.release();
    }
}

async function teardownTestDatabase() {
    const client = await pool.connect();
    try {
        await client.query('DROP TABLE IF EXISTS watermark, announcements, registries, names CASCADE');
    } finally {
        client.release();
    }
    await pool.end();
}

describe('Reorg Handling', () => {
    beforeAll(async () => {
        await teardownTestDatabase();
        await setupTestDatabase();
    });

    afterAll(async () => {
        await teardownTestDatabase();
    });

    beforeEach(async () => {
        const client = await pool.connect();
        try {
            await client.query('TRUNCATE TABLE watermark, announcements, registries, names RESTART IDENTITY');
        } finally {
            client.release();
        }
    });

    it('should rollback events beyond the given ledger', async () => {
        const contractAddress = 'TEST_CONTRACT';
        const client = await pool.connect();

        try {
            // Insert some events
            await insertAnnouncement(
                100,
                'tx1',
                contractAddress,
                1,
                'STEALTH1',
                Buffer.from('ephem1'),
                null,
                client
            );
            await insertAnnouncement(
                101,
                'tx2',
                contractAddress,
                1,
                'STEALTH2',
                Buffer.from('ephem2'),
                null,
                client
            );
            await insertAnnouncement(
                102,
                'tx3',
                contractAddress,
                1,
                'STEALTH3',
                Buffer.from('ephem3'),
                null,
                client
            );
            await setWatermark(contractAddress, 102, 'cursor3', client);

            // Verify initial state
            const initialResult = await client.query('SELECT * FROM announcements');
            expect(initialResult.rows.length).toBe(3);

            const initialWatermark = await getWatermark(contractAddress, client);
            expect(initialWatermark.latestLedger).toBe(102);

            // Rollback to ledger 100
            await rollbackToLedger(contractAddress, 100, client);

            // Verify rollback
            const afterRollbackResult = await client.query('SELECT * FROM announcements');
            expect(afterRollbackResult.rows.length).toBe(1);
            expect(afterRollbackResult.rows[0].ledger).toBe(100);

            const afterRollbackWatermark = await getWatermark(contractAddress, client);
            expect(afterRollbackWatermark.latestLedger).toBe(100);
        } finally {
            client.release();
        }
    });
});
