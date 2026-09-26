import { Address, BigInt, Bytes } from '@graphprotocol/graph-ts';
import { assert, beforeEach, clearStore, describe, test } from 'matchstick-as/assembly/index';

import { handleAnnouncement } from '../mappings/announcer';
import { CALLER, STEALTH_ADDRESS, TX_HASH, createAnnouncementEvent } from './utils';

const SCHEME_ID = BigInt.fromI32(1);
const EPHEMERAL_PUB_KEY = Bytes.fromHexString('0x02b1c2d3');
const METADATA = Bytes.fromHexString('0xdeadbeef');

function announcementId(txHash: Bytes, logIndex: i32): string {
  return txHash.toHexString() + '-' + BigInt.fromI32(logIndex).toString();
}

describe('handleAnnouncement', () => {
  beforeEach(() => {
    clearStore();
  });

  test('creates an Announcement entity with every indexed field', () => {
    let event = createAnnouncementEvent(
      SCHEME_ID,
      STEALTH_ADDRESS,
      CALLER,
      EPHEMERAL_PUB_KEY,
      METADATA,
    );

    handleAnnouncement(event);

    let id = announcementId(TX_HASH, 0);
    assert.fieldEquals('Announcement', id, 'schemeId', SCHEME_ID.toString());
    assert.fieldEquals('Announcement', id, 'stealthAddress', STEALTH_ADDRESS.toHexString());
    assert.fieldEquals('Announcement', id, 'caller', CALLER.toHexString());
    assert.fieldEquals('Announcement', id, 'ephemeralPubKey', EPHEMERAL_PUB_KEY.toHexString());
    assert.fieldEquals('Announcement', id, 'metadata', METADATA.toHexString());
  });

  test('records block, transaction and log provenance', () => {
    let event = createAnnouncementEvent(
      SCHEME_ID,
      STEALTH_ADDRESS,
      CALLER,
      EPHEMERAL_PUB_KEY,
      METADATA,
      1234,
      1700009999,
      TX_HASH,
      7,
    );

    handleAnnouncement(event);

    let id = announcementId(TX_HASH, 7);
    assert.fieldEquals('Announcement', id, 'blockNumber', '1234');
    assert.fieldEquals('Announcement', id, 'blockTimestamp', '1700009999');
    assert.fieldEquals('Announcement', id, 'transactionHash', TX_HASH.toHexString());
    assert.fieldEquals('Announcement', id, 'logIndex', '7');
  });

  test('preserves metadata bytes verbatim, including empty payloads', () => {
    let event = createAnnouncementEvent(
      BigInt.fromI32(0),
      STEALTH_ADDRESS,
      CALLER,
      Bytes.empty(),
      Bytes.empty(),
    );

    handleAnnouncement(event);

    let id = announcementId(TX_HASH, 0);
    assert.fieldEquals('Announcement', id, 'metadata', Bytes.empty().toHexString());
    assert.fieldEquals('Announcement', id, 'ephemeralPubKey', Bytes.empty().toHexString());
  });

  test('different callers produce distinct entities keyed by transaction and log index', () => {
    let first = createAnnouncementEvent(
      SCHEME_ID,
      STEALTH_ADDRESS,
      CALLER,
      EPHEMERAL_PUB_KEY,
      METADATA,
      100,
      1700000000,
      TX_HASH,
      0,
    );
    let second = createAnnouncementEvent(
      SCHEME_ID,
      STEALTH_ADDRESS,
      Address.fromString('0xaaaa000000000000000000000000000000000099'),
      EPHEMERAL_PUB_KEY,
      METADATA,
      100,
      1700000000,
      TX_HASH,
      1,
    );

    handleAnnouncement(first);
    handleAnnouncement(second);

    assert.entityCount('Announcement', 2);
    assert.fieldEquals('Announcement', announcementId(TX_HASH, 0), 'caller', CALLER.toHexString());
    assert.fieldEquals(
      'Announcement',
      announcementId(TX_HASH, 1),
      'caller',
      '0xaaaa000000000000000000000000000000000099',
    );
  });

  test('tracks different scheme ids independently', () => {
    let mph = createAnnouncementEvent(
      BigInt.fromI32(1),
      STEALTH_ADDRESS,
      CALLER,
      EPHEMERAL_PUB_KEY,
      METADATA,
      100,
      1700000000,
      TX_HASH,
      0,
    );
    let dks = createAnnouncementEvent(
      BigInt.fromI32(2),
      STEALTH_ADDRESS,
      CALLER,
      EPHEMERAL_PUB_KEY,
      METADATA,
      100,
      1700000000,
      TX_HASH,
      1,
    );

    handleAnnouncement(mph);
    handleAnnouncement(dks);

    assert.fieldEquals('Announcement', announcementId(TX_HASH, 0), 'schemeId', '1');
    assert.fieldEquals('Announcement', announcementId(TX_HASH, 1), 'schemeId', '2');
  });

  test('duplicate delivery is idempotent and does not double count', () => {
    let event = createAnnouncementEvent(
      SCHEME_ID,
      STEALTH_ADDRESS,
      CALLER,
      EPHEMERAL_PUB_KEY,
      METADATA,
    );

    handleAnnouncement(event);
    handleAnnouncement(event);

    assert.entityCount('Announcement', 1);
    assert.fieldEquals(
      'Announcement',
      announcementId(TX_HASH, 0),
      'metadata',
      METADATA.toHexString(),
    );
  });

  test('replay across a reorg rewrites the same entity id without creating a parallel one', () => {
    let original = createAnnouncementEvent(
      SCHEME_ID,
      STEALTH_ADDRESS,
      CALLER,
      EPHEMERAL_PUB_KEY,
      METADATA,
      100,
      1700000000,
      TX_HASH,
      0,
    );
    let replayed = createAnnouncementEvent(
      SCHEME_ID,
      STEALTH_ADDRESS,
      CALLER,
      Bytes.fromHexString('0x03aabbcc'),
      Bytes.fromHexString('0xc0ffee'),
      101,
      1700000001,
      TX_HASH,
      0,
    );

    handleAnnouncement(original);
    handleAnnouncement(replayed);

    assert.entityCount('Announcement', 1);
    assert.fieldEquals(
      'Announcement',
      announcementId(TX_HASH, 0),
      'metadata',
      Bytes.fromHexString('0xc0ffee').toHexString(),
    );
    assert.fieldEquals('Announcement', announcementId(TX_HASH, 0), 'blockNumber', '101');
  });
});
