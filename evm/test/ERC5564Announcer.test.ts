import { expect } from "chai";
import { ethers } from "hardhat";
import { ERC5564Announcer } from "../typechain-types";

describe("ERC5564Announcer", function () {
  let announcer: ERC5564Announcer;

  beforeEach(async function () {
    const factory = await ethers.getContractFactory("ERC5564Announcer");
    announcer = await factory.deploy();
    await announcer.waitForDeployment();
  });

  it("should emit Announcement event with correct parameters", async function () {
    const [caller] = await ethers.getSigners();
    const schemeId = 1;
    const stealthAddress = "0x1234567890AbcdEF1234567890aBcdef12345678";
    const ephemeralPubKey = "0x" + "ab".repeat(33);
    const metadata = "0xff" + "00".repeat(56);

    await expect(
      announcer.announce(schemeId, stealthAddress, ephemeralPubKey, metadata)
    )
      .to.emit(announcer, "Announcement")
      .withArgs(
        schemeId,
        stealthAddress,
        caller.address,
        ephemeralPubKey,
        metadata
      );
  });

  it("should allow multiple announcements from different callers", async function () {
    const [caller1, caller2] = await ethers.getSigners();
    const schemeId = 1;
    const stealthAddress1 = "0x1111111111111111111111111111111111111111";
    const stealthAddress2 = "0x2222222222222222222222222222222222222222";
    const ephemeralPubKey = "0x" + "ab".repeat(33);
    const metadata = "0xff";

    await expect(
      announcer.announce(schemeId, stealthAddress1, ephemeralPubKey, metadata)
    )
      .to.emit(announcer, "Announcement")
      .withArgs(
        schemeId,
        stealthAddress1,
        caller1.address,
        ephemeralPubKey,
        metadata
      );

    await expect(
      announcer
        .connect(caller2)
        .announce(schemeId, stealthAddress2, ephemeralPubKey, metadata)
    )
      .to.emit(announcer, "Announcement")
      .withArgs(
        schemeId,
        stealthAddress2,
        caller2.address,
        ephemeralPubKey,
        metadata
      );
  });

  it("should preserve metadata bytes", async function () {
    const schemeId = 1;
    const stealthAddress = "0x1234567890AbcdEF1234567890aBcdef12345678";
    const ephemeralPubKey = "0x" + "ab".repeat(33);
    // View tag (0xaa) + function selector + token address + amount
    const metadata =
      "0xaa" +
      "a9059cbb" +
      "0000000000000000000000001234567890abcdef1234567890abcdef12345678" +
      "0000000000000000000000000000000000000000000000000de0b6b3a7640000";

    const tx = await announcer.announce(
      schemeId,
      stealthAddress,
      ephemeralPubKey,
      metadata
    );
    const receipt = await tx.wait();
    const event = receipt!.logs[0];
    const decoded = announcer.interface.parseLog({
      topics: event.topics as string[],
      data: event.data,
    });

    expect(decoded!.args.metadata).to.equal(metadata);
  });
});
