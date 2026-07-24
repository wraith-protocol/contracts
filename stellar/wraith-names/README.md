# wraith-names

Soroban contract mapping `.wraith` names to 64-byte stealth meta-addresses.

Core features:

- **Register / update / release** — names are 3-32 chars, lowercase
  alphanumeric, owned by a Stellar address. Reverse lookup (`name_of`) maps a
  meta-address back to its name.
- **Subdomains** — one level (`payments.alice`), controlled by the parent
  owner.
- **Gasless on-behalf calls** — `*_on_behalf` variants accept an ed25519
  signature from the owner so a sponsor can pay fees.
- **Social recovery** — per-name guardian sets with threshold + delay.
- **TTL keeper** — `extend_name_ttl` is permissionless so anyone can keep
  entries alive.
- **Premium name auctions** — sealed-bid auctions for names of 4 characters
  or fewer during the first 90 days after launch (see below).

Build and test:

```sh
cargo test -p wraith-names
cargo build -p wraith-names --target wasm32-unknown-unknown --release
```

## Premium name auctions

Short names (`joe`, `pay`, `abcd`) would be squatted at scale on day one
under first-come registration. For the first **90 days** after
`init_auctions` is called, top-level names of **≤ 4 characters** can only be
obtained through a sealed-bid auction (`src/auction.rs`). Direct `register`
calls for those names fail with `PremiumAuctionRequired` until the window
ends. Longer names and subdomains are unaffected.

### Flow

| Phase | Function | Who | Notes |
|---|---|---|---|
| Open | `start_auction(name)` | anyone | Name must be eligible, unregistered, no auction yet |
| Commit | `commit_bid(bidder, name, commitment, deposit)` | bidders | Deposit transferred to the contract; must be ≥ reserve price and ≥ the bid revealed later. Deposits may exceed the bid so they don't leak it |
| Reveal | `reveal_bid(bidder, name, amount, salt)` | bidders | Must match the commitment; amount must be ≥ reserve and ≤ deposit. Highest revealed bid wins, ties go to the earliest reveal |
| Settle | `settle_auction(name)` | admin (permissionless) | Winning bid → treasury; winner's excess deposit refunded |
| Claim | `claim_name(winner, name, meta_address)` | winner | Registers the name to the winner |
| Refund | `withdraw_bid(bidder, name)` | losers / non-revealers | Full deposit back, any time after the reveal phase ends |

The commitment is `sha256("wraith-names:auction:v1" || name || bidder_xdr ||
amount_be_16_bytes || salt_32_bytes)`. Compute it off-chain, or via
transaction **simulation only** of `compute_commitment` (submitting it
on-chain would leak the bid).

Phase durations (`commit_secs`, `reveal_secs`), the reserve price, the
payment token, the treasury, and the admin are fixed at `init_auctions`. The
90-day window starts at that call's ledger timestamp.

### Refund guarantees

- Losing and unrevealed bids are refundable **in full** via `withdraw_bid`
  once the reveal phase ends — settlement is *not* a prerequisite.
- The winner's deposit is released at settlement (bid → treasury, excess →
  winner), and settlement is permissionless, so an absent admin cannot trap
  funds.
- If nobody reveals, settlement records no winner and every committer
  withdraws their full deposit.

## Admin runbook: auction settlement

Prerequisites: `stellar` CLI configured with the admin identity, the
deployed contract ID in `$CONTRACT`, and a network alias in `$NETWORK`.

1. **Find auctions ready to settle.** Watch contract events with topics
   `("auction", "start")` / `("auction", "reveal")`, or check a specific
   name (simulation):

   ```sh
   stellar contract invoke --id $CONTRACT --network $NETWORK -- \
     get_auction --name joe
   ```

   The auction is ready when the current ledger timestamp is past
   `reveal_end` and `settled` is `false`.

2. **Settle.**

   ```sh
   stellar contract invoke --id $CONTRACT --network $NETWORK \
     --source-account admin -- \
     settle_auction --name joe
   ```

   This transfers the winning bid to the treasury and refunds the winner's
   excess deposit. Errors: `RevealPhaseNotOver` (too early),
   `AlreadySettled` (someone else settled — fine), `NoAuction` (wrong name).

3. **Verify.** Re-run the `get_auction` simulation: `settled` must be
   `true`. Check the treasury balance received `highest_amount`. A
   `("auction", "settle")` event is emitted with the winner and amount.

4. **Notify the winner** (off-chain) to call `claim_name` with their stealth
   meta-address. Claiming has no deadline, but the name stays unregistered
   until claimed.

5. **Sweep refunds.** Losing bidders self-serve via `withdraw_bid`. If users
   report stuck funds, confirm the auction's `reveal_end` has passed; every
   non-winning bid is withdrawable from that moment, with or without
   settlement.

Settlement is deliberately permissionless: if the admin is unavailable,
anyone can run step 2 and no funds are at risk.

## Storage

See `STORAGE.md` for the persistent-storage layout and rent strategy.
Auction entries (`AuctionKey::Auction`, `AuctionKey::Bid`) live in
persistent storage with a ~30-day TTL extension on every write, which
comfortably covers the configured phase durations.
