import { BigInt, Bytes } from '@graphprotocol/graph-ts';
import { assert, beforeEach, clearStore, describe, test } from 'matchstick-as/assembly/index';

import { handleNonceIncremented, handleStealthMetaAddressSet } from '../mappings/registry';
import {
  REGISTRANT,
  TX_HASH,
  createNonceIncrementedEvent,
  createStealthMetaAddressSetEvent,
} from './utils';

const SCHEME_ID = BigInt.fromI32(1);
const META_ADDRESS = Bytes.fromHexString(
  '0x020000000000000000000000000000000000000000000000000000000000000001',
);
const UPDATED_META_ADDRESS = Bytes.fromHexString(
  '0x030000000000000000000000000000000000000000000000000000000000000002',
);

function registryId(registrant: string, schemeId: BigInt): string {
  return registrant.toLowerCase() + '-' + schemeId.toString();
}

describe('handleStealthMetaAddressSet', () => {
  beforeEach(() => {
    clearStore();
  });

  test('creates a StealthMetaAddress keyed by registrant and scheme id', () => {
    let event = createStealthMetaAddressSetEvent(REGISTRANT, SCHEME_ID, META_ADDRESS);

    handleStealthMetaAddressSet(event);

    let id = registryId(REGISTRANT.toHexString(), SCHEME_ID);
    assert.fieldEquals('StealthMetaAddress', id, 'registrant', REGISTRANT.toHexString());
    assert.fieldEquals('StealthMetaAddress', id, 'schemeId', SCHEME_ID.toString());
    assert.fieldEquals('StealthMetaAddress', id, 'stealthMetaAddress', META_ADDRESS.toHexString());
  });

  test('records block and transaction provenance', () => {
    let event = createStealthMetaAddressSetEvent(
      REGISTRANT,
      SCHEME_ID,
      META_ADDRESS,
      777,
      1700012345,
      TX_HASH,
      3,
    );

    handleStealthMetaAddressSet(event);

    let id = registryId(REGISTRANT.toHexString(), SCHEME_ID);
    assert.fieldEquals('StealthMetaAddress', id, 'blockNumber', '777');
    assert.fieldEquals('StealthMetaAddress', id, 'blockTimestamp', '1700012345');
    assert.fieldEquals('StealthMetaAddress', id, 'transactionHash', TX_HASH.toHexString());
  });

  test('keeps separate entities per scheme id for the same registrant', () => {
    let mph = createStealthMetaAddressSetEvent(REGISTRANT, BigInt.fromI32(1), META_ADDRESS);
    let dks = createStealthMetaAddressSetEvent(REGISTRANT, BigInt.fromI32(2), UPDATED_META_ADDRESS);

    handleStealthMetaAddressSet(mph);
    handleStealthMetaAddressSet(dks);

    assert.entityCount('StealthMetaAddress', 2);
    assert.fieldEquals(
      'StealthMetaAddress',
      registryId(REGISTRANT.toHexString(), BigInt.fromI32(1)),
      'stealthMetaAddress',
      META_ADDRESS.toHexString(),
    );
    assert.fieldEquals(
      'StealthMetaAddress',
      registryId(REGISTRANT.toHexString(), BigInt.fromI32(2)),
      'stealthMetaAddress',
      UPDATED_META_ADDRESS.toHexString(),
    );
  });

  test('an update overwrites the existing entity instead of creating a second one', () => {
    handleStealthMetaAddressSet(
      createStealthMetaAddressSetEvent(REGISTRANT, SCHEME_ID, META_ADDRESS),
    );
    handleStealthMetaAddressSet(
      createStealthMetaAddressSetEvent(
        REGISTRANT,
        SCHEME_ID,
        UPDATED_META_ADDRESS,
        201,
        1700001500,
      ),
    );

    assert.entityCount('StealthMetaAddress', 1);
    let id = registryId(REGISTRANT.toHexString(), SCHEME_ID);
    assert.fieldEquals(
      'StealthMetaAddress',
      id,
      'stealthMetaAddress',
      UPDATED_META_ADDRESS.toHexString(),
    );
    assert.fieldEquals('StealthMetaAddress', id, 'blockNumber', '201');
  });

  test('duplicate delivery is idempotent', () => {
    let event = createStealthMetaAddressSetEvent(REGISTRANT, SCHEME_ID, META_ADDRESS);

    handleStealthMetaAddressSet(event);
    handleStealthMetaAddressSet(event);

    assert.entityCount('StealthMetaAddress', 1);
  });

  test('replay reads the same id derived from registrant and scheme id', () => {
    handleStealthMetaAddressSet(
      createStealthMetaAddressSetEvent(REGISTRANT, SCHEME_ID, META_ADDRESS),
    );
    // Same registrant + scheme, delivered again from a different block/tx.
    handleStealthMetaAddressSet(
      createStealthMetaAddressSetEvent(
        REGISTRANT,
        SCHEME_ID,
        META_ADDRESS,
        202,
        1700001800,
        Bytes.fromHexString('0xcccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc'),
      ),
    );

    assert.entityCount('StealthMetaAddress', 1);
  });
});

describe('handleNonceIncremented', () => {
  beforeEach(() => {
    clearStore();
  });

  test('does not create any entity on its own', () => {
    handleNonceIncremented(createNonceIncrementedEvent(REGISTRANT, BigInt.fromI32(5)));

    assert.entityCount('StealthMetaAddress', 0);
  });

  test('leaves the stored meta-address untouched', () => {
    handleStealthMetaAddressSet(
      createStealthMetaAddressSetEvent(REGISTRANT, SCHEME_ID, META_ADDRESS),
    );
    handleNonceIncremented(createNonceIncrementedEvent(REGISTRANT, BigInt.fromI32(1)));

    assert.entityCount('StealthMetaAddress', 1);
    let id = registryId(REGISTRANT.toHexString(), SCHEME_ID);
    assert.fieldEquals('StealthMetaAddress', id, 'stealthMetaAddress', META_ADDRESS.toHexString());
  });

  test('is idempotent across duplicate delivery and replay', () => {
    let event = createNonceIncrementedEvent(REGISTRANT, BigInt.fromI32(2));

    handleNonceIncremented(event);
    handleNonceIncremented(event);

    assert.entityCount('StealthMetaAddress', 0);
  });
});
