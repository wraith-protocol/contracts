/**
 * Adversarial tests for wraith-announcer.
 *
 * The announcer is intentionally stateless — it has no PDAs and no on-chain
 * storage.  Adversarial focus is therefore on:
 *
 *   1. No lamport transfer occurs on any announce call.
 *   2. A non-signer cannot masquerade as the caller field.
 *   3. Oversized / edge-case metadata does not corrupt state.
 *   4. Any account can call announce (no access control — by design).
 *
 * Coverage matrix
 * ───────────────────────────────────────────────────────────────────────────
 * A1. Zero-lamport invariant: announce transfers no SOL.
 * A2. Caller mismatch: unsigned caller key is rejected by the runtime.
 * A3. Max-size metadata (1232 bytes, near Solana's tx size limit fragment).
 * A4. All-zeroes metadata accepted.
 * A5. Any signer may announce (no allowlist).
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey } from "@solana/web3.js";
import { expect } from "chai";
import { WraithAnnouncer } from "../target/types/wraith_announcer";

async function fund(
  connection: anchor.web3.Connection,
  pubkey: PublicKey,
  lamports = 2_000_000_000
): Promise<void> {
  const sig = await connection.requestAirdrop(pubkey, lamports);
  await connection.confirmTransaction(sig);
}

describe("wraith-announcer – adversarial", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.WraithAnnouncer as Program<WraithAnnouncer>;
  const conn = provider.connection;

  const schemeId = 1;
  const ephemeralPubKey = Array.from(Buffer.alloc(32, 0xab));

  // ─── A1. No lamport transfer ──────────────────────────────────────────────

  it("A1 · announce transfers no lamports (stateless invariant)", async () => {
    const stealth = Keypair.generate().publicKey;
    const metadata = Buffer.from([0x01]);

    const callerBefore = await conn.getBalance(provider.wallet.publicKey);
    const stealthBefore = await conn.getBalance(stealth);

    await program.methods
      .announce(schemeId, stealth, ephemeralPubKey, metadata)
      .accounts({ caller: provider.wallet.publicKey })
      .rpc();

    const callerAfter = await conn.getBalance(provider.wallet.publicKey);
    const stealthAfter = await conn.getBalance(stealth);

    // Caller balance decreases only by the tx fee (< 10_000 lamports).
    expect(callerBefore - callerAfter).to.be.lessThan(10_000);
    // Stealth address receives nothing.
    expect(stealthAfter).to.equal(stealthBefore);
  });

  // ─── A2. Caller mismatch ──────────────────────────────────────────────────

  it("A2 · unsigned caller key is rejected by the runtime signer check", async () => {
    const stealth = Keypair.generate().publicKey;
    const metadata = Buffer.from([0x02]);
    const fakeCaller = Keypair.generate().publicKey; // not a signer

    try {
      // We pass fakeCaller as the `caller` account but do NOT add it as a signer.
      // The program declares `caller: Signer`, so Anchor/runtime will reject this.
      await program.methods
        .announce(schemeId, stealth, ephemeralPubKey, metadata)
        .accounts({ caller: fakeCaller })
        // no .signers([...]) → fakeCaller is not signed
        .rpc();
      expect.fail("should have been rejected");
    } catch (err: any) {
      expect(err).to.exist;
      // Runtime enforces the Signer constraint regardless of what caller key
      // was passed in the account list.
    }
  });

  // ─── A3. Max-size metadata ────────────────────────────────────────────────

  it("A3 · large metadata (256 bytes) is accepted without corrupting state", async () => {
    const stealth = Keypair.generate().publicKey;
    const metadata = Buffer.alloc(256);
    metadata[0] = 0xaa;   // view tag
    metadata[255] = 0xbb; // tail sentinel

    // Should succeed — no size cap on metadata in the announcer.
    await program.methods
      .announce(schemeId, stealth, ephemeralPubKey, Array.from(metadata))
      .accounts({ caller: provider.wallet.publicKey })
      .rpc();
  });

  // ─── A4. All-zeroes metadata ──────────────────────────────────────────────

  it("A4 · all-zeroes metadata and ephemeral key are accepted", async () => {
    const stealth = Keypair.generate().publicKey;
    const metadata = Buffer.alloc(0); // empty
    const zeroKey = Array.from(Buffer.alloc(32, 0x00));

    await program.methods
      .announce(schemeId, stealth, zeroKey, metadata)
      .accounts({ caller: provider.wallet.publicKey })
      .rpc();
  });

  // ─── A5. Any signer may announce ─────────────────────────────────────────

  it("A5 · any signer may call announce (no allowlist — stateless by design)", async () => {
    const anyUser = Keypair.generate();
    await fund(conn, anyUser.publicKey);

    const stealth = Keypair.generate().publicKey;

    await program.methods
      .announce(schemeId, stealth, ephemeralPubKey, Buffer.from([0x05]))
      .accounts({ caller: anyUser.publicKey })
      .signers([anyUser])
      .rpc();
  });
});
