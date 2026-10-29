import type { Chain } from './manifest.js';

/**
 * Contracts each chain is expected to deploy. Used to list what is not yet
 * deployed in the README and to keep stellar/contract-ids.json keyed by crate.
 */
export const EXPECTED_CONTRACTS: Record<Chain, string[]> = {
  evm: ['ERC5564Announcer', 'ERC6538Registry', 'WraithSender', 'WraithNames', 'WraithWithdrawer'],
  stellar: ['stealth-announcer', 'stealth-registry', 'stealth-sender', 'wraith-names'],
  solana: ['wraith_announcer', 'wraith_sender', 'wraith_names'],
  ckb: ['wraith-stealth-lock', 'wraith-names-type'],
};

/**
 * Chains whose records must account for every contract in EXPECTED_CONTRACTS:
 * each one is either recorded under `contracts` or listed under `undeployed`.
 * EVM is the first; add a chain here once its records are complete.
 */
export const EXPLICIT_COVERAGE_CHAINS: Chain[] = ['evm'];

export const CHAIN_LABEL: Record<Chain, string> = {
  evm: 'EVM',
  stellar: 'Stellar',
  solana: 'Solana',
  ckb: 'CKB',
};

export const ID_LABEL: Record<Chain, string> = {
  evm: 'Address',
  stellar: 'Contract ID',
  solana: 'Program ID',
  ckb: 'Code hash',
};

export const HEIGHT_LABEL: Record<Chain, string> = {
  evm: 'Block',
  stellar: 'Ledger',
  solana: 'Slot',
  ckb: 'Block',
};

/** The Stellar network whose IDs stellar/contract-ids.json (and the TS bindings) use. */
export const STELLAR_BINDINGS_NETWORK = 'stellar-testnet';

export const EVM_PLACEHOLDER_ADDRESS = '0x000000000000000000000000000000000000dead';
