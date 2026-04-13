import { expect } from "chai";
import { ethers } from "hardhat";
import { ERC5564Announcer, WraithSender } from "../typechain-types";

describe("WraithSender", function () {
  let announcer: ERC5564Announcer;
  let sender: WraithSender;
  const schemeId = 1;
  const ephemeralPubKey = "0x" + "ab".repeat(33);
  const metadata = "0xff" + "00".repeat(10);

  async function deployToken() {
    const factory = await ethers.getContractFactory(
      "contracts/test/ERC20Mock.sol:ERC20Mock"
    );
    const token = await factory.deploy();
    await token.waitForDeployment();
    return token;
  }

  beforeEach(async function () {
    const AnnouncerFactory =
      await ethers.getContractFactory("ERC5564Announcer");
    announcer = await AnnouncerFactory.deploy();
    await announcer.waitForDeployment();

    const SenderFactory = await ethers.getContractFactory("WraithSender");
    sender = await SenderFactory.deploy(await announcer.getAddress());
    await sender.waitForDeployment();
  });

  describe("sendETH", function () {
    it("should transfer ETH and emit Announcement atomically", async function () {
      const [, recipient] = await ethers.getSigners();
      const stealthAddress = recipient.address;
      const amount = ethers.parseEther("1.0");

      const balanceBefore = await ethers.provider.getBalance(stealthAddress);

      await expect(
        sender.sendETH(schemeId, stealthAddress, ephemeralPubKey, metadata, {
          value: amount,
        })
      )
        .to.emit(announcer, "Announcement")
        .withArgs(
          schemeId,
          stealthAddress,
          await sender.getAddress(),
          ephemeralPubKey,
          metadata
        );

      const balanceAfter = await ethers.provider.getBalance(stealthAddress);
      expect(balanceAfter - balanceBefore).to.equal(amount);
    });
  });

  describe("sendERC20", function () {
    it("should transfer tokens and emit Announcement atomically", async function () {
      const [deployer, recipient] = await ethers.getSigners();
      const stealthAddress = recipient.address;

      const token = await deployToken();
      const tokenAddress = await token.getAddress();
      const senderAddress = await sender.getAddress();

      const amount = ethers.parseEther("100");
      await token.mint(deployer.address, amount);
      await token.approve(senderAddress, amount);

      await expect(
        sender.sendERC20(
          tokenAddress,
          amount,
          schemeId,
          stealthAddress,
          ephemeralPubKey,
          metadata
        )
      ).to.emit(announcer, "Announcement");

      expect(await token.balanceOf(stealthAddress)).to.equal(amount);
    });

    it("should forward ETH gas tip to the stealth address", async function () {
      const [deployer, recipient] = await ethers.getSigners();
      const stealthAddress = recipient.address;

      const token = await deployToken();
      const tokenAddress = await token.getAddress();
      const senderAddress = await sender.getAddress();

      const tokenAmount = ethers.parseEther("100");
      const gasTip = ethers.parseEther("0.005");

      await token.mint(deployer.address, tokenAmount);
      await token.approve(senderAddress, tokenAmount);

      const ethBefore = await ethers.provider.getBalance(stealthAddress);

      await sender.sendERC20(
        tokenAddress,
        tokenAmount,
        schemeId,
        stealthAddress,
        ephemeralPubKey,
        metadata,
        { value: gasTip }
      );

      expect(await token.balanceOf(stealthAddress)).to.equal(tokenAmount);
      const ethAfter = await ethers.provider.getBalance(stealthAddress);
      expect(ethAfter - ethBefore).to.equal(gasTip);
    });

    it("should work without gas tip (zero msg.value)", async function () {
      const [deployer, recipient] = await ethers.getSigners();
      const stealthAddress = recipient.address;

      const token = await deployToken();
      const tokenAddress = await token.getAddress();
      const senderAddress = await sender.getAddress();

      const amount = ethers.parseEther("50");
      await token.mint(deployer.address, amount);
      await token.approve(senderAddress, amount);

      const ethBefore = await ethers.provider.getBalance(stealthAddress);

      await sender.sendERC20(
        tokenAddress,
        amount,
        schemeId,
        stealthAddress,
        ephemeralPubKey,
        metadata
      );

      expect(await token.balanceOf(stealthAddress)).to.equal(amount);
      const ethAfter = await ethers.provider.getBalance(stealthAddress);
      expect(ethAfter).to.equal(ethBefore);
    });
  });

  describe("batchSendETH", function () {
    it("should send ETH to multiple stealth addresses and announce each", async function () {
      const signers = await ethers.getSigners();
      const stealthAddresses = [
        signers[1].address,
        signers[2].address,
        signers[3].address,
      ];
      const amounts = [
        ethers.parseEther("1.0"),
        ethers.parseEther("2.0"),
        ethers.parseEther("0.5"),
      ];
      const totalValue = amounts.reduce((a, b) => a + b, 0n);
      const ephPubKeys = [ephemeralPubKey, ephemeralPubKey, ephemeralPubKey];
      const metas = [metadata, metadata, metadata];

      const balancesBefore = await Promise.all(
        stealthAddresses.map((a) => ethers.provider.getBalance(a))
      );

      const tx = await sender.batchSendETH(
        schemeId,
        stealthAddresses,
        ephPubKeys,
        metas,
        amounts,
        { value: totalValue }
      );
      const receipt = await tx.wait();

      const announcerInterface = announcer.interface;
      const announcementLogs = receipt!.logs.filter((log) => {
        try {
          announcerInterface.parseLog({
            topics: log.topics as string[],
            data: log.data,
          });
          return true;
        } catch {
          return false;
        }
      });
      expect(announcementLogs.length).to.equal(3);

      for (let i = 0; i < stealthAddresses.length; i++) {
        const balanceAfter = await ethers.provider.getBalance(
          stealthAddresses[i]
        );
        expect(balanceAfter - balancesBefore[i]).to.equal(amounts[i]);
      }
    });

    it("should revert if msg.value does not match sum of amounts", async function () {
      const [, r1] = await ethers.getSigners();

      await expect(
        sender.batchSendETH(
          schemeId,
          [r1.address],
          [ephemeralPubKey],
          [metadata],
          [ethers.parseEther("1.0")],
          { value: ethers.parseEther("0.5") }
        )
      ).to.be.reverted;
    });

    it("should revert on array length mismatch", async function () {
      const [, r1, r2] = await ethers.getSigners();

      await expect(
        sender.batchSendETH(
          schemeId,
          [r1.address, r2.address],
          [ephemeralPubKey],
          [metadata, metadata],
          [ethers.parseEther("1.0"), ethers.parseEther("1.0")],
          { value: ethers.parseEther("2.0") }
        )
      ).to.be.revertedWithCustomError(sender, "LengthMismatch");
    });
  });

  describe("batchSendERC20", function () {
    it("should send tokens to multiple stealth addresses and announce each", async function () {
      const signers = await ethers.getSigners();
      const deployer = signers[0];
      const stealthAddresses = [signers[1].address, signers[2].address];
      const amounts = [ethers.parseEther("50"), ethers.parseEther("100")];

      const token = await deployToken();
      const tokenAddress = await token.getAddress();
      const senderAddress = await sender.getAddress();

      const totalAmount = amounts.reduce((a, b) => a + b, 0n);
      await token.mint(deployer.address, totalAmount);
      await token.approve(senderAddress, totalAmount);

      const tx = await sender.batchSendERC20(
        tokenAddress,
        schemeId,
        stealthAddresses,
        [ephemeralPubKey, ephemeralPubKey],
        [metadata, metadata],
        amounts
      );
      const receipt = await tx.wait();

      const announcerAddress = (await announcer.getAddress()).toLowerCase();
      const announcementLogs = receipt!.logs.filter(
        (log) => log.address.toLowerCase() === announcerAddress
      );
      expect(announcementLogs.length).to.equal(2);

      for (let i = 0; i < stealthAddresses.length; i++) {
        expect(await token.balanceOf(stealthAddresses[i])).to.equal(
          amounts[i]
        );
      }
    });

    it("should split ETH gas tip equally across all stealth addresses", async function () {
      const signers = await ethers.getSigners();
      const deployer = signers[0];
      const stealthAddresses = [signers[4].address, signers[5].address];
      const amounts = [ethers.parseEther("50"), ethers.parseEther("100")];
      const tipPerRecipient = ethers.parseEther("0.005");
      const totalTip = tipPerRecipient * 2n;

      const token = await deployToken();
      const tokenAddress = await token.getAddress();
      const senderAddress = await sender.getAddress();

      const totalAmount = amounts.reduce((a, b) => a + b, 0n);
      await token.mint(deployer.address, totalAmount);
      await token.approve(senderAddress, totalAmount);

      const ethBefore = await Promise.all(
        stealthAddresses.map((a) => ethers.provider.getBalance(a))
      );

      await sender.batchSendERC20(
        tokenAddress,
        schemeId,
        stealthAddresses,
        [ephemeralPubKey, ephemeralPubKey],
        [metadata, metadata],
        amounts,
        { value: totalTip }
      );

      for (let i = 0; i < stealthAddresses.length; i++) {
        expect(await token.balanceOf(stealthAddresses[i])).to.equal(
          amounts[i]
        );
        const ethAfter = await ethers.provider.getBalance(
          stealthAddresses[i]
        );
        expect(ethAfter - ethBefore[i]).to.equal(tipPerRecipient);
      }
    });

    it("should work without gas tip (zero msg.value)", async function () {
      const signers = await ethers.getSigners();
      const deployer = signers[0];
      const stealthAddresses = [signers[6].address];
      const amounts = [ethers.parseEther("25")];

      const token = await deployToken();
      const tokenAddress = await token.getAddress();
      const senderAddress = await sender.getAddress();

      await token.mint(deployer.address, amounts[0]);
      await token.approve(senderAddress, amounts[0]);

      const ethBefore = await ethers.provider.getBalance(
        stealthAddresses[0]
      );

      await sender.batchSendERC20(
        tokenAddress,
        schemeId,
        stealthAddresses,
        [ephemeralPubKey],
        [metadata],
        amounts
      );

      expect(await token.balanceOf(stealthAddresses[0])).to.equal(amounts[0]);
      const ethAfter = await ethers.provider.getBalance(stealthAddresses[0]);
      expect(ethAfter).to.equal(ethBefore);
    });
  });
});
