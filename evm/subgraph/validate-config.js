#!/usr/bin/env node

/**
 * Validate subgraph configuration files for placeholder addresses.
 * Fails the build if any contract address is set to 0xdead.
 */

const fs = require('fs');
const path = require('path');

const PLACEHOLDER_ADDRESS = '0x000000000000000000000000000000000000dead';
const PLACEHOLDER_START_BLOCK = 0;

function validateConfig(configPath) {
  const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
  const errors = [];

  config.instances.forEach((instance, index) => {
    if (instance.address.toLowerCase() === PLACEHOLDER_ADDRESS) {
      errors.push(
        `Instance ${index + 1} (${instance.abi}): Address is placeholder ${PLACEHOLDER_ADDRESS}`,
      );
    }
    if (instance.startBlock === PLACEHOLDER_START_BLOCK) {
      errors.push(
        `Instance ${index + 1} (${instance.abi}): Start block is placeholder ${PLACEHOLDER_START_BLOCK}`,
      );
    }
  });

  if (errors.length > 0) {
    console.error(`❌ Validation failed for ${configPath}:`);
    errors.forEach((error) => console.error(`  - ${error}`));
    console.error(
      '\nPlease deploy the contracts and update the configuration with real addresses and start blocks.',
    );
    process.exit(1);
  }

  console.log(`✅ Configuration valid: ${configPath}`);
  return true;
}

// Validate all network configs
const networksDir = path.join(__dirname);
const networkDirs = fs
  .readdirSync(networksDir, { withFileTypes: true })
  .filter((dirent) => dirent.isDirectory())
  .map((dirent) => dirent.name);

let allValid = true;
networkDirs.forEach((network) => {
  const configPath = path.join(networksDir, network, 'instant-config.json');
  if (fs.existsSync(configPath)) {
    try {
      validateConfig(configPath);
    } catch (error) {
      allValid = false;
    }
  }
});

if (!allValid) {
  process.exit(1);
}

console.log('\n✅ All subgraph configurations are valid!');
