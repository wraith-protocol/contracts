import { ethers } from "hardhat";

async function main() {
  const [deployer] = await ethers.getSigners();
  console.log("Deploying contracts with:", deployer.address);

  const Announcer = await ethers.getContractFactory("ERC5564Announcer");
  const announcer = await Announcer.deploy();
  await announcer.waitForDeployment();
  console.log("ERC5564Announcer:", await announcer.getAddress());

  const Registry = await ethers.getContractFactory("ERC6538Registry");
  const registry = await Registry.deploy();
  await registry.waitForDeployment();
  console.log("ERC6538Registry:", await registry.getAddress());

  const Sender = await ethers.getContractFactory("WraithSender");
  const sender = await Sender.deploy(await announcer.getAddress());
  await sender.waitForDeployment();
  console.log("WraithSender:", await sender.getAddress());

  const Names = await ethers.getContractFactory("WraithNames");
  const names = await Names.deploy();
  await names.waitForDeployment();
  console.log("WraithNames:", await names.getAddress());

  const Withdrawer = await ethers.getContractFactory("WraithWithdrawer");
  const withdrawer = await Withdrawer.deploy();
  await withdrawer.waitForDeployment();
  console.log("WraithWithdrawer:", await withdrawer.getAddress());
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
