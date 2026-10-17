# Wraith Names TTL Keeper Service

The TTL Keeper is a permissionless service for extending the Time-To-Live (TTL) of registered Wraith Names before they expire and get archived.

## Context

Soroban contract storage entries have a TTL (Time-To-Live). Entries that aren't accessed within their TTL window get archived, requiring restoration before use. The Keeper service proactively extends the TTL of registered names to prevent archival.

## How It Works

1. **Enumerate names.** The contract keeps no on-chain name list, so the keeper rebuilds it from the contract's `register`, `update` and `release` events (Soroban RPC `getEvents`, starting at `--start-ledger`) and/or from an off-chain index file (`--index-file`, a JSON array of names, e.g. exported from `examples/indexer`). Both sources are merged.
2. **Read TTLs.** For each name it reads the persistent `Name(sha256(name))` ledger entry via `getLedgerEntries` and computes `liveUntilLedgerSeq - latestLedger`.
3. **Extend.** Names with remaining TTL below `--threshold` get a real `extend_name_ttl(name, latest + extend-to)` transaction (simulated, signed, submitted and polled to completion).

## Fails closed

The keeper exits with status 1 (and prints `keeper failed [stage]: ...`) when:

- no name source is configured, or the RPC no longer retains events back to `--start-ledger` (history would be incomplete);
- event enumeration or the latest-ledger read fails;
- any TTL read fails, or a listed name has no live ledger entry (released, expired or archived);
- any `extend_name_ttl` submission fails.

`--dry-run` performs the same enumeration and TTL reads and fails the same way, but submits nothing.

Public RPC nodes keep only a few days of events. For a long-lived deployment, either run the keeper often enough, point it at a longer-retention RPC, or pass an index file.

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
  --contract CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4 \
  --start-ledger 1234567
```

### With Custom Thresholds

```bash
tsx scripts/keeper/keeper.ts extend \
  --network testnet \
  --contract CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4 \
  --start-ledger 1234567 \
  --threshold 17280 \
  --extend-to 518400
```

### Dry-Run Mode

Preview what would be done without submitting transactions:

```bash
tsx scripts/keeper/keeper.ts extend \
  --network testnet \
  --contract CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4 \
  --start-ledger 1234567 \
  --index-file names.json \
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
| `--start-ledger` | - | First ledger to scan events from (deployment ledger) |
| `--index-file` | - | JSON array of names from an off-chain index |
| `--threshold` | 17280 | Extend when remaining TTL is below this (ledgers, ~1 day) |
| `--extend-to` | 518400 | Ledgers from now to extend to (~30 days) |
| `--secret-key` | - | Keeper account secret key |
| `--dry-run` | false | Show actions without executing |
| `--rpc-url` | network default | Custom Soroban RPC URL |

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
latest ledger 123456
found 42 registered name(s)
15/42 below threshold 17280
extended "alice" to ledger 641856 (tx 9f1c...)
...
done: 42 names, 15 below threshold, 15 extended
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
ExecStart=/usr/bin/node /opt/keeper/keeper.ts extend --network testnet --start-ledger <n>
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
0 */6 * * * /usr/bin/tsx /opt/keeper/scripts/keeper/keeper.ts extend --network testnet --start-ledger <n> >> /var/log/wraith-keeper.log 2>&1
```

## Performance Notes

- **Throughput**: one `extend_name_ttl` transaction per name, submitted sequentially
- **Cost**: ~0.00002 XLM per name (base fee + Soroban resource fees)
- **Frequency**: Recommend running every 6-24 hours

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

### "no live ledger entry for N name(s)"

The name was released, expired or archived. Remove it from the index file, or restore the entry first.

### "RPC event history starts at ledger ..."

The RPC has pruned events older than `--start-ledger`. Use a longer-retention RPC or pass `--index-file`.

### "extend_name_ttl failed"

- Check the keeper account has enough XLM for fees
- Check the contract is not paused

## Testing

```bash
cd stellar
pnpm test scripts/keeper
```

The tests use an in-memory fake RPC (no network), plus CLI runs that check the nonzero exit codes. The real Soroban RPC client (`keeper-rpc.ts`) is exercised only by hand against testnet.

## Cost Model

See [KEEPER_COSTS.md](./KEEPER_COSTS.md) for detailed cost analysis.

## References

- [Soroban TTL & State Archival](https://developers.stellar.org/docs/build/guides/contract-development/storage/state-archival)
- [Wraith Names Contract](../wraith-names/)
- [Stellar Fee Structure](https://developers.stellar.org/docs/learn/fees)
