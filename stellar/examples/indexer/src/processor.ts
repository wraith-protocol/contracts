import { Server } from '@stellar/stellar-sdk';
import {
    getClient,
    getWatermark,
    setWatermark,
    rollbackToLedger,
    insertAnnouncement,
    insertRegistry,
    insertName,
    deleteName,
} from './db.js';
import dotenv from 'dotenv';

dotenv.config();

const RPC_URL = process.env.RPC_URL || 'https://futurenet.sorobanrpc.com';
const server = new Server(RPC_URL);

/**
 * Contract kinds the reference indexer understands.
 *
 * Announcement sources (announcer, sender, splitter, batch-sender) all share
 * one processing path. Splitter and batch-sender route per-transfer
 * announcements through the announcer contract, so the events appear on the
 * announcer address with the v2 4-topic layout:
 *
 *   topics: ("announce", scheme_id, view_tag_bucket, metadata_kind)
 *   data:   (stealth_address, ephemeral_pub_key, metadata)
 *
 * There is no batch-sender-specific single-topic ("ANNOUNCE",) handler —
 * that emission was removed so a topic-3 view-tag filter works uniformly.
 */
type ContractType = 'announcer' | 'registry' | 'sender' | 'names' | 'splitter' | 'batch-sender';

interface ContractConfig {
    address: string;
    type: ContractType;
}

const ANNOUNCEMENT_SOURCES: ReadonlySet<ContractType> = new Set([
    'announcer',
    'sender',
    'splitter',
    'batch-sender',
]);

const CONTRACTS: Record<string, ContractConfig> = {
    stealthAnnouncer: {
        address: process.env.STEALTH_ANNOUNCER_ADDRESS || '',
        type: 'announcer',
    },
    stealthRegistry: {
        address: process.env.STEALTH_REGISTRY_ADDRESS || '',
        type: 'registry',
    },
    stealthSender: {
        address: process.env.STEALTH_SENDER_ADDRESS || '',
        type: 'sender',
    },
    stealthSplitter: {
        address: process.env.STEALTH_SPLITTER_ADDRESS || '',
        type: 'splitter',
    },
    stealthBatchSender: {
        address: process.env.STEALTH_BATCH_SENDER_ADDRESS || '',
        type: 'batch-sender',
    },
    wraithNames: {
        address: process.env.WRAITH_NAMES_ADDRESS || '',
        type: 'names',
    },
};

export async function processContract(config: ContractConfig) {
    const client = await getClient();
    try {
        await client.query('BEGIN');

        const { latestLedger, cursor } = await getWatermark(config.address, client);

        // Check for reorg: fetch ledger info
        let latestKnownLedger = latestLedger;
        try {
            const latestLedgerInfo = await server.getLatestLedger();
            if (latestLedger > latestLedgerInfo.sequence) {
                console.warn(`Reorg detected for ${config.address}! Rolling back to ledger ${latestLedgerInfo.sequence}`);
                await rollbackToLedger(config.address, latestLedgerInfo.sequence, client);
                latestKnownLedger = latestLedgerInfo.sequence;
            }
        } catch (e) {
            console.error('Error checking latest ledger:', e);
        }

        // Fetch events
        const response = await server.getEvents({
            filters: [
                {
                    type: 'contract',
                    contractIds: [config.address],
                },
            ],
            cursor,
            limit: 100,
        });

        let newCursor = cursor;
        let newLatestLedger = latestKnownLedger;

        for (const event of response.events) {
            newCursor = event.pagingToken;
            newLatestLedger = event.ledger;

            try {
                await processEvent(config, event, client);
            } catch (err) {
                console.error('Error processing event:', err);
            }
        }

        // Update watermark
        await setWatermark(config.address, newLatestLedger, newCursor, client);

        await client.query('COMMIT');
    } catch (err) {
        await client.query('ROLLBACK');
        console.error('Error processing contract:', err);
        throw err;
    } finally {
        client.release();
    }
}

async function processEvent(config: ContractConfig, event: any, client: any) {
    const txHash = event.transactionHash || '';
    const ledger = event.ledger;

    if (!event.value || !Array.isArray(event.value)) {
        console.warn('Invalid event value:', event);
        return;
    }

    // Single code path for every announcement source (announcer, sender,
    // splitter, batch-sender). All emit / forward the v2 4-topic "announce"
    // layout; no per-source decoding remains.
    if (ANNOUNCEMENT_SOURCES.has(config.type)) {
        if (Array.isArray(event.topic) && event.topic[0] === 'announce') {
            // topics: ("announce", scheme_id, view_tag_bucket, metadata_kind)
            // data:   (stealth_address, ephemeral_pub_key, metadata)
            const schemeId = event.topic[1];
            const [stealthAddress, ephemeralPubKey, metadata] = event.value;
            await insertAnnouncement(
                ledger,
                txHash,
                config.address,
                schemeId,
                stealthAddress,
                Buffer.from(ephemeralPubKey, 'base64'),
                metadata ? Buffer.from(metadata, 'base64') : null,
                client
            );
        }
        return;
    }

    switch (config.type) {
        case 'registry':
            if (Array.isArray(event.topic) && event.topic[0] === 'register_keys') {
                const [registrant, schemeId, stealthMetaAddress] = event.value;
                await insertRegistry(
                    ledger,
                    txHash,
                    config.address,
                    registrant,
                    schemeId,
                    Buffer.from(stealthMetaAddress, 'base64'),
                    client
                );
            }
            break;
        case 'names':
            if (Array.isArray(event.topic)) {
                if (event.topic[0] === 'register') {
                    const [owner, name, stealthMetaAddress] = event.value;
                    await insertName(
                        ledger,
                        txHash,
                        config.address,
                        name,
                        owner,
                        Buffer.from(stealthMetaAddress, 'base64'),
                        client
                    );
                } else if (event.topic[0] === 'release') {
                    const [owner, name] = event.value;
                    await deleteName(ledger, txHash, config.address, name, owner, client);
                } else if (event.topic[0] === 'update') {
                    const [owner, name, newMetaAddress] = event.value;
                    await insertName(
                        ledger,
                        txHash,
                        config.address,
                        name,
                        owner,
                        Buffer.from(newMetaAddress, 'base64'),
                        client
                    );
                }
            }
            break;
    }
}

export async function startProcessing() {
    const POLL_INTERVAL_MS = parseInt(process.env.POLL_INTERVAL_MS || '5000', 10);

    for (const [name, config] of Object.entries(CONTRACTS)) {
        if (!config.address) {
            console.log(`Skipping ${name}: no address configured`);
            continue;
        }

        // Initial process
        await processContract(config);

        // Poll
        setInterval(async () => {
            await processContract(config);
        }, POLL_INTERVAL_MS);
    }
}
