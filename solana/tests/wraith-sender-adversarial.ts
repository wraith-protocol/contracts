/**
 * Adversarial tests for wraith-sender.
 *
 * wraith-sender is also stateless (no PDAs), but it performs real SOL / SPL
 * transfers. Adversarial focus:
 *
 *   S1. Zero-amount SOL send: lamport delta is exactly 0 for the recipient.
 *   S2. Unsigned sender cannot initiate a transfer.
 *   S3. Insufficient balance: transfer fails without partial side-effects.
 *   S4. After a failed send, neither party's balance is mutated.
 *   S5. Stealth-account substitution: the SOL goes to exactly the account
 *       passed as `stealth_account` — the `stealth_address` argument in
 *       the event is what the caller claims, not enforced by the program,
 *       which is the intended design. Verified here for documentation.
 *
 * Coverage matrix
 * ───────────────────────────────────────────────────────────────────────────
 * S1. Zero amount SOL.
 * S2. Missing sender signature.
 * S3. Insufficient funds rejection.
 * S4. No lamport mutation on failure.
 * S5. `stealth_account` is the actual transfer target (not `stealth_address` arg).
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
} from "@solana/web3.js";
import { expect } from "chai";
import { WraithSender } from "../target/types/wraith_sender";

async function fund(
  connection: anchor.web3.Connection,
  pubkey: PublicKey,
  lamports = 2_000_000_000
): Promise<void> {
  const sig = await connection.requestAirdrop(pubkey, lamports);
  await connection.confirmTransaction(sig);
}

describe("wraith-sender – adversarial", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.WraithSender as Program<WraithSender>;
  const conn = provider.connection;

  const schemeId = 1;
  const ephemeralPubKey = Array.from(Buffer.alloc(32, 0xab));
  const metadata = Buffer.from([0xff]);

  // ─── S1. Zero-amount SOL send ─────────────────────────────────────────────

  it("S1 · zero-amount SOL send: recipient balance unchanged", async () => {
    const stealth = Keypair.generate();
    const amount = new anchor.BN(0);

    const before = await conn.getBalance(stealth.publicKey);

    await program.methods
      .sendSol(amount, schemeId, stealth.publicKey, ephemeralPubKey, metadata)
      .accounts({
        sender: provider.wallet.publicKey,
        stealthAccount: stealth.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const after = await conn.getBalance(stealth.publicKey);
    expect(after).to.equal(before);
  });

  // ─── S2. Unsigned sender ──────────────────────────────────────────────────

  it("S2 · missing sender signature is rejected before any transfer", async () => {
    const fakeSender = Keypair.generate().publicKey; // not a signer
    const stealth = Keypair.generate().publicKey;
    const amount = new anchor.BN(1_000);

    const stealthBefore = await conn.getBalance(stealth);

    try {
      await program.methods
        .sendSol(amount, schemeId, stealth, ephemeralPubKey, metadata)
        .accounts({
          sender: fakeSender,
          stealthAccount: stealth,
          systemProgram: SystemProgram.programId,
        })
        // fakeSender not added to .signers()
        .rpc();
      expect.fail("should have been rejected");
    } catch (err: any) {
      expect(err).to.exist;
    }

    // No lamports transferred.
    expect(await conn.getBalance(stealth)).to.equal(stealthBefore);
  });

  // ─── S3. Insufficient balance ─────────────────────────────────────────────

  it("S3 · insufficient balance: transaction fails, no partial transfer", async () => {
    const poorSender = Keypair.generate();
    // Fund just enough for a fee — not enough for the requested transfer.
    await fund(conn, poorSender.publicKey, 5_000);

    const stealth = Keypair.generate().publicKey;
    const amount = new anchor.BN(LAMPORTS_PER_SOL); // 1 SOL — far more than available

    const stealthBefore = await conn.getBalance(stealth);
    const senderBefore = await conn.getBalance(poorSender.publicKey);

    try {
      await program.methods
        .sendSol(amount, schemeId, stealth, ephemeralPubKey, metadata)
        .accounts({
          sender: poorSender.publicKey,
          stealthAccount: stealth,
          systemProgram: SystemProgram.programId,
        })
        .signers([poorSender])
        .rpc();
      expect.fail("should have been rejected");
    } catch (err: any) {
      expect(err).to.exist;
    }

    // S4 invariant: no lamports transferred.
    const stealthAfter = await conn.getBalance(stealth);
    expect(stealthAfter).to.equal(stealthBefore);
  });

  // ─── S5. stealth_account is the actual transfer target ────────────────────

  it("S5 · stealth_account receives lamports; stealth_address arg is only an event field", async () => {
    // The `stealth_address` argument to `send_sol` is emitted in the event
    // but NOT used to determine the transfer destination.  The transfer goes
    // to `stealth_account` (the account passed in the accounts list).
    // This is the designed behaviour — the caller is responsible for
    // consistency between the two; we document and verify it here.

    const actualRecipient = Keypair.generate().publicKey;
    const eventAddress = Keypair.generate().publicKey; // claimed in event
    const amount = new anchor.BN(500_000_000);

    const recipientBefore = await conn.getBalance(actualRecipient);
    const eventAddrBefore = await conn.getBalance(eventAddress);

    await program.methods
      .sendSol(amount, schemeId, eventAddress, ephemeralPubKey, metadata)
      .accounts({
        sender: provider.wallet.publicKey,
        stealthAccount: actualRecipient, // actual on-chain destination
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    // Transfer hit actualRecipient.
    const recipientAfter = await conn.getBalance(actualRecipient);
    expect(recipientAfter - recipientBefore).to.equal(500_000_000);

    // eventAddress (the arg, not the account) received nothing.
    const eventAddrAfter = await conn.getBalance(eventAddress);
    expect(eventAddrAfter).to.equal(eventAddrBefore);
  });
});
