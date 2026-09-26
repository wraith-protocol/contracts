import { startProcessing } from './processor.js';
import { startServer } from './server.js';

async function main() {
    console.log('Starting Wraith Indexer...');
    await startProcessing();
    await startServer();
}

main().catch((err) => {
    console.error('Fatal error:', err);
    process.exit(1);
});
