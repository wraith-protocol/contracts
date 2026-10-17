import { ethers } from 'hardhat';
import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';

declare const hre: any;

interface Deployed {
  name: string;
  address: string;
  txHash: string;
}

async function deploy(name: string, ...args: unknown[]): Promise<Deployed> {
  const factory = await ethers.getContractFactory(name);
  const contract = await factory.deploy(...args);
  await contract.waitForDeployment();
  const address = await contract.getAddress();
  const txHash = contract.deploymentTransaction()!.hash;
  console.log(`${name}:`, address);
  return { name, address, txHash };
}

async function main() {
  const [deployer] = await ethers.getSigners();
  console.log('Deploying contracts with:', deployer.address);

  const announcer = await deploy('ERC5564Announcer');
  const registry = await deploy('ERC6538Registry');
  const sender = await deploy('WraithSender', announcer.address);
  const names = await deploy('WraithNames');
  const withdrawer = await deploy('WraithWithdrawer');
  const deployed = [announcer, registry, sender, names, withdrawer];

  // Record the deployment in deployments/evm/<network>.json. The manifest tool
  // reads each contract's deployment receipt for its exact block, hashes the
  // runtime bytecode, then rebuilds deployments/manifest.json and the subgraph
  // instant config. See deployments/README.md.
  const network = hre.network.name;
  const slug = network.replace(/_/g, '-');
  if (network === 'hardhat' || network === 'localhost') {
    console.log('\nLocal network: not recording to deployments/.');
  } else {
    const { version } = JSON.parse(
      fs.readFileSync(path.join(__dirname, '../package.json'), 'utf8'),
    );
    execFileSync(
      'npx',
      [
        'tsx',
        'src/cli.ts',
        'record',
        'evm',
        '--network',
        slug,
        '--rpc',
        hre.network.config.url,
        '--recorded-by',
        'deploy-script',
        '--source-commit',
        'HEAD',
        '--version',
        version,
        ...deployed.flatMap((d) => ['--contract', `${d.name}=${d.address}@${d.txHash}`]),
      ],
      { cwd: path.join(__dirname, '../../scripts/deployment-manifest'), stdio: 'inherit' },
    );
    console.log(
      `\nGenerate the subgraph manifest from the record (from evm/): cd subgraph && npm run manifest -- --network ${slug}`,
    );
  }

  console.log('\nDeployment summary:');
  for (const d of deployed) console.log(`${d.name}:`, d.address, `(tx ${d.txHash})`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
