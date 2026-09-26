# Wraith Names TTL Keeper Service

The TTL Keeper is a permissionless service for extending the Time-To-Live (TTL) of registered Wraith Names before they expire and get archived.

## Context

Soroban contract storage entries have a TTL (Time-To-Live). Entries that aren't accessed within their TTL window get archived, requiring restoration before use. The Keeper service proactively extends the TTL of registered names to prevent archival.

## How It Works

1. **Enumerate Names**: Queries the Wraith Names contract to find all registered names
2. **Check TTLs**: Reads the remaining TTL for each name entry
3. **Identify At-Risk Names**: Filters names with TTL below the threshold
4. **Extend TTLs**: Calls `extend_name_ttl()` for names needing extension
5. **Monitor Results**: Logs events and statistics

## Permissionless Design

Any account can run the Keeper—no special permissions or keys required. The `extend_name_ttl()` contract function has no access control, allowing anyone to extend any name's TTL.

## Installation

```bash
cd stellar
pnpm install
```

## Usage

### Basic Run

```bash
tsx scripts/keeper/keeper.ts extend \
  --network testnet \
  --contract CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4
```

### With Custom Thresholds

```bash
tsx scripts/keeper/keeper.ts extend \
  --network testnet \
  --contract CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4 \
  --threshold 1000 \
  --extend-to 500000
```

### Dry-Run Mode

Preview what would be done without submitting transactions:

```bash
tsx scripts/keeper/keeper.ts extend \
  --network testnet \
  --contract CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4 \
  --dry-run
```

### Cost Estimation

Estimate the cost of extending names:

```bash
tsx scripts/keeper/keeper.ts cost-estimate --names-count 1000
```

## Configuration

| Option | Default | Description |
|--------|---------|-------------|
| `--network` | testnet | Stellar network (testnet or mainnet) |
| `--contract` | - | Wraith Names contract ID (required) |
| `--threshold` | 1000 | TTL threshold in ledgers (~7 minutes) |
| `--extend-to` | 500000 | Target TTL in ledgers (~33 days) |
| `--secret-key` | - | Keeper account secret key |
| `--dry-run` | false | Show actions without executing |
| `--rpc-url` | - | Custom Soroban RPC URL |
| `--horizon-url` | - | Custom Horizon URL |

## Environment Variables

```bash
# Keeper account secret (Ed25519)
export WRAITH_KEEPER_SECRET=SBXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX

# Contract ID (optional)
export WRAITH_NAMES_CONTRACT_ID=CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4
```

## Idempotency

The Keeper is safe to run multiple times:

- **Same ledger**: Running twice in the same ledger is a no-op (second call doesn't extend again)
- **Different ledgers**: Each ledger can extend names, allowing frequent runs
- **No state required**: Keeper doesn't maintain state—it's stateless and resumable

## Contract Events

The contract emits `extend` events when TTLs are extended:

```
Event: extend
Topics:
  - symbol: "extend"
  - name_hash: BytesN<32>
Data:
  - extend_to_ledger: u32
```

These events can be monitored to track Keeper activity.

## Observability

The Keeper logs:

- Current ledger number
- Number of names found
- Number of names needing extension
- Progress of each extend operation
- Cost summary

Example output:

```
🔧 Wraith Names TTL Keeper
   Network: testnet
   Contract: CAAAA...ABSC4
   TTL Threshold: 1000 ledgers
   Extend To: 500000 ledgers
   Dry Run: No

📊 Current ledger: 123456

📖 Enumerating registered names...
   Found 42 registered names

⏱️  Checking TTLs...
   15/42 names at risk or need extension

🔄 Extending TTLs...
   - alice
   - bob
   - carol
   ... (12 more)

✅ Done!
```

## Running as a Service

### Docker

```dockerfile
FROM node:22-alpine
WORKDIR /app
COPY package.json pnpm-lock.yaml ./
RUN npm install -g pnpm && pnpm install
COPY scripts ./scripts
ENV WRAITH_KEEPER_SECRET=<your-secret-key>
ENV WRAITH_NAMES_CONTRACT_ID=<contract-id>
CMD ["tsx", "scripts/keeper/keeper.ts", "extend", "--network", "testnet"]
```

### Systemd Timer

Create `/etc/systemd/system/wraith-keeper.service`:

```ini
[Unit]
Description=Wraith Names TTL Keeper
After=network.target

[Service]
Type=oneshot
User=keeper
WorkingDirectory=/opt/keeper
Environment="WRAITH_KEEPER_SECRET=SBXXXXXXXX..."
Environment="WRAITH_NAMES_CONTRACT_ID=CAAAAAAA..."
ExecStart=/usr/bin/node /opt/keeper/keeper.ts extend --network testnet
StandardOutput=journal
StandardError=journal
```

Create `/etc/systemd/system/wraith-keeper.timer`:

```ini
[Unit]
Description=Wraith Names TTL Keeper Timer
Requires=wraith-keeper.service

[Timer]
# Run every 6 hours
OnBootSec=10min
OnUnitActiveSec=6h
AccuracySec=1min

[Install]
WantedBy=timers.target
```

Enable and start:

```bash
systemctl enable wraith-keeper.timer
systemctl start wraith-keeper.timer
systemctl status wraith-keeper.timer
```

### Cron Job

```bash
# Run every 6 hours
0 */6 * * * /usr/bin/tsx /opt/keeper/scripts/keeper/keeper.ts extend --network testnet >> /var/log/wraith-keeper.log 2>&1
```

## Performance Notes

- **Throughput**: Extend ~100 names per transaction batch
- **Cost**: ~0.00002 XLM per name (base fee + Soroban resource fees)
- **Frequency**: Recommend running every 6-24 hours
- **Latency**: Typically completes in 5-30 seconds for 1000 names

## Troubleshooting

### "No secret key provided"

Set `WRAITH_KEEPER_SECRET` environment variable:

```bash
export WRAITH_KEEPER_SECRET=SBXXXXXXXX...
```

### "Contract ID not found"

Verify the contract is deployed and ID is correct:

```bash
soroban contract info --network testnet --id CAAAA...
```

### "Names not found"

- Verify names are actually registered in the contract
- Check that you're querying the correct network
- Ensure the contract ID is correct

### "extend_name_ttl failed"

- Verify `extend_to_ledger` is in the future
- Check keeper account has enough XLM for fees
- Ensure the name exists in the contract

## Testing

Run the keeper tests:

```bash
# Unit tests (requires vitest)
pnpm test scripts/keeper

# Integration test with local contract
cargo test --manifest-path stellar/Cargo.toml
```

## Cost Model

See [KEEPER_COSTS.md](./KEEPER_COSTS.md) for detailed cost analysis.

## References

- [Soroban TTL & State Archival](https://developers.stellar.org/docs/build/guides/contract-development/storage/state-archival)
- [Wraith Names Contract](../wraith-names/)
- [Stellar Fee Structure](https://developers.stellar.org/docs/learn/fees)
