# Wraith Rescue Tool — `rescue-stealth-funds.ts`

A recovery mechanism for the hypothetical case where funds land at a stealth
address without a matching on-chain announcement.

## When to Use This

| Scenario | Example | Use Rescue Tool? |
|---|---|---|
| **Operator error** | UI sends transfer but forgets to call `stealth-sender::send` | ✅ Yes |
| **Direct external send** | Someone sends tokens directly to a stealth address | ✅ Yes |
| **Chain reorg** | Stellar ledger reorg drops the announcement tx | ✅ Yes (theoretical) |
| **Normal operation** | `stealth-sender::send` works correctly | ❌ No — everything is fine |
| **Funds already moved** | Recipient already withdrew or forwarded funds | ❌ No — would be a no-op |

## Trust Assumptions

1. **The sender MUST still have the ephemeral private key.** Without it, the
   rescue tool cannot prove the connection between the stealth address and the
   intended recipient. If the ephemeral key is lost, the funds are unrecoverable
   (this is by design — it's what makes stealth addresses secure).

2. **The tool NEVER requests the sender's long-term spending key.** Only the
   ephemeral key material is needed. If you are asked for a spending key,
   you are using a modified or malicious version of this tool.

3. **The "rescue" only restores findability, not the original tx hash.** The
   announcement event lets the recipient discover the payment during scanning.
   The original transfer transaction hash is different from the announcement
   transaction hash.

## How It Works

```
Sender's ephemeral key  +  Recipient's meta-address
           │                        │
           └──────────┬─────────────┘
                      ▼
         Compute shared secret (ECDH)
                      │
                      ▼
         Derive stealth address
                      │
                      ▼
         Query balance at stealth address
                      │
                      ▼
         If balance matches expected amount
                      │
                      ▼
         Build & broadcast announcement
         via StealthAnnouncer contract
                      │
                      ▼
         Recipient can now scan & find payment
```

## Usage

### Prerequisites

- Node.js 20+
- Access to a Stellar RPC endpoint (Horizon or Soroban RPC)
- The ephemeral private key used in the original transfer
- The recipient's 64-byte stealth meta-address

### Installation

```bash
cd scripts
npm install
```

### Running

```bash
npx tsx rescue-stealth-funds.ts \
  --ephemeral-key <32-byte-hex> \
  --recipient-meta-address <64-byte-hex> \
  --amount <number> \
  --asset <asset-id> \
  --announcer <contract-id> \
  --rpc <rpc-url> \
  --network-passphrase "<passphrase>" \
  --yes
```

### Example

```bash
npx tsx rescue-stealth-funds.ts \
  --ephemeral-key 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
  --recipient-meta-address aaaa...bbbb \
  --amount 100 \
  --asset XLM \
  --announcer CDLZFC3SYJYDKTNBT7YIJ4HPN5XKKBYYY7QB7QY7PJY7PJY7PJY7PJY \
  --rpc https://horizon-testnet.stellar.org \
  --network-passphrase "Test SDF Network ; September 2025" \
  --yes
```

### Options

| Option | Required | Description |
|---|---|---|
| `--ephemeral-key` | ✅ | Ephemeral private key (32 bytes hex) |
| `--recipient-meta-address` | ✅ | Recipient's stealth meta-address (64 bytes hex) |
| `--amount` | ✅ | Amount of tokens sent |
| `--asset` | ✅ | Asset identifier ("XLM" or "CODE:ISSUER") |
| `--announcer` | ✅ | StealthAnnouncer contract ID or account ID |
| `--rpc` | ❌ | RPC URL (defaults to testnet) |
| `--network-passphrase` | ❌ | Network passphrase (defaults to testnet) |
| `--yes` | ❌ | Skip confirmation prompt |

## Safety Features

1. **Balance verification:** The tool queries the stealth address balance and
   compares it to the expected amount. If the balance doesn't match, it warns
   the user.

2. **Moved-funds detection:** If the balance is significantly lower than
   expected, the tool suspects funds have been moved and refuses to proceed.

3. **Refusal on empty addresses:** If the stealth address has no funds or the
   balance is zero, the tool warns and asks for confirmation.

4. **Explicit confirmation:** The `--yes` flag is required to prevent
   accidental announcements.

## Testing

```bash
cd scripts
npm install
npx vitest run
```

The test suite validates:
- Parsing of 64-byte meta-addresses
- Deterministic shared secret computation
- Stealth address derivation
- Edge cases (wrong-length keys, invalid hex)
- Balance matching logic
- Announcement payload construction

## Integration with Soroban

In a production deployment, the tool would use `@stellar/stellar-sdk` to:

1. Build a Soroban transaction that calls `announce()` on the announcer contract
2. Simulate the transaction to check validity
3. Sign with the sender's key (for fee payment — never the spending key)
4. Submit the transaction to the Soroban RPC

The current version outputs the announcement payload and prepares the
transaction structure. Full Soroban RPC integration requires:
- A funded Stellar account for fee payment
- The `@stellar/stellar-sdk` library

## Linking

This tool is linked from:
- [Mainnet Readiness Doc](../MAINNET_READINESS.md)
- [POSTMORTEMS.md — PM-001/R](../stellar/POSTMORTEMS.md#pm-001r-rescue-tool-design-rationale)
