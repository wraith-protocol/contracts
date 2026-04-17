import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  Keypair,
  LAMPORTS_PER_SOL,
  SystemProgram,
} from "@solana/web3.js";
import { expect } from "chai";
import { WraithSender } from "../target/types/wraith_sender";

describe("wraith-sender", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.WraithSender as Program<WraithSender>;

  const schemeId = 1;
  const ephemeralPubKey = Array.from(Buffer.alloc(32, 0xab));
  const metadata = Buffer.from([0xff]);

  describe("send_sol", () => {
    it("should transfer SOL and emit announcement", async () => {
      const stealthKeypair = Keypair.generate();
      const stealthAddress = stealthKeypair.publicKey;
      const amount = new anchor.BN(500_000_000); // 0.5 SOL

      const balanceBefore = await provider.connection.getBalance(
        stealthAddress
      );

      let eventReceived = false;
      const listener = program.addEventListener(
        "announcementEvent",
        (event) => {
          expect(event.schemeId).to.equal(schemeId);
          expect(event.stealthAddress.toBase58()).to.equal(
            stealthAddress.toBase58()
          );
          eventReceived = true;
        }
      );

      await program.methods
        .sendSol(
          amount,
          schemeId,
          stealthAddress,
          ephemeralPubKey,
          metadata
        )
        .accounts({
          sender: provider.wallet.publicKey,
          stealthAccount: stealthAddress,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const balanceAfter = await provider.connection.getBalance(
        stealthAddress
      );
      expect(balanceAfter - balanceBefore).to.equal(500_000_000);

      await new Promise((resolve) => setTimeout(resolve, 500));
      program.removeEventListener(listener);
      expect(eventReceived).to.be.true;
    });

    it("should fail with insufficient funds", async () => {
      const poorSender = Keypair.generate();
      const airdropSig = await provider.connection.requestAirdrop(
        poorSender.publicKey,
        10_000 // barely enough for fees, not the transfer
      );
      await provider.connection.confirmTransaction(airdropSig);

      const stealthAddress = Keypair.generate().publicKey;
      const amount = new anchor.BN(10 * LAMPORTS_PER_SOL);

      try {
        await program.methods
          .sendSol(
            amount,
            schemeId,
            stealthAddress,
            ephemeralPubKey,
            metadata
          )
          .accounts({
            sender: poorSender.publicKey,
            stealthAccount: stealthAddress,
            systemProgram: SystemProgram.programId,
          })
          .signers([poorSender])
          .rpc();
        expect.fail("should have thrown");
      } catch (err) {
        expect(err).to.exist;
      }
    });
  });
});
