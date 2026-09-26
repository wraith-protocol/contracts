import pg from 'pg';
import dotenv from 'dotenv';

dotenv.config();

const pool = new pg.Pool({
    connectionString: process.env.DATABASE_URL,
});

export async function getClient() {
    return pool.connect();
}

export async function getWatermark(contractAddress: string, client: pg.PoolClient): Promise<{ latestLedger: number; cursor: string }> {
    const result = await client.query(
        'SELECT latest_ledger, cursor FROM watermark WHERE contract_address = $1',
        [contractAddress]
    );
    if (result.rows.length === 0) {
        return { latestLedger: 0, cursor: '0' };
    }
    return {
        latestLedger: result.rows[0].latest_ledger,
        cursor: result.rows[0].cursor,
    };
}

export async function setWatermark(
    contractAddress: string,
    latestLedger: number,
    cursor: string,
    client: pg.PoolClient
): Promise<void> {
    await client.query(
        `INSERT INTO watermark (contract_address, latest_ledger, cursor, updated_at)
         VALUES ($1, $2, $3, NOW())
         ON CONFLICT (contract_address)
         DO UPDATE SET latest_ledger = $2, cursor = $3, updated_at = NOW()`,
        [contractAddress, latestLedger, cursor]
    );
}

export async function rollbackToLedger(contractAddress: string, ledger: number, client: pg.PoolClient): Promise<void> {
    await client.query(
        'DELETE FROM announcements WHERE contract_address = $1 AND ledger > $2',
        [contractAddress, ledger]
    );
    await client.query(
        'DELETE FROM registries WHERE contract_address = $1 AND ledger > $2',
        [contractAddress, ledger]
    );
    await client.query(
        'DELETE FROM names WHERE contract_address = $1 AND ledger > $2',
        [contractAddress, ledger]
    );
    await client.query(
        `UPDATE watermark SET latest_ledger = $2, updated_at = NOW() WHERE contract_address = $1`,
        [contractAddress, ledger]
    );
}

export async function insertAnnouncement(
    ledger: number,
    txHash: string,
    contractAddress: string,
    schemeId: number,
    stealthAddress: string,
    ephemeralPubKey: Buffer,
    metadata: Buffer | null,
    client: pg.PoolClient
): Promise<void> {
    await client.query(
        `INSERT INTO announcements (ledger, transaction_hash, contract_address, scheme_id, stealth_address, ephemeral_pub_key, metadata)
         VALUES ($1, $2, $3, $4, $5, $6, $7)`,
        [ledger, txHash, contractAddress, schemeId, stealthAddress, ephemeralPubKey, metadata]
    );
}

export async function insertRegistry(
    ledger: number,
    txHash: string,
    contractAddress: string,
    registrant: string,
    schemeId: number,
    stealthMetaAddress: Buffer,
    client: pg.PoolClient
): Promise<void> {
    await client.query(
        `INSERT INTO registries (ledger, transaction_hash, contract_address, registrant, scheme_id, stealth_meta_address)
         VALUES ($1, $2, $3, $4, $5, $6)`,
        [ledger, txHash, contractAddress, registrant, schemeId, stealthMetaAddress]
    );
}

export async function insertName(
    ledger: number,
    txHash: string,
    contractAddress: string,
    name: string,
    owner: string,
    stealthMetaAddress: Buffer,
    client: pg.PoolClient
): Promise<void> {
    await client.query(
        `INSERT INTO names (ledger, transaction_hash, contract_address, name, owner, stealth_meta_address)
         VALUES ($1, $2, $3, $4, $5, $6)`,
        [ledger, txHash, contractAddress, name, owner, stealthMetaAddress]
    );
}

export async function deleteName(
    ledger: number,
    txHash: string,
    contractAddress: string,
    name: string,
    owner: string,
    client: pg.PoolClient
): Promise<void> {
    await client.query(
        `DELETE FROM names WHERE contract_address = $1 AND name = $2 AND owner = $3`,
        [contractAddress, name, owner]
    );
}
