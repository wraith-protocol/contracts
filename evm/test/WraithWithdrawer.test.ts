import { expect } from "chai";
import { ethers } from "hardhat";
import { WraithWithdrawer } from "../typechain-types";

describe("WraithWithdrawer", function () {
  let withdrawer: WraithWithdrawer;

  beforeEach(async function () {
    const factory = await ethers.getContractFactory("WraithWithdrawer");
    withdrawer = await factory.deploy();
    await withdrawer.waitForDeployment();
  });

  it("should compile and deploy successfully", async function () {
    const address = await withdrawer.getAddress();
    expect(address).to.be.properAddress;
  });

  it("should revert withdrawETHDirect with no balance", async function () {
    const [, dest] = await ethers.getSigners();
    await expect(
      withdrawer.withdrawETHDirect(dest.address)
    ).to.be.revertedWithCustomError(withdrawer, "InsufficientBalance");
  });

  it("should revert withdrawERC20Direct with no balance", async function () {
    const [, dest] = await ethers.getSigners();
    const TokenFactory = await ethers.getContractFactory(
      "contracts/test/ERC20Mock.sol:ERC20Mock"
    );
    const token = await TokenFactory.deploy();
    await token.waitForDeployment();

    await expect(
      withdrawer.withdrawERC20Direct(await token.getAddress(), dest.address)
    ).to.be.revertedWithCustomError(withdrawer, "InsufficientBalance");
  });

  it("should revert withdrawETH if sponsorFee >= balance", async function () {
    const [, dest] = await ethers.getSigners();
    await expect(
      withdrawer.withdrawETH(dest.address, ethers.parseEther("1"))
    ).to.be.revertedWithCustomError(withdrawer, "InsufficientBalance");
  });
});
