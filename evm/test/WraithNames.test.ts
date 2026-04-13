import { expect } from "chai";
import { ethers } from "hardhat";
import { WraithNames } from "../typechain-types";

describe("WraithNames", function () {
  let names: WraithNames;

  // Generate a deterministic stealth meta-address from a known spending key
  // The meta-address is spendingPubKey (33 bytes) + viewingPubKey (33 bytes) = 66 bytes
  async function makeMetaAddress(spendingPrivKey: string) {
    const spendingWallet = new ethers.Wallet(spendingPrivKey);
    // Use a different key for viewing
    const viewingPrivKey = ethers.keccak256(ethers.toUtf8Bytes("viewing-" + spendingPrivKey));
    const viewingWallet = new ethers.Wallet(viewingPrivKey);

    const spendingPub = spendingWallet.signingKey.compressedPublicKey;
    const viewingPub = viewingWallet.signingKey.compressedPublicKey;

    // 0x + 66 hex (spending) + 66 hex (viewing) = 0x + 132 hex = 66 bytes
    const metaAddress = spendingPub + viewingPub.slice(2);
    return { metaAddress, spendingWallet, spendingPub };
  }

  async function signRegistration(
    wallet: ethers.Wallet,
    name: string,
    metaAddress: string
  ) {
    const digest = ethers.keccak256(
      ethers.solidityPacked(["string", "bytes"], [name, metaAddress])
    );
    return wallet.signMessage(ethers.getBytes(digest));
  }

  async function signRegistrationOnBehalf(
    wallet: ethers.Wallet,
    name: string,
    metaAddress: string,
    nonce: bigint
  ) {
    const digest = ethers.keccak256(
      ethers.solidityPacked(["string", "bytes", "uint256"], [name, metaAddress, nonce])
    );
    return wallet.signMessage(ethers.getBytes(digest));
  }

  const spendingKey1 = "0x1111111111111111111111111111111111111111111111111111111111111111";
  const spendingKey2 = "0x2222222222222222222222222222222222222222222222222222222222222222";

  beforeEach(async function () {
    const factory = await ethers.getContractFactory("WraithNames");
    names = await factory.deploy();
    await names.waitForDeployment();
  });

  describe("register", function () {
    it("should register a name and resolve it", async function () {
      const { metaAddress, spendingWallet } = await makeMetaAddress(spendingKey1);
      const sig = await signRegistration(spendingWallet, "truth", metaAddress);

      await names.register("truth", metaAddress, sig);

      const resolved = await names.resolve("truth");
      expect(resolved.toLowerCase()).to.equal(metaAddress.toLowerCase());
    });

    it("should emit NameRegistered event", async function () {
      const { metaAddress, spendingWallet } = await makeMetaAddress(spendingKey1);
      const sig = await signRegistration(spendingWallet, "alice", metaAddress);

      await expect(names.register("alice", metaAddress, sig))
        .to.emit(names, "NameRegistered");
    });

    it("should support reverse lookup", async function () {
      const { metaAddress, spendingWallet } = await makeMetaAddress(spendingKey1);
      const sig = await signRegistration(spendingWallet, "bob", metaAddress);

      await names.register("bob", metaAddress, sig);

      const name = await names.nameOf(metaAddress);
      expect(name).to.equal("bob");
    });

    it("should reject duplicate names", async function () {
      const { metaAddress, spendingWallet } = await makeMetaAddress(spendingKey1);
      const sig = await signRegistration(spendingWallet, "taken", metaAddress);
      await names.register("taken", metaAddress, sig);

      const m2 = await makeMetaAddress(spendingKey2);
      const sig2 = await signRegistration(m2.spendingWallet, "taken", m2.metaAddress);

      await expect(names.register("taken", m2.metaAddress, sig2))
        .to.be.revertedWithCustomError(names, "NameTaken");
    });

    it("should reject names shorter than 3 characters", async function () {
      const { metaAddress, spendingWallet } = await makeMetaAddress(spendingKey1);
      const sig = await signRegistration(spendingWallet, "ab", metaAddress);

      await expect(names.register("ab", metaAddress, sig))
        .to.be.revertedWithCustomError(names, "NameTooShort");
    });

    it("should reject names with uppercase or special characters", async function () {
      const { metaAddress, spendingWallet } = await makeMetaAddress(spendingKey1);
      const sig = await signRegistration(spendingWallet, "Hello", metaAddress);

      await expect(names.register("Hello", metaAddress, sig))
        .to.be.revertedWithCustomError(names, "InvalidNameCharacter");
    });

    it("should reject invalid signature (wrong signer)", async function () {
      const { metaAddress } = await makeMetaAddress(spendingKey1);
      const wrongWallet = new ethers.Wallet(spendingKey2);
      const sig = await signRegistration(wrongWallet, "hacker", metaAddress);

      await expect(names.register("hacker", metaAddress, sig))
        .to.be.revertedWithCustomError(names, "InvalidSignature");
    });
  });

  describe("registerOnBehalf", function () {
    it("should register via relayer with nonce-protected signature", async function () {
      const { metaAddress, spendingWallet } = await makeMetaAddress(spendingKey1);
      const [, relayer] = await ethers.getSigners();

      const sig = await signRegistrationOnBehalf(spendingWallet, "relayed", metaAddress, 0n);

      await names.connect(relayer).registerOnBehalf("relayed", metaAddress, sig);

      const resolved = await names.resolve("relayed");
      expect(resolved.toLowerCase()).to.equal(metaAddress.toLowerCase());
    });

    it("should prevent replay with nonce", async function () {
      const { metaAddress, spendingWallet } = await makeMetaAddress(spendingKey1);
      const [, relayer] = await ethers.getSigners();

      const sig = await signRegistrationOnBehalf(spendingWallet, "replay", metaAddress, 0n);
      await names.connect(relayer).registerOnBehalf("replay", metaAddress, sig);

      const m2 = await makeMetaAddress(spendingKey1);
      // Same nonce (0) should fail because nonce incremented
      const sig2 = await signRegistrationOnBehalf(spendingWallet, "replay2", m2.metaAddress, 0n);
      await expect(names.connect(relayer).registerOnBehalf("replay2", m2.metaAddress, sig2))
        .to.be.revertedWithCustomError(names, "InvalidSignature");
    });
  });

  describe("update", function () {
    it("should update the meta-address for an existing name", async function () {
      const { metaAddress, spendingWallet } = await makeMetaAddress(spendingKey1);
      const sig = await signRegistration(spendingWallet, "updatable", metaAddress);
      await names.register("updatable", metaAddress, sig);

      const m2 = await makeMetaAddress(spendingKey2);
      const updateSig = await signRegistration(spendingWallet, "updatable", m2.metaAddress);
      await names.update("updatable", m2.metaAddress, updateSig);

      const resolved = await names.resolve("updatable");
      expect(resolved.toLowerCase()).to.equal(m2.metaAddress.toLowerCase());
    });

    it("should reject update from non-owner", async function () {
      const { metaAddress, spendingWallet } = await makeMetaAddress(spendingKey1);
      const sig = await signRegistration(spendingWallet, "owned", metaAddress);
      await names.register("owned", metaAddress, sig);

      const m2 = await makeMetaAddress(spendingKey2);
      const wrongSig = await signRegistration(m2.spendingWallet, "owned", m2.metaAddress);

      await expect(names.update("owned", m2.metaAddress, wrongSig))
        .to.be.revertedWithCustomError(names, "NotOwner");
    });
  });

  describe("release", function () {
    it("should release a name and allow re-registration", async function () {
      const { metaAddress, spendingWallet } = await makeMetaAddress(spendingKey1);
      const sig = await signRegistration(spendingWallet, "temporary", metaAddress);
      await names.register("temporary", metaAddress, sig);

      const releaseSig = await (async () => {
        const digest = ethers.keccak256(
          ethers.solidityPacked(["string"], ["temporary"])
        );
        return spendingWallet.signMessage(ethers.getBytes(digest));
      })();

      await expect(names.release("temporary", releaseSig))
        .to.emit(names, "NameReleased");

      const resolved = await names.resolve("temporary");
      expect(resolved).to.equal("0x");

      // Re-register with different meta-address
      const m2 = await makeMetaAddress(spendingKey2);
      const sig2 = await signRegistration(m2.spendingWallet, "temporary", m2.metaAddress);
      await names.register("temporary", m2.metaAddress, sig2);

      const resolved2 = await names.resolve("temporary");
      expect(resolved2.toLowerCase()).to.equal(m2.metaAddress.toLowerCase());
    });
  });

  describe("resolve", function () {
    it("should return empty bytes for unregistered names", async function () {
      const resolved = await names.resolve("nonexistent");
      expect(resolved).to.equal("0x");
    });
  });

  describe("nameOf", function () {
    it("should return empty string for unregistered meta-addresses", async function () {
      const { metaAddress } = await makeMetaAddress(spendingKey1);
      const name = await names.nameOf(metaAddress);
      expect(name).to.equal("");
    });
  });
});
