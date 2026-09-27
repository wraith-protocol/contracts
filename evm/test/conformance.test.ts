/**
 * Conformance fixtures — EVM event pipeline.
 *
 * Each test calls a contract function with fixed fixture constants, obtains the
 * receipt, decodes the logs with `interface.parseLog()`, and asserts every field
 * equals the original fixture value.  The canonical constants are defined once at
 * the top of this file; any contract-side change surfaces as an assertion failure.
 *
 * WraithSender and WraithWithdrawer emit NO events.  The subgraph indexes them via
 * call-level handlers (handleSendETH, handleSendERC20, handleWithdrawETH, etc.).
 * Those sections decode the transaction's call input with `interface.decodeFunctionData`
 * and assert the exact field names the subgraph mapping reads.  This is the
 * call-handler parity check that catches API drift between the contracts and the
 * subgraph mapping.
 */

import { expect } from 'chai';
import { ethers } from 'hardhat';
import {
  ERC5564Announcer,
  ERC6538Registry,
  WraithNames,
  WraithSender,
  WraithWithdrawer,
} from '../typechain-types';

// ── Canonical fixture constants ───────────────────────────────────────────────

/** ERC-5564 scheme ID — secp256k1 with view tags per ERC-5564. */
const FIXTURE_SCHEME_ID = 1n;

/** 33-byte compressed ephemeral public key (all 0xab bytes). */
const FIXTURE_EPK = '0x' + 'ab'.repeat(33);

/**
 * Metadata: first byte is the view tag (0xfe = 254), followed by 10 bytes of
 * zero padding.  The view tag is the most important byte for drift detection.
 */
const FIXTURE_METADATA = '0xfe' + '00'.repeat(10);

/** View tag: first byte of FIXTURE_METADATA as a number. */
const FIXTURE_VIEW_TAG = 0xfe;

/** 66-byte stealth meta-address: 33-byte spending key + 33-byte viewing key. */
const FIXTURE_STEALTH_META_ADDRESS = '0x' + '01'.repeat(33) + '02'.repeat(33);

/** A well-known stealth address (used in Announcer / Sender tests). */
const FIXTURE_STEALTH_ADDRESS = '0x1234567890AbcdEF1234567890aBcdef12345678';

/** Token amount used in ERC-20 sender tests. */
const FIXTURE_TOKEN_AMOUNT = ethers.parseEther('500');

/** Sponsor fee used in Withdrawer tests. */
const FIXTURE_SPONSOR_FEE = ethers.parseEther('0.001');

// ── Helpers ───────────────────────────────────────────────────────────────────

/**
 * Deploy a minimal ERC-20 mock for Sender/Withdrawer tests.
 * Uses the ERC20Mock already in contracts/test/.
 */
async function deployToken() {
  const factory = await ethers.getContractFactory('contracts/test/ERC20Mock.sol:ERC20Mock');
  const token = await factory.deploy();
  await token.waitForDeployment();
  return token;
}

// ── ERC5564Announcer conformance ──────────────────────────────────────────────

describe('Conformance: ERC5564Announcer', function () {
  let announcer: ERC5564Announcer;

  beforeEach(async function () {
    const factory = await ethers.getContractFactory('ERC5564Announcer');
    announcer = await factory.deploy();
    await announcer.waitForDeployment();
  });

  it('Announcement event: emits exactly one log', async function () {
    const [caller] = await ethers.getSigners();
    const tx = await announcer
      .connect(caller)
      .announce(FIXTURE_SCHEME_ID, FIXTURE_STEALTH_ADDRESS, FIXTURE_EPK, FIXTURE_METADATA);
    const receipt = await tx.wait();
    expect(receipt!.logs.length).to.equal(1, 'expected exactly one log');
  });

  it('Announcement event: topic[0] is the event selector', async function () {
    const tx = await announcer.announce(
      FIXTURE_SCHEME_ID,
      FIXTURE_STEALTH_ADDRESS,
      FIXTURE_EPK,
      FIXTURE_METADATA,
    );
    const receipt = await tx.wait();
    const log = receipt!.logs[0];
    const expectedSelector = announcer.interface.getEvent('Announcement').topicHash;
    expect(log.topics[0]).to.equal(
      expectedSelector,
      'topic[0] must be the Announcement event selector',
    );
  });

  it('Announcement event: schemeId is indexed at topic[1]', async function () {
    const tx = await announcer.announce(
      FIXTURE_SCHEME_ID,
      FIXTURE_STEALTH_ADDRESS,
      FIXTURE_EPK,
      FIXTURE_METADATA,
    );
    const receipt = await tx.wait();
    const log = receipt!.logs[0];
    // topic[1] is the padded schemeId
    const decodedSchemeId = BigInt(log.topics[1]);
    expect(decodedSchemeId).to.equal(FIXTURE_SCHEME_ID, 'topic[1] must be schemeId');
  });

  it('Announcement event: stealthAddress is indexed at topic[2]', async function () {
    const tx = await announcer.announce(
      FIXTURE_SCHEME_ID,
      FIXTURE_STEALTH_ADDRESS,
      FIXTURE_EPK,
      FIXTURE_METADATA,
    );
    const receipt = await tx.wait();
    const log = receipt!.logs[0];
    const decoded = announcer.interface.parseLog({
      topics: log.topics as string[],
      data: log.data,
    });
    expect(decoded!.args.stealthAddress.toLowerCase()).to.equal(
      FIXTURE_STEALTH_ADDRESS.toLowerCase(),
      'topic[2] must be stealthAddress',
    );
  });

  it('Announcement event: caller is indexed at topic[3]', async function () {
    const [caller] = await ethers.getSigners();
    const tx = await announcer
      .connect(caller)
      .announce(FIXTURE_SCHEME_ID, FIXTURE_STEALTH_ADDRESS, FIXTURE_EPK, FIXTURE_METADATA);
    const receipt = await tx.wait();
    const log = receipt!.logs[0];
    const decoded = announcer.interface.parseLog({
      topics: log.topics as string[],
      data: log.data,
    });
    expect(decoded!.args.caller.toLowerCase()).to.equal(
      caller.address.toLowerCase(),
      'topic[3] must be msg.sender (caller)',
    );
  });

  it('Announcement event: exactly 4 topics (selector + 3 indexed fields)', async function () {
    const tx = await announcer.announce(
      FIXTURE_SCHEME_ID,
      FIXTURE_STEALTH_ADDRESS,
      FIXTURE_EPK,
      FIXTURE_METADATA,
    );
    const receipt = await tx.wait();
    const log = receipt!.logs[0];
    expect(log.topics.length).to.equal(
      4,
      'Announcement must have 4 topics: selector, schemeId, stealthAddress, caller',
    );
  });

  it('Announcement event: non-indexed ephemeralPubKey round-trips', async function () {
    const tx = await announcer.announce(
      FIXTURE_SCHEME_ID,
      FIXTURE_STEALTH_ADDRESS,
      FIXTURE_EPK,
      FIXTURE_METADATA,
    );
    const receipt = await tx.wait();
    const log = receipt!.logs[0];
    const decoded = announcer.interface.parseLog({
      topics: log.topics as string[],
      data: log.data,
    });
    expect(decoded!.args.ephemeralPubKey).to.equal(
      FIXTURE_EPK,
      'ephemeralPubKey must round-trip through non-indexed log data',
    );
  });

  it('Announcement event: non-indexed metadata round-trips with view tag', async function () {
    const tx = await announcer.announce(
      FIXTURE_SCHEME_ID,
      FIXTURE_STEALTH_ADDRESS,
      FIXTURE_EPK,
      FIXTURE_METADATA,
    );
    const receipt = await tx.wait();
    const log = receipt!.logs[0];
    const decoded = announcer.interface.parseLog({
      topics: log.topics as string[],
      data: log.data,
    });
    expect(decoded!.args.metadata).to.equal(
      FIXTURE_METADATA,
      'metadata must round-trip; first byte is the view tag',
    );
    // First byte of the decoded metadata must equal FIXTURE_VIEW_TAG.
    const firstByte = parseInt(decoded!.args.metadata.slice(2, 4), 16);
    expect(firstByte).to.equal(FIXTURE_VIEW_TAG, 'first byte of metadata must be view tag');
  });

  it('Announcement event: full withArgs round-trip', async function () {
    const [caller] = await ethers.getSigners();
    await expect(
      announcer
        .connect(caller)
        .announce(FIXTURE_SCHEME_ID, FIXTURE_STEALTH_ADDRESS, FIXTURE_EPK, FIXTURE_METADATA),
    )
      .to.emit(announcer, 'Announcement')
      .withArgs(FIXTURE_SCHEME_ID, FIXTURE_STEALTH_ADDRESS, caller.address, FIXTURE_EPK, FIXTURE_METADATA);
  });

  // ── Raw topic-position assertions (Task 3.1) ─────────────────────────────────
  // parseLog() resolves fields by ABI name, not raw slot.  These tests read
  // log.topics[N] directly, so a reorder in the Solidity source (which would
  // also regenerate the ABI) would still break them.  Pattern mirrors the
  // existing BigInt(log.topics[1]) check for schemeId above.

  it('Announcement event: stealthAddress is at raw topic[2] slot', async function () {
    const tx = await announcer.announce(
      FIXTURE_SCHEME_ID,
      FIXTURE_STEALTH_ADDRESS,
      FIXTURE_EPK,
      FIXTURE_METADATA,
    );
    const receipt = await tx.wait();
    const log = receipt!.logs[0];
    // Indexed addresses are left-padded to 32 bytes in the raw topic.
    const expectedTopic2 = ethers.zeroPadValue(FIXTURE_STEALTH_ADDRESS, 32);
    expect(log.topics[2].toLowerCase()).to.equal(
      expectedTopic2.toLowerCase(),
      'raw topic[2] must be the ABI-padded stealthAddress',
    );
  });

  it('Announcement event: caller is at raw topic[3] slot', async function () {
    const [caller] = await ethers.getSigners();
    const tx = await announcer
      .connect(caller)
      .announce(FIXTURE_SCHEME_ID, FIXTURE_STEALTH_ADDRESS, FIXTURE_EPK, FIXTURE_METADATA);
    const receipt = await tx.wait();
    const log = receipt!.logs[0];
    const expectedTopic3 = ethers.zeroPadValue(caller.address, 32);
    expect(log.topics[3].toLowerCase()).to.equal(
      expectedTopic3.toLowerCase(),
      'raw topic[3] must be the ABI-padded caller address',
    );
  });
});


// ── ERC6538Registry conformance ───────────────────────────────────────────────

describe('Conformance: ERC6538Registry', function () {
  let registry: ERC6538Registry;

  beforeEach(async function () {
    const factory = await ethers.getContractFactory('ERC6538Registry');
    registry = await factory.deploy();
    await registry.waitForDeployment();
  });

  it('StealthMetaAddressSet: topic[0] is the event selector', async function () {
    const [registrant] = await ethers.getSigners();
    const tx = await registry
      .connect(registrant)
      .registerKeys(FIXTURE_SCHEME_ID, FIXTURE_STEALTH_META_ADDRESS);
    const receipt = await tx.wait();
    const log = receipt!.logs[0];
    const expectedSelector = registry.interface.getEvent('StealthMetaAddressSet').topicHash;
    expect(log.topics[0]).to.equal(expectedSelector);
  });

  it('StealthMetaAddressSet: registrant is indexed at topic[1]', async function () {
    const [registrant] = await ethers.getSigners();
    const tx = await registry
      .connect(registrant)
      .registerKeys(FIXTURE_SCHEME_ID, FIXTURE_STEALTH_META_ADDRESS);
    const receipt = await tx.wait();
    const decoded = registry.interface.parseLog({
      topics: receipt!.logs[0].topics as string[],
      data: receipt!.logs[0].data,
    });
    expect(decoded!.args.registrant.toLowerCase()).to.equal(
      registrant.address.toLowerCase(),
      'topic[1] must be registrant address',
    );
  });

  it('StealthMetaAddressSet: schemeId is indexed at topic[2]', async function () {
    const [registrant] = await ethers.getSigners();
    const tx = await registry
      .connect(registrant)
      .registerKeys(FIXTURE_SCHEME_ID, FIXTURE_STEALTH_META_ADDRESS);
    const receipt = await tx.wait();
    const decoded = registry.interface.parseLog({
      topics: receipt!.logs[0].topics as string[],
      data: receipt!.logs[0].data,
    });
    expect(decoded!.args.schemeId).to.equal(FIXTURE_SCHEME_ID, 'topic[2] must be schemeId');
  });

  it('StealthMetaAddressSet: stealthMetaAddress is non-indexed data and round-trips', async function () {
    const [registrant] = await ethers.getSigners();
    const tx = await registry
      .connect(registrant)
      .registerKeys(FIXTURE_SCHEME_ID, FIXTURE_STEALTH_META_ADDRESS);
    const receipt = await tx.wait();
    const decoded = registry.interface.parseLog({
      topics: receipt!.logs[0].topics as string[],
      data: receipt!.logs[0].data,
    });
    expect(decoded!.args.stealthMetaAddress.toLowerCase()).to.equal(
      FIXTURE_STEALTH_META_ADDRESS.toLowerCase(),
      'stealthMetaAddress must round-trip as non-indexed data',
    );
  });

  it('StealthMetaAddressSet: exactly 3 topics (selector + 2 indexed)', async function () {
    const [registrant] = await ethers.getSigners();
    const tx = await registry
      .connect(registrant)
      .registerKeys(FIXTURE_SCHEME_ID, FIXTURE_STEALTH_META_ADDRESS);
    const receipt = await tx.wait();
    expect(receipt!.logs[0].topics.length).to.equal(
      3,
      'StealthMetaAddressSet must have 3 topics: selector, registrant, schemeId',
    );
  });

  it('StealthMetaAddressSet: full withArgs round-trip', async function () {
    const [registrant] = await ethers.getSigners();
    await expect(
      registry.connect(registrant).registerKeys(FIXTURE_SCHEME_ID, FIXTURE_STEALTH_META_ADDRESS),
    )
      .to.emit(registry, 'StealthMetaAddressSet')
      .withArgs(registrant.address, FIXTURE_SCHEME_ID, FIXTURE_STEALTH_META_ADDRESS);
  });

  it('NonceIncremented: registrant is indexed at topic[1], newNonce is non-indexed', async function () {
    const [registrant] = await ethers.getSigners();
    const tx = await registry.connect(registrant).incrementNonce();
    const receipt = await tx.wait();
    const decoded = registry.interface.parseLog({
      topics: receipt!.logs[0].topics as string[],
      data: receipt!.logs[0].data,
    });
    expect(decoded!.args.registrant.toLowerCase()).to.equal(
      registrant.address.toLowerCase(),
      'NonceIncremented topic[1] must be registrant',
    );
    expect(decoded!.args.newNonce).to.equal(1n, 'NonceIncremented newNonce must be 1 after first increment');
  });

  it('NonceIncremented: exactly 2 topics (selector + 1 indexed)', async function () {
    const [registrant] = await ethers.getSigners();
    const tx = await registry.connect(registrant).incrementNonce();
    const receipt = await tx.wait();
    expect(receipt!.logs[0].topics.length).to.equal(
      2,
      'NonceIncremented must have 2 topics: selector + registrant',
    );
  });

  it('NonceIncremented: full withArgs round-trip', async function () {
    const [registrant] = await ethers.getSigners();
    await expect(registry.connect(registrant).incrementNonce())
      .to.emit(registry, 'NonceIncremented')
      .withArgs(registrant.address, 1n);
  });

  // ── Raw topic-position assertions ────────────────────────────────────────────
  // parseLog() resolves fields by ABI name, not raw slot.  These tests read
  // log.topics[N] directly, so a reorder in the Solidity source (which would
  // also regenerate the ABI) would still break them.  Pattern mirrors the
  // existing raw-slot checks in the ERC5564Announcer section above.

  it('StealthMetaAddressSet: registrant is at raw topic[1] slot', async function () {
    const [registrant] = await ethers.getSigners();
    const tx = await registry
      .connect(registrant)
      .registerKeys(FIXTURE_SCHEME_ID, FIXTURE_STEALTH_META_ADDRESS);
    const receipt = await tx.wait();
    const log = receipt!.logs[0];
    // Indexed addresses are left-padded to 32 bytes in the raw topic.
    const expectedTopic1 = ethers.zeroPadValue(registrant.address, 32);
    expect(log.topics[1].toLowerCase()).to.equal(
      expectedTopic1.toLowerCase(),
      'raw topic[1] must be the ABI-padded registrant address',
    );
  });

  it('StealthMetaAddressSet: schemeId is at raw topic[2] slot', async function () {
    const [registrant] = await ethers.getSigners();
    const tx = await registry
      .connect(registrant)
      .registerKeys(FIXTURE_SCHEME_ID, FIXTURE_STEALTH_META_ADDRESS);
    const receipt = await tx.wait();
    const log = receipt!.logs[0];
    // Indexed uint256 values are ABI-encoded as 32-byte big-endian in the raw topic.
    const decodedSchemeId = BigInt(log.topics[2]);
    expect(decodedSchemeId).to.equal(
      FIXTURE_SCHEME_ID,
      'raw topic[2] must be the schemeId',
    );
  });
});

// ── WraithNames conformance ───────────────────────────────────────────────────

describe('Conformance: WraithNames', function () {
  let names: WraithNames;

  /** Private spending key for fixture registrations. */
  const SPENDING_KEY =
    '0x1111111111111111111111111111111111111111111111111111111111111111';

  /**
   * Build a 66-byte stealth meta-address (compressed spending key + compressed
   * viewing key) and the signing wallet.
   */
  async function makeMetaAddress(spendingPrivKey: string) {
    const spendingWallet = new ethers.Wallet(spendingPrivKey);
    const viewingPrivKey = ethers.keccak256(ethers.toUtf8Bytes('viewing-' + spendingPrivKey));
    const viewingWallet = new ethers.Wallet(viewingPrivKey);
    const metaAddress =
      spendingWallet.signingKey.compressedPublicKey +
      viewingWallet.signingKey.compressedPublicKey.slice(2);
    return { metaAddress, spendingWallet };
  }

  async function signRegistration(wallet: ethers.Wallet, name: string, metaAddress: string) {
    const digest = ethers.keccak256(
      ethers.solidityPacked(['string', 'bytes'], [name, metaAddress]),
    );
    return wallet.signMessage(ethers.getBytes(digest));
  }

  async function signRelease(wallet: ethers.Wallet, name: string) {
    const digest = ethers.keccak256(ethers.solidityPacked(['string'], [name]));
    return wallet.signMessage(ethers.getBytes(digest));
  }

  beforeEach(async function () {
    const factory = await ethers.getContractFactory('WraithNames');
    names = await factory.deploy();
    await names.waitForDeployment();
  });

  it('NameRegistered: topic[0] is the event selector', async function () {
    const { metaAddress, spendingWallet } = await makeMetaAddress(SPENDING_KEY);
    const sig = await signRegistration(spendingWallet, 'alice', metaAddress);
    const tx = await names.register('alice', metaAddress, sig);
    const receipt = await tx.wait();
    const expectedSelector = names.interface.getEvent('NameRegistered').topicHash;
    expect(receipt!.logs[0].topics[0]).to.equal(expectedSelector);
  });

  it('NameRegistered: nameHash is indexed at topic[1] and equals keccak256(name)', async function () {
    const { metaAddress, spendingWallet } = await makeMetaAddress(SPENDING_KEY);
    const fixtureName = 'alice';
    const sig = await signRegistration(spendingWallet, fixtureName, metaAddress);
    const tx = await names.register(fixtureName, metaAddress, sig);
    const receipt = await tx.wait();

    const decoded = names.interface.parseLog({
      topics: receipt!.logs[0].topics as string[],
      data: receipt!.logs[0].data,
    });
    const expectedHash = ethers.keccak256(ethers.toUtf8Bytes(fixtureName));
    expect(decoded!.args.nameHash).to.equal(expectedHash, 'topic[1] nameHash must equal keccak256(name)');
  });

  it('NameRegistered: name is non-indexed data and round-trips', async function () {
    const { metaAddress, spendingWallet } = await makeMetaAddress(SPENDING_KEY);
    const fixtureName = 'alice';
    const sig = await signRegistration(spendingWallet, fixtureName, metaAddress);
    const tx = await names.register(fixtureName, metaAddress, sig);
    const receipt = await tx.wait();

    const decoded = names.interface.parseLog({
      topics: receipt!.logs[0].topics as string[],
      data: receipt!.logs[0].data,
    });
    expect(decoded!.args.name).to.equal(fixtureName, 'name must round-trip as non-indexed data');
  });

  it('NameRegistered: stealthMetaAddress is non-indexed data and round-trips', async function () {
    const { metaAddress, spendingWallet } = await makeMetaAddress(SPENDING_KEY);
    const sig = await signRegistration(spendingWallet, 'alice', metaAddress);
    const tx = await names.register('alice', metaAddress, sig);
    const receipt = await tx.wait();

    const decoded = names.interface.parseLog({
      topics: receipt!.logs[0].topics as string[],
      data: receipt!.logs[0].data,
    });
    expect(decoded!.args.stealthMetaAddress.toLowerCase()).to.equal(
      metaAddress.toLowerCase(),
      'stealthMetaAddress must round-trip',
    );
  });

  it('NameRegistered: exactly 2 topics (selector + nameHash)', async function () {
    const { metaAddress, spendingWallet } = await makeMetaAddress(SPENDING_KEY);
    const sig = await signRegistration(spendingWallet, 'alice', metaAddress);
    const tx = await names.register('alice', metaAddress, sig);
    const receipt = await tx.wait();
    expect(receipt!.logs[0].topics.length).to.equal(
      2,
      'NameRegistered must have 2 topics: selector, nameHash',
    );
  });

  it('NameRegistered: nameHash is consistent across register/release pair', async function () {
    const { metaAddress, spendingWallet } = await makeMetaAddress(SPENDING_KEY);
    const fixtureName = 'crosscheck';
    const regSig = await signRegistration(spendingWallet, fixtureName, metaAddress);
    const regTx = await names.register(fixtureName, metaAddress, regSig);
    const regReceipt = await regTx.wait();

    const relSig = await signRelease(spendingWallet, fixtureName);
    const relTx = await names.release(fixtureName, relSig);
    const relReceipt = await relTx.wait();

    const regDecoded = names.interface.parseLog({
      topics: regReceipt!.logs[0].topics as string[],
      data: regReceipt!.logs[0].data,
    });
    const relDecoded = names.interface.parseLog({
      topics: relReceipt!.logs[0].topics as string[],
      data: relReceipt!.logs[0].data,
    });

    expect(relDecoded!.args.nameHash).to.equal(
      regDecoded!.args.nameHash,
      'nameHash must be identical in NameRegistered and NameReleased for the same name',
    );
    expect(relDecoded!.args.nameHash).to.equal(
      ethers.keccak256(ethers.toUtf8Bytes(fixtureName)),
      'nameHash must equal keccak256(name)',
    );
  });

  it('NameReleased: topic[0] is the event selector', async function () {
    const { metaAddress, spendingWallet } = await makeMetaAddress(SPENDING_KEY);
    const sig = await signRegistration(spendingWallet, 'temp', metaAddress);
    await names.register('temp', metaAddress, sig);
    const relSig = await signRelease(spendingWallet, 'temp');
    const tx = await names.release('temp', relSig);
    const receipt = await tx.wait();

    const expectedSelector = names.interface.getEvent('NameReleased').topicHash;
    expect(receipt!.logs[0].topics[0]).to.equal(expectedSelector);
  });

  it('NameReleased: nameHash is indexed at topic[1]', async function () {
    const { metaAddress, spendingWallet } = await makeMetaAddress(SPENDING_KEY);
    const fixtureName = 'released';
    const sig = await signRegistration(spendingWallet, fixtureName, metaAddress);
    await names.register(fixtureName, metaAddress, sig);

    const relSig = await signRelease(spendingWallet, fixtureName);
    const tx = await names.release(fixtureName, relSig);
    const receipt = await tx.wait();
    const decoded = names.interface.parseLog({
      topics: receipt!.logs[0].topics as string[],
      data: receipt!.logs[0].data,
    });
    expect(decoded!.args.nameHash).to.equal(
      ethers.keccak256(ethers.toUtf8Bytes(fixtureName)),
      'NameReleased topic[1] must be keccak256(name)',
    );
  });

  it('NameReleased: name is non-indexed data and round-trips', async function () {
    const { metaAddress, spendingWallet } = await makeMetaAddress(SPENDING_KEY);
    const fixtureName = 'byebye';
    const sig = await signRegistration(spendingWallet, fixtureName, metaAddress);
    await names.register(fixtureName, metaAddress, sig);

    const relSig = await signRelease(spendingWallet, fixtureName);
    const tx = await names.release(fixtureName, relSig);
    const receipt = await tx.wait();
    const decoded = names.interface.parseLog({
      topics: receipt!.logs[0].topics as string[],
      data: receipt!.logs[0].data,
    });
    expect(decoded!.args.name).to.equal(fixtureName, 'NameReleased name must round-trip');
  });

  it('NameReleased: exactly 2 topics (selector + nameHash)', async function () {
    const { metaAddress, spendingWallet } = await makeMetaAddress(SPENDING_KEY);
    const sig = await signRegistration(spendingWallet, 'ephemeral', metaAddress);
    await names.register('ephemeral', metaAddress, sig);
    const relSig = await signRelease(spendingWallet, 'ephemeral');
    const tx = await names.release('ephemeral', relSig);
    const receipt = await tx.wait();
    expect(receipt!.logs[0].topics.length).to.equal(
      2,
      'NameReleased must have 2 topics: selector, nameHash',
    );
  });

  // ── Raw topic-position assertions ────────────────────────────────────────────
  // parseLog() resolves fields by ABI name, not raw slot.  These tests read
  // log.topics[N] directly, so a reorder in the Solidity source (which would
  // also regenerate the ABI) would still break them.  Pattern mirrors the
  // existing raw-slot checks in the ERC5564Announcer section above.
  //
  // nameHash derivation confirmed from WraithNames.sol line 72:
  //   bytes32 nameHash = keccak256(bytes(name));
  // which is equivalent to ethers.keccak256(ethers.toUtf8Bytes(name)).

  it('NameRegistered: nameHash is at raw topic[1] slot', async function () {
    const { metaAddress, spendingWallet } = await makeMetaAddress(SPENDING_KEY);
    const fixtureName = 'alice';
    const sig = await signRegistration(spendingWallet, fixtureName, metaAddress);
    const tx = await names.register(fixtureName, metaAddress, sig);
    const receipt = await tx.wait();
    const log = receipt!.logs[0];
    // nameHash = keccak256(bytes(name)) per WraithNames.sol:72 — identical to
    // ethers.keccak256(ethers.toUtf8Bytes(name)).
    const expectedTopic1 = ethers.keccak256(ethers.toUtf8Bytes(fixtureName));
    expect(log.topics[1]).to.equal(
      expectedTopic1,
      'raw topic[1] must be keccak256(bytes(name))',
    );
  });

  it('NameReleased: nameHash is at raw topic[1] slot', async function () {
    const { metaAddress, spendingWallet } = await makeMetaAddress(SPENDING_KEY);
    const fixtureName = 'released';
    const sig = await signRegistration(spendingWallet, fixtureName, metaAddress);
    await names.register(fixtureName, metaAddress, sig);

    const relSig = await signRelease(spendingWallet, fixtureName);
    const tx = await names.release(fixtureName, relSig);
    const receipt = await tx.wait();
    const log = receipt!.logs[0];
    // Same nameHash derivation: keccak256(bytes(name)) per WraithNames.sol:160.
    const expectedTopic1 = ethers.keccak256(ethers.toUtf8Bytes(fixtureName));
    expect(log.topics[1]).to.equal(
      expectedTopic1,
      'raw topic[1] must be keccak256(bytes(name))',
    );
  });
});

// ── WraithSender call-input parity (subgraph call-handler field drift check) ──

describe('Conformance: WraithSender call-input parity (subgraph field drift)', function () {
  let announcer: ERC5564Announcer;
  let sender: WraithSender;

  beforeEach(async function () {
    const AnnouncerFactory = await ethers.getContractFactory('ERC5564Announcer');
    announcer = await AnnouncerFactory.deploy();
    await announcer.waitForDeployment();

    const SenderFactory = await ethers.getContractFactory('WraithSender');
    sender = await SenderFactory.deploy(await announcer.getAddress());
    await sender.waitForDeployment();
  });

  /**
   * Decode a transaction's call input and assert the exact field names that
   * the subgraph mapping reads.  Any rename or reorder breaks this test.
   *
   * Subgraph mapping reads (sender.ts):
   *   sendETH   → call.inputs.schemeId, stealthAddress, metadata (+ call.transaction.value)
   *   sendERC20 → call.inputs.token, amount, schemeId, stealthAddress, metadata
   *   batchSendETH → call.inputs.schemeId, stealthAddresses[], amounts[], metadatas[]
   *   batchSendERC20 → call.inputs.token, schemeId, stealthAddresses[], amounts[], metadatas[]
   */

  it('sendETH: call input fields match subgraph mapping (schemeId, stealthAddress, metadata)', async function () {
    const [, recipient] = await ethers.getSigners();
    const amount = ethers.parseEther('0.1');
    const tx = await sender.sendETH(
      FIXTURE_SCHEME_ID,
      recipient.address,
      FIXTURE_EPK,
      FIXTURE_METADATA,
      { value: amount },
    );
    const txData = (await ethers.provider.getTransaction(tx.hash))!;
    const decoded = sender.interface.decodeFunctionData('sendETH', txData.data);

    expect(decoded.schemeId).to.equal(FIXTURE_SCHEME_ID, 'sendETH: schemeId field must exist and match');
    expect(decoded.stealthAddress.toLowerCase()).to.equal(
      recipient.address.toLowerCase(),
      'sendETH: stealthAddress field must exist and match',
    );
    expect(decoded.ephemeralPubKey).to.equal(
      FIXTURE_EPK,
      'sendETH: ephemeralPubKey field must exist and match',
    );
    expect(decoded.metadata).to.equal(FIXTURE_METADATA, 'sendETH: metadata field must exist and match');
  });

  it('sendERC20: call input fields match subgraph mapping (token, amount, schemeId, stealthAddress, metadata)', async function () {
    const [deployer, recipient] = await ethers.getSigners();
    const token = await deployToken();
    const tokenAddress = await token.getAddress();
    await token.mint(deployer.address, FIXTURE_TOKEN_AMOUNT);
    await token.approve(await sender.getAddress(), FIXTURE_TOKEN_AMOUNT);

    const tx = await sender.sendERC20(
      tokenAddress,
      FIXTURE_TOKEN_AMOUNT,
      FIXTURE_SCHEME_ID,
      recipient.address,
      FIXTURE_EPK,
      FIXTURE_METADATA,
    );
    const txData = (await ethers.provider.getTransaction(tx.hash))!;
    const decoded = sender.interface.decodeFunctionData('sendERC20', txData.data);

    expect(decoded.token.toLowerCase()).to.equal(
      tokenAddress.toLowerCase(),
      'sendERC20: token field must exist and match',
    );
    expect(decoded.amount).to.equal(FIXTURE_TOKEN_AMOUNT, 'sendERC20: amount field must exist and match');
    expect(decoded.schemeId).to.equal(FIXTURE_SCHEME_ID, 'sendERC20: schemeId field must exist and match');
    expect(decoded.stealthAddress.toLowerCase()).to.equal(
      recipient.address.toLowerCase(),
      'sendERC20: stealthAddress field must exist and match',
    );
    expect(decoded.metadata).to.equal(
      FIXTURE_METADATA,
      'sendERC20: metadata field must exist and match',
    );
  });

  it('batchSendETH: call input fields match subgraph mapping (schemeId, stealthAddresses[], amounts[], metadatas[])', async function () {
    const signers = await ethers.getSigners();
    const recipients = [signers[1].address, signers[2].address];
    const amounts = [ethers.parseEther('0.1'), ethers.parseEther('0.2')];
    const totalValue = amounts.reduce((a, b) => a + b, 0n);

    const tx = await sender.batchSendETH(
      FIXTURE_SCHEME_ID,
      recipients,
      [FIXTURE_EPK, FIXTURE_EPK],
      [FIXTURE_METADATA, FIXTURE_METADATA],
      amounts,
      { value: totalValue },
    );
    const txData = (await ethers.provider.getTransaction(tx.hash))!;
    const decoded = sender.interface.decodeFunctionData('batchSendETH', txData.data);

    expect(decoded.schemeId).to.equal(
      FIXTURE_SCHEME_ID,
      'batchSendETH: schemeId field must exist and match',
    );
    expect(decoded.stealthAddresses.length).to.equal(
      2,
      'batchSendETH: stealthAddresses must be an array',
    );
    expect(decoded.amounts.length).to.equal(2, 'batchSendETH: amounts must be an array');
    expect(decoded.metadatas.length).to.equal(2, 'batchSendETH: metadatas must be an array');
    // Spot-check first element
    expect(decoded.stealthAddresses[0].toLowerCase()).to.equal(
      recipients[0].toLowerCase(),
      'batchSendETH: stealthAddresses[0] must match',
    );
    expect(decoded.amounts[0]).to.equal(amounts[0], 'batchSendETH: amounts[0] must match');
    expect(decoded.metadatas[0]).to.equal(
      FIXTURE_METADATA,
      'batchSendETH: metadatas[0] must match',
    );
  });

  it('batchSendERC20: call input fields match subgraph mapping (token, schemeId, stealthAddresses[], amounts[], metadatas[])', async function () {
    const [deployer, r1, r2] = await ethers.getSigners();
    const token = await deployToken();
    const tokenAddress = await token.getAddress();
    const amounts = [ethers.parseEther('50'), ethers.parseEther('100')];
    const totalAmount = amounts.reduce((a, b) => a + b, 0n);
    await token.mint(deployer.address, totalAmount);
    await token.approve(await sender.getAddress(), totalAmount);

    const tx = await sender.batchSendERC20(
      tokenAddress,
      FIXTURE_SCHEME_ID,
      [r1.address, r2.address],
      [FIXTURE_EPK, FIXTURE_EPK],
      [FIXTURE_METADATA, FIXTURE_METADATA],
      amounts,
    );
    const txData = (await ethers.provider.getTransaction(tx.hash))!;
    const decoded = sender.interface.decodeFunctionData('batchSendERC20', txData.data);

    expect(decoded.token.toLowerCase()).to.equal(
      tokenAddress.toLowerCase(),
      'batchSendERC20: token field must exist and match',
    );
    expect(decoded.schemeId).to.equal(
      FIXTURE_SCHEME_ID,
      'batchSendERC20: schemeId field must exist and match',
    );
    expect(decoded.stealthAddresses[0].toLowerCase()).to.equal(
      r1.address.toLowerCase(),
    );
    expect(decoded.amounts[1]).to.equal(amounts[1]);
    expect(decoded.metadatas[1]).to.equal(FIXTURE_METADATA);
  });

  it('WraithSender emits NO events itself — all sends route through the announcer', async function () {
    const [, recipient] = await ethers.getSigners();
    const tx = await sender.sendETH(
      FIXTURE_SCHEME_ID,
      recipient.address,
      FIXTURE_EPK,
      FIXTURE_METADATA,
      { value: ethers.parseEther('0.01') },
    );
    const receipt = await tx.wait();
    // Every log must come from the announcer address, not from sender.
    const senderAddress = (await sender.getAddress()).toLowerCase();
    for (const log of receipt!.logs) {
      expect(log.address.toLowerCase()).not.to.equal(
        senderAddress,
        'WraithSender must not emit any events; all logs must be from the announcer',
      );
    }
  });
});

// ── WraithWithdrawer call-input parity (subgraph field drift check) ────────────

describe('Conformance: WraithWithdrawer call-input parity (subgraph field drift)', function () {
  let withdrawer: WraithWithdrawer;

  /**
   * Subgraph mapping reads (withdrawer.ts):
   *   withdrawETH      → call.inputs.destination, sponsorFee
   *   withdrawERC20    → call.inputs.token, destination, sponsorFee
   *   withdrawETHDirect  → call.inputs.destination
   *   withdrawERC20Direct → call.inputs.token, destination
   */

  beforeEach(async function () {
    const factory = await ethers.getContractFactory('WraithWithdrawer');
    withdrawer = await factory.deploy();
    await withdrawer.waitForDeployment();
  });

  it('withdrawETH: call input has destination and sponsorFee fields', async function () {
    const [sponsor, destination] = await ethers.getSigners();
    const withdrawerAddr = await withdrawer.getAddress();

    // WraithWithdrawer is an EIP-7702 delegation target with no receive() function.
    // Fund it at the state level using hardhat_setBalance so we can exercise the
    // call without a live EIP-7702 delegation setup.
    await ethers.provider.send('hardhat_setBalance', [
      withdrawerAddr,
      '0x' + ethers.parseEther('0.1').toString(16),
    ]);

    const tx = await withdrawer.connect(sponsor).withdrawETH(destination.address, FIXTURE_SPONSOR_FEE);
    const txData = (await ethers.provider.getTransaction(tx.hash))!;
    const decoded = withdrawer.interface.decodeFunctionData('withdrawETH', txData.data);

    expect(decoded.destination.toLowerCase()).to.equal(
      destination.address.toLowerCase(),
      'withdrawETH: destination field must exist and match',
    );
    expect(decoded.sponsorFee).to.equal(
      FIXTURE_SPONSOR_FEE,
      'withdrawETH: sponsorFee field must exist and match',
    );
  });

  it('withdrawERC20: call input has token, destination, and sponsorFee fields', async function () {
    const [deployer, destination] = await ethers.getSigners();
    const token = await deployToken();
    const tokenAddress = await token.getAddress();
    const withdrawerAddr = await withdrawer.getAddress();

    // Mint tokens directly to the withdrawer (simulating a funded stealth address).
    await token.mint(withdrawerAddr, FIXTURE_TOKEN_AMOUNT);

    const tx = await withdrawer
      .connect(deployer)
      .withdrawERC20(tokenAddress, destination.address, FIXTURE_SPONSOR_FEE);
    const txData = (await ethers.provider.getTransaction(tx.hash))!;
    const decoded = withdrawer.interface.decodeFunctionData('withdrawERC20', txData.data);

    expect(decoded.token.toLowerCase()).to.equal(
      tokenAddress.toLowerCase(),
      'withdrawERC20: token field must exist and match',
    );
    expect(decoded.destination.toLowerCase()).to.equal(
      destination.address.toLowerCase(),
      'withdrawERC20: destination field must exist and match',
    );
    expect(decoded.sponsorFee).to.equal(
      FIXTURE_SPONSOR_FEE,
      'withdrawERC20: sponsorFee field must exist and match',
    );
  });

  it('withdrawETHDirect: call input has destination field', async function () {
    const [sponsor, destination] = await ethers.getSigners();
    const withdrawerAddr = await withdrawer.getAddress();
    await ethers.provider.send('hardhat_setBalance', [
      withdrawerAddr,
      '0x' + ethers.parseEther('0.05').toString(16),
    ]);

    const tx = await withdrawer.connect(sponsor).withdrawETHDirect(destination.address);
    const txData = (await ethers.provider.getTransaction(tx.hash))!;
    const decoded = withdrawer.interface.decodeFunctionData('withdrawETHDirect', txData.data);

    expect(decoded.destination.toLowerCase()).to.equal(
      destination.address.toLowerCase(),
      'withdrawETHDirect: destination field must exist and match',
    );
  });

  it('withdrawERC20Direct: call input has token and destination fields', async function () {
    const [deployer, destination] = await ethers.getSigners();
    const token = await deployToken();
    const tokenAddress = await token.getAddress();
    const withdrawerAddr = await withdrawer.getAddress();
    await token.mint(withdrawerAddr, FIXTURE_TOKEN_AMOUNT);

    const tx = await withdrawer.connect(deployer).withdrawERC20Direct(tokenAddress, destination.address);
    const txData = (await ethers.provider.getTransaction(tx.hash))!;
    const decoded = withdrawer.interface.decodeFunctionData('withdrawERC20Direct', txData.data);

    expect(decoded.token.toLowerCase()).to.equal(
      tokenAddress.toLowerCase(),
      'withdrawERC20Direct: token field must exist and match',
    );
    expect(decoded.destination.toLowerCase()).to.equal(
      destination.address.toLowerCase(),
      'withdrawERC20Direct: destination field must exist and match',
    );
  });

  it('WraithWithdrawer emits NO events', async function () {
    const [sponsor, destination] = await ethers.getSigners();
    const withdrawerAddr = await withdrawer.getAddress();
    await ethers.provider.send('hardhat_setBalance', [
      withdrawerAddr,
      '0x' + ethers.parseEther('0.1').toString(16),
    ]);

    const tx = await withdrawer.connect(sponsor).withdrawETH(destination.address, FIXTURE_SPONSOR_FEE);
    const receipt = await tx.wait();
    expect(receipt!.logs.length).to.equal(0, 'WraithWithdrawer must emit no events');
  });
});

// ── WraithSender → ERC5564Announcer end-to-end integration (Task 2.1) ─────────
//
// WraithSender.sendETH() and sendERC20() forward all five announcement fields
// to ERC5564Announcer.announce() unchanged.  These tests wire the real contracts
// together and decode the resulting Announcement log — proving the sender
// does not mutate schemeId, stealthAddress, caller, ephemeralPubKey, or metadata
// in transit.  A bug in the forwarding path (wrong argument order, truncated
// bytes, etc.) would produce a mismatched assertion here even though the
// standalone announcer tests pass.

describe('Conformance: WraithSender → ERC5564Announcer end-to-end', function () {
  let announcer: ERC5564Announcer;
  let sender: WraithSender;

  beforeEach(async function () {
    const AnnouncerFactory = await ethers.getContractFactory('ERC5564Announcer');
    announcer = await AnnouncerFactory.deploy();
    await announcer.waitForDeployment();

    const SenderFactory = await ethers.getContractFactory('WraithSender');
    sender = await SenderFactory.deploy(await announcer.getAddress());
    await sender.waitForDeployment();
  });

  it('sendETH: Announcement fields match exactly what was passed to sendETH', async function () {
    const [caller, recipient] = await ethers.getSigners();
    const FIXTURE_ETH_AMOUNT = ethers.parseEther('0.05');

    const tx = await sender
      .connect(caller)
      .sendETH(FIXTURE_SCHEME_ID, recipient.address, FIXTURE_EPK, FIXTURE_METADATA, {
        value: FIXTURE_ETH_AMOUNT,
      });
    const receipt = await tx.wait();

    // The Announcement is emitted by the announcer, not the sender.
    // There must be exactly one log (from the announcer).
    expect(receipt!.logs.length).to.equal(1, 'sendETH must produce exactly one log (from announcer)');
    const log = receipt!.logs[0];

    // Confirm it came from the announcer, not the sender.
    expect(log.address.toLowerCase()).to.equal(
      (await announcer.getAddress()).toLowerCase(),
      'Announcement must be emitted by the announcer contract',
    );

    // Decode via the announcer's interface and assert every field.
    const decoded = announcer.interface.parseLog({
      topics: log.topics as string[],
      data: log.data,
    });
    expect(decoded!.args.schemeId).to.equal(
      FIXTURE_SCHEME_ID,
      'sendETH: schemeId must pass through to Announcement unchanged',
    );
    expect(decoded!.args.stealthAddress.toLowerCase()).to.equal(
      recipient.address.toLowerCase(),
      'sendETH: stealthAddress (recipient) must pass through unchanged',
    );
    expect(decoded!.args.caller.toLowerCase()).to.equal(
      (await sender.getAddress()).toLowerCase(),
      'sendETH: caller in Announcement must be the sender contract (it calls announce())',
    );
    expect(decoded!.args.ephemeralPubKey).to.equal(
      FIXTURE_EPK,
      'sendETH: ephemeralPubKey must pass through unchanged',
    );
    expect(decoded!.args.metadata).to.equal(
      FIXTURE_METADATA,
      'sendETH: metadata must pass through unchanged',
    );
  });

  it('sendETH: ETH amount transferred to stealthAddress matches msg.value (Task 4.2)', async function () {
    // The subgraph reads call.transaction.value for the ETH amount.
    // Verify the amount actually landed at the stealth address.
    const [caller] = await ethers.getSigners();
    const FIXTURE_ETH_AMOUNT = ethers.parseEther('0.05');

    // Use a known EOA as the stealth address so we can check its balance.
    const [, , stealthSigner] = await ethers.getSigners();
    const stealthAddr = stealthSigner.address;
    const balanceBefore = await ethers.provider.getBalance(stealthAddr);

    const tx = await sender
      .connect(caller)
      .sendETH(FIXTURE_SCHEME_ID, stealthAddr, FIXTURE_EPK, FIXTURE_METADATA, {
        value: FIXTURE_ETH_AMOUNT,
      });
    await tx.wait();

    const balanceAfter = await ethers.provider.getBalance(stealthAddr);
    expect(balanceAfter - balanceBefore).to.equal(
      FIXTURE_ETH_AMOUNT,
      'sendETH: full msg.value must land at stealthAddress (no fees, no deduction)',
    );
  });

  it('sendERC20: Announcement fields match exactly what was passed to sendERC20', async function () {
    // sendERC20 follows the same forwarding path as sendETH — verify it too,
    // since different parameters could mask an argument-order bug at the call site.
    const [deployer, recipient] = await ethers.getSigners();
    const token = await deployToken();
    const tokenAddress = await token.getAddress();
    await token.mint(deployer.address, FIXTURE_TOKEN_AMOUNT);
    await token.approve(await sender.getAddress(), FIXTURE_TOKEN_AMOUNT);

    const tx = await sender
      .connect(deployer)
      .sendERC20(
        tokenAddress,
        FIXTURE_TOKEN_AMOUNT,
        FIXTURE_SCHEME_ID,
        recipient.address,
        FIXTURE_EPK,
        FIXTURE_METADATA,
      );
    const receipt = await tx.wait();

    // sendERC20 emits one ERC-20 Transfer log + one Announcement log.
    // Find the Announcement log by matching the announcer address.
    const announcerAddr = (await announcer.getAddress()).toLowerCase();
    const announcementLog = receipt!.logs.find(
      (l) => l.address.toLowerCase() === announcerAddr,
    );
    expect(announcementLog).to.not.be.undefined;

    const decoded = announcer.interface.parseLog({
      topics: announcementLog!.topics as string[],
      data: announcementLog!.data,
    });
    expect(decoded!.args.schemeId).to.equal(
      FIXTURE_SCHEME_ID,
      'sendERC20: schemeId must pass through unchanged',
    );
    expect(decoded!.args.stealthAddress.toLowerCase()).to.equal(
      recipient.address.toLowerCase(),
      'sendERC20: stealthAddress must pass through unchanged',
    );
    expect(decoded!.args.caller.toLowerCase()).to.equal(
      (await sender.getAddress()).toLowerCase(),
      'sendERC20: caller in Announcement must be the sender contract',
    );
    expect(decoded!.args.ephemeralPubKey).to.equal(
      FIXTURE_EPK,
      'sendERC20: ephemeralPubKey must pass through unchanged',
    );
    expect(decoded!.args.metadata).to.equal(
      FIXTURE_METADATA,
      'sendERC20: metadata must pass through unchanged',
    );
  });
});

// ── Decode + re-encode round-trips (Task 4.1) ─────────────────────────────────
//
// The issue requires "decode AND re-encode through bindings."  These tests take
// the raw on-chain log, decode it with interface.parseLog(), then re-encode the
// decoded values back to topics+data with interface.encodeEventLog(), and assert
// the re-encoded bytes match the original log exactly.  This proves the ABI
// encoder and decoder are inverses for our fixture values and would catch any
// encoding-width or type-coercion bug introduced by a binding regeneration.

describe('Conformance: decode + re-encode round-trips', function () {
  it('Announcement: re-encoded topics and data match original log bytes', async function () {
    const AnnouncerFactory = await ethers.getContractFactory('ERC5564Announcer');
    const ann = await AnnouncerFactory.deploy();
    await ann.waitForDeployment();

    const [caller] = await ethers.getSigners();
    const tx = await ann
      .connect(caller)
      .announce(FIXTURE_SCHEME_ID, FIXTURE_STEALTH_ADDRESS, FIXTURE_EPK, FIXTURE_METADATA);
    const receipt = await tx.wait();
    const originalLog = receipt!.logs[0];

    // Decode from on-chain log.
    const decoded = ann.interface.parseLog({
      topics: originalLog.topics as string[],
      data: originalLog.data,
    })!;

    // Re-encode the decoded values back to topics + data.
    const reEncoded = ann.interface.encodeEventLog('Announcement', [
      decoded.args.schemeId,
      decoded.args.stealthAddress,
      decoded.args.caller,
      decoded.args.ephemeralPubKey,
      decoded.args.metadata,
    ]);

    // Topics must be byte-for-byte identical.
    expect(reEncoded.topics.length).to.equal(originalLog.topics.length);
    for (let i = 0; i < reEncoded.topics.length; i++) {
      expect(reEncoded.topics[i].toLowerCase()).to.equal(
        originalLog.topics[i].toLowerCase(),
        `re-encoded topic[${i}] must match original`,
      );
    }
    // Non-indexed data must be byte-for-byte identical.
    expect(reEncoded.data.toLowerCase()).to.equal(
      originalLog.data.toLowerCase(),
      're-encoded data must match original log data',
    );
  });

  it('StealthMetaAddressSet: re-encoded topics and data match original log bytes', async function () {
    const RegistryFactory = await ethers.getContractFactory('ERC6538Registry');
    const registry = await RegistryFactory.deploy();
    await registry.waitForDeployment();

    const [registrant] = await ethers.getSigners();
    const tx = await registry
      .connect(registrant)
      .registerKeys(FIXTURE_SCHEME_ID, FIXTURE_STEALTH_META_ADDRESS);
    const receipt = await tx.wait();
    const originalLog = receipt!.logs[0];

    const decoded = registry.interface.parseLog({
      topics: originalLog.topics as string[],
      data: originalLog.data,
    })!;

    const reEncoded = registry.interface.encodeEventLog('StealthMetaAddressSet', [
      decoded.args.registrant,
      decoded.args.schemeId,
      decoded.args.stealthMetaAddress,
    ]);

    expect(reEncoded.topics.length).to.equal(originalLog.topics.length);
    for (let i = 0; i < reEncoded.topics.length; i++) {
      expect(reEncoded.topics[i].toLowerCase()).to.equal(
        originalLog.topics[i].toLowerCase(),
        `re-encoded topic[${i}] must match original`,
      );
    }
    expect(reEncoded.data.toLowerCase()).to.equal(
      originalLog.data.toLowerCase(),
      're-encoded StealthMetaAddressSet data must match original',
    );
  });

  it('NameRegistered: re-encoded topics and data match original log bytes', async function () {
    const NamesFactory = await ethers.getContractFactory('WraithNames');
    const names = await NamesFactory.deploy();
    await names.waitForDeployment();

    // Sign and register a name using the same helper logic as the existing suite.
    const SPENDING_KEY = '0x1111111111111111111111111111111111111111111111111111111111111111';
    const spendingWallet = new ethers.Wallet(SPENDING_KEY);
    const viewingPrivKey = ethers.keccak256(ethers.toUtf8Bytes('viewing-' + SPENDING_KEY));
    const viewingWallet = new ethers.Wallet(viewingPrivKey);
    const metaAddress =
      spendingWallet.signingKey.compressedPublicKey +
      viewingWallet.signingKey.compressedPublicKey.slice(2);
    const fixtureName = 'reencodetest';
    const digest = ethers.keccak256(
      ethers.solidityPacked(['string', 'bytes'], [fixtureName, metaAddress]),
    );
    const sig = await spendingWallet.signMessage(ethers.getBytes(digest));

    const tx = await names.register(fixtureName, metaAddress, sig);
    const receipt = await tx.wait();
    const originalLog = receipt!.logs[0];

    const decoded = names.interface.parseLog({
      topics: originalLog.topics as string[],
      data: originalLog.data,
    })!;

    const reEncoded = names.interface.encodeEventLog('NameRegistered', [
      decoded.args.nameHash,
      decoded.args.name,
      decoded.args.stealthMetaAddress,
    ]);

    expect(reEncoded.topics.length).to.equal(originalLog.topics.length);
    for (let i = 0; i < reEncoded.topics.length; i++) {
      expect(reEncoded.topics[i].toLowerCase()).to.equal(
        originalLog.topics[i].toLowerCase(),
        `re-encoded topic[${i}] must match original`,
      );
    }
    expect(reEncoded.data.toLowerCase()).to.equal(
      originalLog.data.toLowerCase(),
      're-encoded NameRegistered data must match original',
    );
  });
});
