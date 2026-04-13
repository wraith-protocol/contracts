import { expect } from "chai";
import { ethers } from "hardhat";
import { ERC6538Registry } from "../typechain-types";

describe("ERC6538Registry", function () {
  let registry: ERC6538Registry;
  const SCHEME_ID = 1;
  const stealthMetaAddress = "0x" + "ab".repeat(66); // 66 bytes (two 33-byte compressed pubkeys)

  beforeEach(async function () {
    const factory = await ethers.getContractFactory("ERC6538Registry");
    registry = await factory.deploy();
    await registry.waitForDeployment();
  });

  describe("registerKeys", function () {
    it("should register and retrieve a stealth meta-address", async function () {
      const [registrant] = await ethers.getSigners();

      await registry.registerKeys(SCHEME_ID, stealthMetaAddress);

      const result = await registry.stealthMetaAddressOf(
        registrant.address,
        SCHEME_ID
      );
      expect(result).to.equal(stealthMetaAddress);
    });

    it("should emit StealthMetaAddressSet event", async function () {
      const [registrant] = await ethers.getSigners();

      await expect(registry.registerKeys(SCHEME_ID, stealthMetaAddress))
        .to.emit(registry, "StealthMetaAddressSet")
        .withArgs(registrant.address, SCHEME_ID, stealthMetaAddress);
    });

    it("should allow overwriting a previously registered meta-address", async function () {
      const [registrant] = await ethers.getSigners();
      const newMetaAddress = "0x" + "cd".repeat(66);

      await registry.registerKeys(SCHEME_ID, stealthMetaAddress);
      await registry.registerKeys(SCHEME_ID, newMetaAddress);

      const result = await registry.stealthMetaAddressOf(
        registrant.address,
        SCHEME_ID
      );
      expect(result).to.equal(newMetaAddress);
    });
  });

  describe("stealthMetaAddressOf", function () {
    it("should return empty bytes for unregistered addresses", async function () {
      const [, unregistered] = await ethers.getSigners();

      const result = await registry.stealthMetaAddressOf(
        unregistered.address,
        SCHEME_ID
      );
      expect(result).to.equal("0x");
    });
  });

  describe("registerKeysOnBehalf", function () {
    async function getSignature(
      registry: ERC6538Registry,
      signer: Awaited<ReturnType<typeof ethers.getSigners>>[number],
      schemeId: number,
      metaAddress: string
    ) {
      const registryAddress = await registry.getAddress();
      const nonce = await registry.nonceOf(signer.address);
      const chainId = (await ethers.provider.getNetwork()).chainId;

      const domain = {
        name: "ERC6538Registry",
        version: "1",
        chainId,
        verifyingContract: registryAddress as `0x${string}`,
      };

      const types = {
        Erc6538RegistryEntry: [
          { name: "schemeId", type: "uint256" },
          { name: "stealthMetaAddress", type: "bytes" },
          { name: "nonce", type: "uint256" },
        ],
      };

      const value = {
        schemeId,
        stealthMetaAddress: metaAddress,
        nonce,
      };

      return signer.signTypedData(domain, types, value);
    }

    it("should register keys with a valid EIP-712 signature", async function () {
      const [, registrant, relayer] = await ethers.getSigners();

      const signature = await getSignature(
        registry,
        registrant,
        SCHEME_ID,
        stealthMetaAddress
      );

      await expect(
        registry
          .connect(relayer)
          .registerKeysOnBehalf(
            registrant.address,
            SCHEME_ID,
            signature,
            stealthMetaAddress
          )
      )
        .to.emit(registry, "StealthMetaAddressSet")
        .withArgs(registrant.address, SCHEME_ID, stealthMetaAddress);

      const result = await registry.stealthMetaAddressOf(
        registrant.address,
        SCHEME_ID
      );
      expect(result).to.equal(stealthMetaAddress);
    });

    it("should revert with invalid signature", async function () {
      const [, registrant, relayer] = await ethers.getSigners();

      // Use a random invalid signature
      const invalidSignature = "0x" + "00".repeat(65);

      await expect(
        registry
          .connect(relayer)
          .registerKeysOnBehalf(
            registrant.address,
            SCHEME_ID,
            invalidSignature,
            stealthMetaAddress
          )
      ).to.be.revertedWithCustomError(
        registry,
        "ERC6538Registry__InvalidSignature"
      );
    });

    it("should revert when signature is from wrong signer", async function () {
      const [wrongSigner, registrant, relayer] = await ethers.getSigners();

      // Sign with the wrong account
      const signature = await getSignature(
        registry,
        wrongSigner,
        SCHEME_ID,
        stealthMetaAddress
      );

      await expect(
        registry
          .connect(relayer)
          .registerKeysOnBehalf(
            registrant.address,
            SCHEME_ID,
            signature,
            stealthMetaAddress
          )
      ).to.be.revertedWithCustomError(
        registry,
        "ERC6538Registry__InvalidSignature"
      );
    });

    it("should prevent replay attacks (nonce increments)", async function () {
      const [, registrant, relayer] = await ethers.getSigners();

      const signature = await getSignature(
        registry,
        registrant,
        SCHEME_ID,
        stealthMetaAddress
      );

      // First use succeeds
      await registry
        .connect(relayer)
        .registerKeysOnBehalf(
          registrant.address,
          SCHEME_ID,
          signature,
          stealthMetaAddress
        );

      // Same signature should fail (nonce incremented)
      await expect(
        registry
          .connect(relayer)
          .registerKeysOnBehalf(
            registrant.address,
            SCHEME_ID,
            signature,
            stealthMetaAddress
          )
      ).to.be.revertedWithCustomError(
        registry,
        "ERC6538Registry__InvalidSignature"
      );
    });

    it("should increment nonce after successful registration", async function () {
      const [, registrant, relayer] = await ethers.getSigners();

      const nonceBefore = await registry.nonceOf(registrant.address);
      expect(nonceBefore).to.equal(0);

      const signature = await getSignature(
        registry,
        registrant,
        SCHEME_ID,
        stealthMetaAddress
      );

      await registry
        .connect(relayer)
        .registerKeysOnBehalf(
          registrant.address,
          SCHEME_ID,
          signature,
          stealthMetaAddress
        );

      const nonceAfter = await registry.nonceOf(registrant.address);
      expect(nonceAfter).to.equal(1);
    });
  });

  describe("incrementNonce", function () {
    it("should increment the caller's nonce", async function () {
      const [caller] = await ethers.getSigners();

      const nonceBefore = await registry.nonceOf(caller.address);
      expect(nonceBefore).to.equal(0);

      await registry.incrementNonce();

      const nonceAfter = await registry.nonceOf(caller.address);
      expect(nonceAfter).to.equal(1);
    });

    it("should emit NonceIncremented event", async function () {
      const [caller] = await ethers.getSigners();

      await expect(registry.incrementNonce())
        .to.emit(registry, "NonceIncremented")
        .withArgs(caller.address, 1);
    });

    it("should invalidate outstanding signatures", async function () {
      const [, registrant, relayer] = await ethers.getSigners();

      // Get signature with current nonce (0)
      const signature = await (async () => {
        const registryAddress = await registry.getAddress();
        const chainId = (await ethers.provider.getNetwork()).chainId;
        return registrant.signTypedData(
          {
            name: "ERC6538Registry",
            version: "1",
            chainId,
            verifyingContract: registryAddress as `0x${string}`,
          },
          {
            Erc6538RegistryEntry: [
              { name: "schemeId", type: "uint256" },
              { name: "stealthMetaAddress", type: "bytes" },
              { name: "nonce", type: "uint256" },
            ],
          },
          {
            schemeId: SCHEME_ID,
            stealthMetaAddress: stealthMetaAddress,
            nonce: 0,
          }
        );
      })();

      // Registrant increments nonce
      await registry.connect(registrant).incrementNonce();

      // Old signature should now be invalid
      await expect(
        registry
          .connect(relayer)
          .registerKeysOnBehalf(
            registrant.address,
            SCHEME_ID,
            signature,
            stealthMetaAddress
          )
      ).to.be.revertedWithCustomError(
        registry,
        "ERC6538Registry__InvalidSignature"
      );
    });
  });

  describe("DOMAIN_SEPARATOR", function () {
    it("should return a valid domain separator", async function () {
      const domainSeparator = await registry.DOMAIN_SEPARATOR();
      expect(domainSeparator).to.not.equal(ethers.ZeroHash);
    });
  });
});
