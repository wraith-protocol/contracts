import { Address, BigInt } from '@graphprotocol/graph-ts';
import { assert, beforeEach, clearStore, describe, test } from 'matchstick-as/assembly/index';

import {
  handleWithdrawERC20,
  handleWithdrawERC20Direct,
  handleWithdrawETHDirect,
  handleWithdrawETH,
} from '../mappings/withdrawer';
import {
  CALLER,
  DESTINATION,
  TOKEN_ADDRESS,
  TX_HASH,
  createWithdrawERC20Call,
  createWithdrawERC20DirectCall,
  createWithdrawETHDirectCall,
  createWithdrawETHCall,
} from './utils';

function withdrawalId(txHash: string, txIndex: i32): string {
  return txHash + '-' + BigInt.fromI32(txIndex).toString();
}

describe('handleWithdrawETH', () => {
  beforeEach(() => {
    clearStore();
  });

  test('records a sponsored native withdrawal with its fee', () => {
    let fee = BigInt.fromString('1000000000000000');
    handleWithdrawETH(createWithdrawETHCall(CALLER, DESTINATION, fee, 600, 1700005000, TX_HASH, 1));

    let id = withdrawalId(TX_HASH.toHexString(), 1);
    assert.fieldEquals('Withdrawal', id, 'token', Address.zero().toHexString());
    assert.fieldEquals('Withdrawal', id, 'destination', DESTINATION.toHexString());
    assert.fieldEquals('Withdrawal', id, 'sponsor', CALLER.toHexString());
    assert.fieldEquals('Withdrawal', id, 'sponsorFee', fee.toString());
    assert.fieldEquals('Withdrawal', id, 'amount', '0');
    assert.fieldEquals('Withdrawal', id, 'blockNumber', '600');
    assert.fieldEquals('Withdrawal', id, 'blockTimestamp', '1700005000');
    assert.fieldEquals('Withdrawal', id, 'transactionHash', TX_HASH.toHexString());
  });

  test('zero-fee sponsored withdrawal keeps a zero sponsor fee', () => {
    handleWithdrawETH(createWithdrawETHCall(CALLER, DESTINATION, BigInt.zero()));

    assert.fieldEquals('Withdrawal', withdrawalId(TX_HASH.toHexString(), 0), 'sponsorFee', '0');
  });

  test('duplicate delivery is idempotent', () => {
    let call = createWithdrawETHCall(CALLER, DESTINATION, BigInt.fromString('5'));

    handleWithdrawETH(call);
    handleWithdrawETH(call);

    assert.entityCount('Withdrawal', 1);
  });

  test('replay with a different block yields one entity', () => {
    handleWithdrawETH(
      createWithdrawETHCall(
        CALLER,
        DESTINATION,
        BigInt.fromString('5'),
        600,
        1700005000,
        TX_HASH,
        0,
      ),
    );
    handleWithdrawETH(
      createWithdrawETHCall(
        CALLER,
        DESTINATION,
        BigInt.fromString('5'),
        601,
        1700005001,
        TX_HASH,
        0,
      ),
    );

    assert.entityCount('Withdrawal', 1);
  });
});

describe('handleWithdrawERC20', () => {
  beforeEach(() => {
    clearStore();
  });

  test('records token, destination and sponsor fee', () => {
    let fee = BigInt.fromString('2500');
    handleWithdrawERC20(createWithdrawERC20Call(CALLER, TOKEN_ADDRESS, DESTINATION, fee));

    let id = withdrawalId(TX_HASH.toHexString(), 0);
    assert.fieldEquals('Withdrawal', id, 'token', TOKEN_ADDRESS.toHexString());
    assert.fieldEquals('Withdrawal', id, 'destination', DESTINATION.toHexString());
    assert.fieldEquals('Withdrawal', id, 'sponsor', CALLER.toHexString());
    assert.fieldEquals('Withdrawal', id, 'sponsorFee', fee.toString());
  });

  test('duplicate delivery is idempotent', () => {
    let call = createWithdrawERC20Call(CALLER, TOKEN_ADDRESS, DESTINATION, BigInt.fromString('1'));

    handleWithdrawERC20(call);
    handleWithdrawERC20(call);

    assert.entityCount('Withdrawal', 1);
  });

  test('replay yields one entity', () => {
    handleWithdrawERC20(
      createWithdrawERC20Call(CALLER, TOKEN_ADDRESS, DESTINATION, BigInt.fromString('1')),
    );
    handleWithdrawERC20(
      createWithdrawERC20Call(
        CALLER,
        TOKEN_ADDRESS,
        DESTINATION,
        BigInt.fromString('1'),
        601,
        1700005001,
        TX_HASH,
        0,
      ),
    );

    assert.entityCount('Withdrawal', 1);
  });
});

describe('handleWithdrawETHDirect', () => {
  beforeEach(() => {
    clearStore();
  });

  test('records a self-funded native withdrawal with a zero sponsor fee', () => {
    handleWithdrawETHDirect(createWithdrawETHDirectCall(CALLER, DESTINATION));

    let id = withdrawalId(TX_HASH.toHexString(), 0);
    assert.fieldEquals('Withdrawal', id, 'token', Address.zero().toHexString());
    assert.fieldEquals('Withdrawal', id, 'destination', DESTINATION.toHexString());
    assert.fieldEquals('Withdrawal', id, 'sponsor', CALLER.toHexString());
    assert.fieldEquals('Withdrawal', id, 'sponsorFee', '0');
    assert.fieldEquals('Withdrawal', id, 'amount', '0');
  });

  test('duplicate delivery is idempotent', () => {
    let call = createWithdrawETHDirectCall(CALLER, DESTINATION);

    handleWithdrawETHDirect(call);
    handleWithdrawETHDirect(call);

    assert.entityCount('Withdrawal', 1);
  });
});

describe('handleWithdrawERC20Direct', () => {
  beforeEach(() => {
    clearStore();
  });

  test('records a self-funded token withdrawal with a zero sponsor fee', () => {
    handleWithdrawERC20Direct(createWithdrawERC20DirectCall(CALLER, TOKEN_ADDRESS, DESTINATION));

    let id = withdrawalId(TX_HASH.toHexString(), 0);
    assert.fieldEquals('Withdrawal', id, 'token', TOKEN_ADDRESS.toHexString());
    assert.fieldEquals('Withdrawal', id, 'destination', DESTINATION.toHexString());
    assert.fieldEquals('Withdrawal', id, 'sponsor', CALLER.toHexString());
    assert.fieldEquals('Withdrawal', id, 'sponsorFee', '0');
  });

  test('duplicate delivery is idempotent', () => {
    let call = createWithdrawERC20DirectCall(CALLER, TOKEN_ADDRESS, DESTINATION);

    handleWithdrawERC20Direct(call);
    handleWithdrawERC20Direct(call);

    assert.entityCount('Withdrawal', 1);
  });

  test('replay yields one entity', () => {
    handleWithdrawERC20Direct(createWithdrawERC20DirectCall(CALLER, TOKEN_ADDRESS, DESTINATION));
    handleWithdrawERC20Direct(
      createWithdrawERC20DirectCall(
        CALLER,
        TOKEN_ADDRESS,
        DESTINATION,
        601,
        1700005001,
        TX_HASH,
        0,
      ),
    );

    assert.entityCount('Withdrawal', 1);
  });
});
