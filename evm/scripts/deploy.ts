import { ethers } from 'hardhat';
import * as fs from 'node:fs';
import * as path from 'node:path';

declare const hre: any;

async function main() {
  const [deployer] = await ethers.getSigners();
  console.log('Deploying contracts with:', deployer.address);

  const Announcer = await ethers.getContractFactory('ERC5564Announcer');
  const announcer = await Announcer.deploy();
  await announcer.waitForDeployment();
  const announcerAddress = await announcer.getAddress();
  console.log('ERC5564Announcer:', announcerAddress);

  const Registry = await ethers.getContractFactory('ERC6538Registry');
  const registry = await Registry.deploy();
  await registry.waitForDeployment();
  const registryAddress = await registry.getAddress();
  console.log('ERC6538Registry:', registryAddress);

  const Sender = await ethers.getContractFactory('WraithSender');
  const sender = await Sender.deploy(announcerAddress);
  await sender.waitForDeployment();
  const senderAddress = await sender.getAddress();
  console.log('WraithSender:', senderAddress);

  const Names = await ethers.getContractFactory('WraithNames');
  const names = await Names.deploy();
  await names.waitForDeployment();
  const namesAddress = await names.getAddress();
  console.log('WraithNames:', namesAddress);

  const Withdrawer = await ethers.getContractFactory('WraithWithdrawer');
  const withdrawer = await Withdrawer.deploy();
  await withdrawer.waitForDeployment();
  const withdrawerAddress = await withdrawer.getAddress();
  console.log('WraithWithdrawer:', withdrawerAddress);

  // Get deployment block number
  const deploymentBlock = await ethers.provider.getBlockNumber();
  console.log('Deployment block:', deploymentBlock);

  // Update subgraph config
  const network = hre.network.name;
  const configPath = path.join(__dirname, '../subgraph', network, 'instant-config.json');

  if (fs.existsSync(configPath)) {
    const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));

    const addressMap: Record<string, string> = {
      ERC5564Announcer: announcerAddress,
      ERC6538Registry: registryAddress,
      WraithNames: namesAddress,
      WraithSender: senderAddress,
      WraithWithdrawer: withdrawerAddress,
    };

    config.instances.forEach((instance: any) => {
      if (addressMap[instance.abi]) {
        instance.address = addressMap[instance.abi];
        instance.startBlock = deploymentBlock;
      }
    });

    fs.writeFileSync(configPath, JSON.stringify(config, null, 2));
    console.log(`Updated subgraph config: ${configPath}`);
  }

  console.log('\nDeployment summary:');
  console.log('ERC5564Announcer:', announcerAddress);
  console.log('ERC6538Registry:', registryAddress);
  console.log('WraithSender:', senderAddress);
  console.log('WraithNames:', namesAddress);
  console.log('WraithWithdrawer:', withdrawerAddress);
  console.log('Start block:', deploymentBlock);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
