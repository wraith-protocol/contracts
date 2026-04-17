import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { expect } from "chai";
import { WraithNames } from "../target/types/wraith_names";

describe("wraith-names", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.WraithNames as Program<WraithNames>;

  function makeMetaAddress(): number[] {
    return Array.from(Buffer.alloc(64, 0).map((_, i) => i + 1));
  }

  function findNamePda(name: string): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("name"), Buffer.from(name)],
      program.programId
    );
  }

  describe("register", () => {
    it("should register a name and resolve it", async () => {
      const name = "alice";
      const metaAddress = makeMetaAddress();
      const [nameRecord] = findNamePda(name);

      await program.methods
        .register(name, metaAddress)
        .accounts({
          nameRecord,
          owner: provider.wallet.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const result = await program.methods
        .resolve()
        .accounts({ nameRecord })
        .view();

      expect(Array.from(result)).to.deep.equal(metaAddress);
    });

    it("should store correct owner", async () => {
      const name = "bob123";
      const metaAddress = makeMetaAddress();
      const [nameRecord] = findNamePda(name);

      await program.methods
        .register(name, metaAddress)
        .accounts({
          nameRecord,
          owner: provider.wallet.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const account = await program.account.nameRecord.fetch(nameRecord);
      expect(account.owner.toBase58()).to.equal(
        provider.wallet.publicKey.toBase58()
      );
      expect(account.name).to.equal(name);
    });

    it("should reject names shorter than 3 characters", async () => {
      const name = "ab";
      const metaAddress = makeMetaAddress();
      const [nameRecord] = findNamePda(name);

      try {
        await program.methods
          .register(name, metaAddress)
          .accounts({
            nameRecord,
            owner: provider.wallet.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .rpc();
        expect.fail("should have thrown");
      } catch (err: any) {
        expect(err.error.errorCode.code).to.equal("InvalidNameLength");
      }
    });

    it("should reject names with uppercase characters", async () => {
      const name = "Hello";
      const metaAddress = makeMetaAddress();
      const [nameRecord] = findNamePda(name);

      try {
        await program.methods
          .register(name, metaAddress)
          .accounts({
            nameRecord,
            owner: provider.wallet.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .rpc();
        expect.fail("should have thrown");
      } catch (err: any) {
        expect(err.error.errorCode.code).to.equal("InvalidNameCharacter");
      }
    });

    it("should reject duplicate names", async () => {
      const name = "unique1";
      const metaAddress = makeMetaAddress();
      const [nameRecord] = findNamePda(name);

      await program.methods
        .register(name, metaAddress)
        .accounts({
          nameRecord,
          owner: provider.wallet.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      // Second registration of same name should fail (PDA already initialized)
      try {
        await program.methods
          .register(name, metaAddress)
          .accounts({
            nameRecord,
            owner: provider.wallet.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .rpc();
        expect.fail("should have thrown");
      } catch (err) {
        expect(err).to.exist;
      }
    });
  });

  describe("update", () => {
    it("should update meta-address by owner", async () => {
      const name = "updatable";
      const metaAddress = makeMetaAddress();
      const newMeta = Array.from(Buffer.alloc(64, 0xff));
      const [nameRecord] = findNamePda(name);

      await program.methods
        .register(name, metaAddress)
        .accounts({
          nameRecord,
          owner: provider.wallet.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      await program.methods
        .update(newMeta)
        .accounts({
          nameRecord,
          owner: provider.wallet.publicKey,
        })
        .rpc();

      const result = await program.methods
        .resolve()
        .accounts({ nameRecord })
        .view();

      expect(Array.from(result)).to.deep.equal(newMeta);
    });

    it("should reject update from non-owner", async () => {
      const name = "protected";
      const metaAddress = makeMetaAddress();
      const [nameRecord] = findNamePda(name);

      await program.methods
        .register(name, metaAddress)
        .accounts({
          nameRecord,
          owner: provider.wallet.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const nonOwner = Keypair.generate();
      const airdropSig = await provider.connection.requestAirdrop(
        nonOwner.publicKey,
        1_000_000_000
      );
      await provider.connection.confirmTransaction(airdropSig);

      const newMeta = Array.from(Buffer.alloc(64, 0xaa));
      try {
        await program.methods
          .update(newMeta)
          .accounts({
            nameRecord,
            owner: nonOwner.publicKey,
          })
          .signers([nonOwner])
          .rpc();
        expect.fail("should have thrown");
      } catch (err: any) {
        expect(err.error.errorCode.code).to.equal("NotOwner");
      }
    });
  });

  describe("release", () => {
    it("should release a name and allow re-registration", async () => {
      const name = "temporary";
      const metaAddress = makeMetaAddress();
      const [nameRecord] = findNamePda(name);

      await program.methods
        .register(name, metaAddress)
        .accounts({
          nameRecord,
          owner: provider.wallet.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      await program.methods
        .release()
        .accounts({
          nameRecord,
          owner: provider.wallet.publicKey,
        })
        .rpc();

      // Account should be closed
      const info = await provider.connection.getAccountInfo(nameRecord);
      expect(info).to.be.null;

      // Re-register should work
      const newMeta = Array.from(Buffer.alloc(64, 0xbb));
      await program.methods
        .register(name, newMeta)
        .accounts({
          nameRecord,
          owner: provider.wallet.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const result = await program.methods
        .resolve()
        .accounts({ nameRecord })
        .view();

      expect(Array.from(result)).to.deep.equal(newMeta);
    });
  });
});
