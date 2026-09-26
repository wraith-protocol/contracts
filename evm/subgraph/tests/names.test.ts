import { Bytes } from '@graphprotocol/graph-ts';
import { assert, beforeEach, clearStore, describe, test } from 'matchstick-as/assembly/index';

import { Name } from '../generated/schema';
import { handleNameRegistered, handleNameReleased } from '../mappings/names';
import { TX_HASH, createNameRegisteredEvent, createNameReleasedEvent } from './utils';

const NAME_HASH = Bytes.fromHexString(
  '0x1111111111111111111111111111111111111111111111111111111111111111',
);
const NAME = 'alice';
const META_ADDRESS = Bytes.fromHexString(
  '0x020000000000000000000000000000000000000000000000000000000000000001',
);
const UPDATED_META_ADDRESS = Bytes.fromHexString(
  '0x030000000000000000000000000000000000000000000000000000000000000002',
);

describe('handleNameRegistered', () => {
  beforeEach(() => {
    clearStore();
  });

  test('creates an active Name entity with its meta-address and registration timestamp', () => {
    handleNameRegistered(createNameRegisteredEvent(NAME_HASH, NAME, META_ADDRESS, 300, 1700002000));

    let id = NAME_HASH.toHexString();
    assert.fieldEquals('Name', id, 'name', NAME);
    assert.fieldEquals('Name', id, 'nameHash', NAME_HASH.toHexString());
    assert.fieldEquals('Name', id, 'stealthMetaAddress', META_ADDRESS.toHexString());
    assert.fieldEquals('Name', id, 'registeredAt', '1700002000');
    assert.fieldEquals('Name', id, 'active', 'true');
    assert.fieldEquals('Name', id, 'blockNumber', '300');
    assert.fieldEquals('Name', id, 'transactionHash', TX_HASH.toHexString());
  });

  test('a freshly registered name has no updatedAt or releasedAt', () => {
    handleNameRegistered(createNameRegisteredEvent(NAME_HASH, NAME, META_ADDRESS));

    let name = Name.load(NAME_HASH.toHexString())!;
    assert.assertTrue(name.updatedAt === null, 'updatedAt must be unset on registration');
    assert.assertTrue(name.releasedAt === null, 'releasedAt must be unset on registration');
  });

  test('duplicate delivery is idempotent', () => {
    let event = createNameRegisteredEvent(NAME_HASH, NAME, META_ADDRESS);

    handleNameRegistered(event);
    handleNameRegistered(event);

    assert.entityCount('Name', 1);
  });

  test('re-registering the same name hash rewrites the entity rather than duplicating it', () => {
    handleNameRegistered(createNameRegisteredEvent(NAME_HASH, NAME, META_ADDRESS, 300, 1700002000));
    // Same name hash delivered from a later block (e.g. a reorg replay).
    handleNameRegistered(
      createNameRegisteredEvent(NAME_HASH, NAME, UPDATED_META_ADDRESS, 301, 1700002500),
    );

    assert.entityCount('Name', 1);
    let id = NAME_HASH.toHexString();
    assert.fieldEquals('Name', id, 'stealthMetaAddress', UPDATED_META_ADDRESS.toHexString());
    assert.fieldEquals('Name', id, 'registeredAt', '1700002500');
    assert.fieldEquals('Name', id, 'active', 'true');
  });

  test('different names are tracked as separate entities', () => {
    let otherHash = Bytes.fromHexString(
      '0x2222222222222222222222222222222222222222222222222222222222222222',
    );

    handleNameRegistered(createNameRegisteredEvent(NAME_HASH, NAME, META_ADDRESS));
    handleNameRegistered(createNameRegisteredEvent(otherHash, 'bob', UPDATED_META_ADDRESS));

    assert.entityCount('Name', 2);
    assert.fieldEquals('Name', NAME_HASH.toHexString(), 'name', 'alice');
    assert.fieldEquals('Name', otherHash.toHexString(), 'name', 'bob');
  });
});

describe('handleNameReleased', () => {
  beforeEach(() => {
    clearStore();
  });

  test('marks an existing name inactive and stamps the release', () => {
    handleNameRegistered(createNameRegisteredEvent(NAME_HASH, NAME, META_ADDRESS));

    handleNameReleased(createNameReleasedEvent(NAME_HASH, NAME, 400, 1700003000, TX_HASH, 1));

    let id = NAME_HASH.toHexString();
    assert.fieldEquals('Name', id, 'active', 'false');
    assert.fieldEquals('Name', id, 'releasedAt', '1700003000');
    assert.fieldEquals('Name', id, 'blockNumber', '400');
  });

  test('release without a prior registration still creates a tombstone entity', () => {
    handleNameReleased(createNameReleasedEvent(NAME_HASH, NAME));

    assert.entityCount('Name', 1);
    let id = NAME_HASH.toHexString();
    assert.fieldEquals('Name', id, 'name', NAME);
    assert.fieldEquals('Name', id, 'stealthMetaAddress', Bytes.empty().toHexString());
    assert.fieldEquals('Name', id, 'active', 'false');
  });

  test('duplicate release delivery is idempotent', () => {
    handleNameRegistered(createNameRegisteredEvent(NAME_HASH, NAME, META_ADDRESS));
    let event = createNameReleasedEvent(NAME_HASH, NAME);

    handleNameReleased(event);
    handleNameReleased(event);

    assert.entityCount('Name', 1);
    assert.fieldEquals('Name', NAME_HASH.toHexString(), 'active', 'false');
  });

  test('release followed by re-register reactivates the same entity', () => {
    handleNameRegistered(createNameRegisteredEvent(NAME_HASH, NAME, META_ADDRESS, 300, 1700002000));
    handleNameReleased(createNameReleasedEvent(NAME_HASH, NAME, 400, 1700003000, TX_HASH, 1));
    handleNameRegistered(
      createNameRegisteredEvent(NAME_HASH, NAME, UPDATED_META_ADDRESS, 500, 1700004000),
    );

    assert.entityCount('Name', 1);
    let id = NAME_HASH.toHexString();
    assert.fieldEquals('Name', id, 'active', 'true');
    assert.fieldEquals('Name', id, 'stealthMetaAddress', UPDATED_META_ADDRESS.toHexString());
    assert.fieldEquals('Name', id, 'registeredAt', '1700004000');
  });
});
