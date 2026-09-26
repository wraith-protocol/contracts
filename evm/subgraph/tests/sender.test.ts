import { Address, BigInt, Bytes } from '@graphprotocol/graph-ts';
import { assert, beforeEach, clearStore, describe, test } from 'matchstick-as/assembly/index';

import { Send } from '../generated/schema';
import {
  handleBatchSendERC20,
  handleBatchSendETH,
  handleSendERC20,
  handleSendETH,
} from '../mappings/sender';
import {
  CALLER,
  STEALTH_ADDRESS,
  TOKEN_ADDRESS,
  TX_HASH,
  createBatchSendERC20Call,
  createBatchSendETHCall,
  createSendERC20Call,
  createSendETHCall,
} from './utils';

const SCHEME_ID = BigInt.fromI32(1);
const EPHEMERAL_PUB_KEY = Bytes.fromHexString('0x02b1c2d3');
const METADATA = Bytes.fromHexString('0xdeadbeef');
const SECOND_STEALTH = Address.fromString('0xcccc0000000000000000000000000000000000ff');

function sendId(txHash: Bytes, txIndex: i32): string {
  return txHash.toHexString() + '-' + BigInt.fromI32(txIndex).toString();
}

describe('handleSendETH', () => {
  beforeEach(() => {
    clearStore();
  });

  test('creates a Send keyed by transaction hash and index for a native transfer', () => {
    let value = BigInt.fromString('1000000000000000000');
    handleSendETH(
      createSendETHCall(
        CALLER,
        SCHEME_ID,
        STEALTH_ADDRESS,
        EPHEMERAL_PUB_KEY,
        METADATA,
        value,
        500,
        1700004000,
        TX_HASH,
        2,
      ),
    );

    let id = sendId(TX_HASH, 2);
    assert.fieldEquals('Send', id, 'sender', CALLER.toHexString());
    assert.fieldEquals('Send', id, 'token', Address.zero().toHexString());
    assert.fieldEquals('Send', id, 'schemeId', SCHEME_ID.toString());
    assert.fieldEquals('Send', id, 'totalAmount', value.toString());
    assert.fieldEquals('Send', id, 'blockNumber', '500');
    assert.fieldEquals('Send', id, 'blockTimestamp', '1700004000');
    assert.fieldEquals('Send', id, 'transactionHash', TX_HASH.toHexString());
  });

  test('records the single stealth recipient, amount and metadata', () => {
    let value = BigInt.fromString('42');
    handleSendETH(
      createSendETHCall(CALLER, SCHEME_ID, STEALTH_ADDRESS, EPHEMERAL_PUB_KEY, METADATA, value),
    );

    let send = Send.load(sendId(TX_HASH, 0))!;
    assert.i32Equals(send.stealthAddresses.length, 1, 'one recipient expected');
    assert.bytesEquals(send.stealthAddresses[0], STEALTH_ADDRESS, 'recipient mismatch');
    assert.i32Equals(send.amounts.length, 1, 'one amount expected');
    assert.bigIntEquals(send.amounts[0], value, 'amount mismatch');
    assert.i32Equals(send.metadatas.length, 1, 'one metadata expected');
    assert.bytesEquals(send.metadatas[0], METADATA, 'metadata mismatch');
  });

  test('duplicate delivery is idempotent', () => {
    let call = createSendETHCall(
      CALLER,
      SCHEME_ID,
      STEALTH_ADDRESS,
      EPHEMERAL_PUB_KEY,
      METADATA,
      BigInt.fromString('7'),
    );

    handleSendETH(call);
    handleSendETH(call);

    assert.entityCount('Send', 1);
  });

  test('replay with a different block still resolves to one entity', () => {
    handleSendETH(
      createSendETHCall(
        CALLER,
        SCHEME_ID,
        STEALTH_ADDRESS,
        EPHEMERAL_PUB_KEY,
        METADATA,
        BigInt.fromString('7'),
        500,
        1700004000,
        TX_HASH,
        0,
      ),
    );
    handleSendETH(
      createSendETHCall(
        CALLER,
        SCHEME_ID,
        STEALTH_ADDRESS,
        EPHEMERAL_PUB_KEY,
        METADATA,
        BigInt.fromString('7'),
        501,
        1700004001,
        TX_HASH,
        0,
      ),
    );

    assert.entityCount('Send', 1);
  });

  test('different transaction indices yield distinct entity ids', () => {
    handleSendETH(
      createSendETHCall(
        CALLER,
        SCHEME_ID,
        STEALTH_ADDRESS,
        EPHEMERAL_PUB_KEY,
        METADATA,
        BigInt.fromString('1'),
        500,
        1700004000,
        TX_HASH,
        0,
      ),
    );
    handleSendETH(
      createSendETHCall(
        CALLER,
        SCHEME_ID,
        SECOND_STEALTH,
        EPHEMERAL_PUB_KEY,
        METADATA,
        BigInt.fromString('2'),
        500,
        1700004000,
        TX_HASH,
        1,
      ),
    );

    assert.entityCount('Send', 2);
  });
});

describe('handleSendERC20', () => {
  beforeEach(() => {
    clearStore();
  });

  test('captures token, amount and total amount', () => {
    let amount = BigInt.fromString('5000000');
    handleSendERC20(
      createSendERC20Call(
        CALLER,
        TOKEN_ADDRESS,
        amount,
        SCHEME_ID,
        STEALTH_ADDRESS,
        EPHEMERAL_PUB_KEY,
        METADATA,
      ),
    );

    let id = sendId(TX_HASH, 0);
    assert.fieldEquals('Send', id, 'token', TOKEN_ADDRESS.toHexString());
    assert.fieldEquals('Send', id, 'totalAmount', amount.toString());
    assert.fieldEquals('Send', id, 'schemeId', SCHEME_ID.toString());
    assert.fieldEquals('Send', id, 'sender', CALLER.toHexString());
  });

  test('an optional ETH gas tip does not change the recorded token amount', () => {
    let amount = BigInt.fromString('5000000');
    let gasTip = BigInt.fromString('1000000000000000');
    handleSendERC20(
      createSendERC20Call(
        CALLER,
        TOKEN_ADDRESS,
        amount,
        SCHEME_ID,
        STEALTH_ADDRESS,
        EPHEMERAL_PUB_KEY,
        METADATA,
        gasTip,
      ),
    );

    let send = Send.load(sendId(TX_HASH, 0))!;
    assert.bigIntEquals(send.totalAmount, amount, 'token amount must exclude the tip');
    assert.bigIntEquals(send.amounts[0], amount, 'amount must exclude the tip');
  });

  test('duplicate delivery is idempotent', () => {
    let call = createSendERC20Call(
      CALLER,
      TOKEN_ADDRESS,
      BigInt.fromString('5'),
      SCHEME_ID,
      STEALTH_ADDRESS,
      EPHEMERAL_PUB_KEY,
      METADATA,
    );

    handleSendERC20(call);
    handleSendERC20(call);

    assert.entityCount('Send', 1);
  });

  test('replay produces a single entity', () => {
    handleSendERC20(
      createSendERC20Call(
        CALLER,
        TOKEN_ADDRESS,
        BigInt.fromString('5'),
        SCHEME_ID,
        STEALTH_ADDRESS,
        EPHEMERAL_PUB_KEY,
        METADATA,
      ),
    );
    handleSendERC20(
      createSendERC20Call(
        CALLER,
        TOKEN_ADDRESS,
        BigInt.fromString('5'),
        SCHEME_ID,
        STEALTH_ADDRESS,
        EPHEMERAL_PUB_KEY,
        METADATA,
        BigInt.zero(),
        501,
        1700004001,
        TX_HASH,
        0,
      ),
    );

    assert.entityCount('Send', 1);
  });
});

describe('handleBatchSendETH', () => {
  beforeEach(() => {
    clearStore();
  });

  test('records all recipients and amounts from a batch', () => {
    let recipients = [STEALTH_ADDRESS, SECOND_STEALTH];
    let amounts = [BigInt.fromString('10'), BigInt.fromString('20')];
    let metadatas = [METADATA, Bytes.fromHexString('0xc0ffee')];
    let pubKeys = [EPHEMERAL_PUB_KEY, EPHEMERAL_PUB_KEY];

    handleBatchSendETH(
      createBatchSendETHCall(
        CALLER,
        SCHEME_ID,
        recipients,
        pubKeys,
        metadatas,
        amounts,
        BigInt.fromString('30'),
      ),
    );

    let send = Send.load(sendId(TX_HASH, 0))!;
    assert.i32Equals(send.stealthAddresses.length, 2, 'recipient count mismatch');
    assert.bytesEquals(send.stealthAddresses[0], STEALTH_ADDRESS);
    assert.bytesEquals(send.stealthAddresses[1], SECOND_STEALTH);
    assert.i32Equals(send.amounts.length, 2, 'amount count mismatch');
    assert.bigIntEquals(send.amounts[0], BigInt.fromString('10'));
    assert.bigIntEquals(send.amounts[1], BigInt.fromString('20'));
    assert.i32Equals(send.metadatas.length, 2, 'metadata count mismatch');
    assert.bytesEquals(send.metadatas[1], Bytes.fromHexString('0xc0ffee'));
    assert.bigIntEquals(send.totalAmount, BigInt.fromString('30'));
  });

  test('duplicate delivery of a batch is idempotent', () => {
    let call = createBatchSendETHCall(
      CALLER,
      SCHEME_ID,
      [STEALTH_ADDRESS, SECOND_STEALTH],
      [EPHEMERAL_PUB_KEY, EPHEMERAL_PUB_KEY],
      [METADATA, METADATA],
      [BigInt.fromString('1'), BigInt.fromString('2')],
      BigInt.fromString('3'),
    );

    handleBatchSendETH(call);
    handleBatchSendETH(call);

    assert.entityCount('Send', 1);
  });

  test('replay of a batch yields one entity with the original arrays', () => {
    handleBatchSendETH(
      createBatchSendETHCall(
        CALLER,
        SCHEME_ID,
        [STEALTH_ADDRESS, SECOND_STEALTH],
        [EPHEMERAL_PUB_KEY, EPHEMERAL_PUB_KEY],
        [METADATA, METADATA],
        [BigInt.fromString('1'), BigInt.fromString('2')],
        BigInt.fromString('3'),
      ),
    );
    handleBatchSendETH(
      createBatchSendETHCall(
        CALLER,
        SCHEME_ID,
        [STEALTH_ADDRESS, SECOND_STEALTH],
        [EPHEMERAL_PUB_KEY, EPHEMERAL_PUB_KEY],
        [METADATA, METADATA],
        [BigInt.fromString('1'), BigInt.fromString('2')],
        BigInt.fromString('3'),
        501,
        1700004001,
        TX_HASH,
        0,
      ),
    );

    assert.entityCount('Send', 1);
    let send = Send.load(sendId(TX_HASH, 0))!;
    assert.i32Equals(send.amounts.length, 2);
  });
});

describe('handleBatchSendERC20', () => {
  beforeEach(() => {
    clearStore();
  });

  test('sums the batch amounts into totalAmount', () => {
    let amounts = [BigInt.fromString('100'), BigInt.fromString('250')];

    handleBatchSendERC20(
      createBatchSendERC20Call(
        CALLER,
        TOKEN_ADDRESS,
        SCHEME_ID,
        [STEALTH_ADDRESS, SECOND_STEALTH],
        [EPHEMERAL_PUB_KEY, EPHEMERAL_PUB_KEY],
        [METADATA, METADATA],
        amounts,
      ),
    );

    let send = Send.load(sendId(TX_HASH, 0))!;
    assert.bigIntEquals(send.totalAmount, BigInt.fromString('350'));
    assert.fieldEquals('Send', sendId(TX_HASH, 0), 'token', TOKEN_ADDRESS.toHexString());
    assert.i32Equals(send.amounts.length, 2);
  });

  test('duplicate delivery of a token batch is idempotent', () => {
    let call = createBatchSendERC20Call(
      CALLER,
      TOKEN_ADDRESS,
      SCHEME_ID,
      [STEALTH_ADDRESS, SECOND_STEALTH],
      [EPHEMERAL_PUB_KEY, EPHEMERAL_PUB_KEY],
      [METADATA, METADATA],
      [BigInt.fromString('1'), BigInt.fromString('2')],
    );

    handleBatchSendERC20(call);
    handleBatchSendERC20(call);

    assert.entityCount('Send', 1);
    let send = Send.load(sendId(TX_HASH, 0))!;
    assert.bigIntEquals(send.totalAmount, BigInt.fromString('3'));
  });

  test('replay of a token batch yields one entity', () => {
    handleBatchSendERC20(
      createBatchSendERC20Call(
        CALLER,
        TOKEN_ADDRESS,
        SCHEME_ID,
        [STEALTH_ADDRESS, SECOND_STEALTH],
        [EPHEMERAL_PUB_KEY, EPHEMERAL_PUB_KEY],
        [METADATA, METADATA],
        [BigInt.fromString('1'), BigInt.fromString('2')],
      ),
    );
    handleBatchSendERC20(
      createBatchSendERC20Call(
        CALLER,
        TOKEN_ADDRESS,
        SCHEME_ID,
        [STEALTH_ADDRESS, SECOND_STEALTH],
        [EPHEMERAL_PUB_KEY, EPHEMERAL_PUB_KEY],
        [METADATA, METADATA],
        [BigInt.fromString('1'), BigInt.fromString('2')],
        BigInt.zero(),
        501,
        1700004001,
        TX_HASH,
        0,
      ),
    );

    assert.entityCount('Send', 1);
  });
});
