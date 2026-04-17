import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey } from "@solana/web3.js";
import { expect } from "chai";
import { WraithAnnouncer } from "../target/types/wraith_announcer";

describe("wraith-announcer", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace
    .WraithAnnouncer as Program<WraithAnnouncer>;

  const schemeId = 1;
  const stealthAddress = Keypair.generate().publicKey;
  const ephemeralPubKey = Array.from(Buffer.alloc(32, 0xab));
  const metadata = Buffer.from([0xff, 0x00, 0x01]);

  it("should emit AnnouncementEvent on announce", async () => {
    const listener = program.addEventListener(
      "announcementEvent",
      (event) => {
        expect(event.schemeId).to.equal(schemeId);
        expect(event.stealthAddress.toBase58()).to.equal(
          stealthAddress.toBase58()
        );
        expect(event.caller.toBase58()).to.equal(
          provider.wallet.publicKey.toBase58()
        );
        expect(Buffer.from(event.ephemeralPubKey)).to.deep.equal(
          Buffer.from(ephemeralPubKey)
        );
        expect(Buffer.from(event.metadata)).to.deep.equal(metadata);
      }
    );

    await program.methods
      .announce(schemeId, stealthAddress, ephemeralPubKey, metadata)
      .accounts({ caller: provider.wallet.publicKey })
      .rpc();

    // Give time for event listener to fire
    await new Promise((resolve) => setTimeout(resolve, 500));
    program.removeEventListener(listener);
  });

  it("should allow multiple announcements from different callers", async () => {
    const caller2 = Keypair.generate();
    const airdropSig = await provider.connection.requestAirdrop(
      caller2.publicKey,
      1_000_000_000
    );
    await provider.connection.confirmTransaction(airdropSig);

    const stealth1 = Keypair.generate().publicKey;
    const stealth2 = Keypair.generate().publicKey;

    await program.methods
      .announce(schemeId, stealth1, ephemeralPubKey, metadata)
      .accounts({ caller: provider.wallet.publicKey })
      .rpc();

    await program.methods
      .announce(schemeId, stealth2, ephemeralPubKey, metadata)
      .accounts({ caller: caller2.publicKey })
      .signers([caller2])
      .rpc();
  });

  it("should preserve metadata bytes", async () => {
    const longMetadata = Buffer.alloc(64, 0);
    longMetadata[0] = 0xaa;
    longMetadata[1] = 0xbb;
    longMetadata[63] = 0xcc;

    let receivedMetadata: Buffer | null = null;
    const listener = program.addEventListener(
      "announcementEvent",
      (event) => {
        receivedMetadata = Buffer.from(event.metadata);
      }
    );

    await program.methods
      .announce(schemeId, stealthAddress, ephemeralPubKey, longMetadata)
      .accounts({ caller: provider.wallet.publicKey })
      .rpc();

    await new Promise((resolve) => setTimeout(resolve, 500));
    program.removeEventListener(listener);

    expect(receivedMetadata).to.not.be.null;
    expect(receivedMetadata!).to.deep.equal(longMetadata);
  });
});
