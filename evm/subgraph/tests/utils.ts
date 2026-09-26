import { Address, BigInt, Bytes, ethereum } from '@graphprotocol/graph-ts';
import { newMockCallWithIO, newMockEvent } from 'matchstick-as/assembly/index';

import { Announcement as AnnouncementEvent } from '../generated/ERC5564Announcer/ERC5564Announcer';
import {
  NonceIncremented as NonceIncrementedEvent,
  StealthMetaAddressSet as StealthMetaAddressSetEvent,
} from '../generated/ERC6538Registry/ERC6538Registry';
import {
  NameRegistered as NameRegisteredEvent,
  NameReleased as NameReleasedEvent,
} from '../generated/WraithNames/WraithNames';
import {
  BatchSendERC20Call,
  BatchSendETHCall,
  SendERC20Call,
  SendETHCall,
} from '../generated/WraithSender/WraithSender';
import {
  WithdrawERC20Call,
  WithdrawERC20DirectCall,
  WithdrawETHDirectCall,
  WithdrawETHCall,
} from '../generated/WraithWithdrawer/WraithWithdrawer';

export const ANNOUNCER_ADDRESS = Address.fromString('0x8ae65c05e7eb48b9ba652781bc0a3dba09a484f3');
export const REGISTRY_ADDRESS = Address.fromString('0x953e6cedcdfae321796e7637d33653f6ce05c527');
export const SENDER_ADDRESS = Address.fromString('0x1111111111111111111111111111111111111111');
export const NAMES_ADDRESS = Address.fromString('0x2222222222222222222222222222222222222222');
export const WITHDRAWER_ADDRESS = Address.fromString('0x3333333333333333333333333333333333333333');

export const CALLER = Address.fromString('0xaaaa000000000000000000000000000000000001');
export const REGISTRANT = Address.fromString('0xbbbb000000000000000000000000000000000002');
export const STEALTH_ADDRESS = Address.fromString('0xcccc000000000000000000000000000000000003');
export const TOKEN_ADDRESS = Address.fromString('0xdddd000000000000000000000000000000000004');
export const DESTINATION = Address.fromString('0xeeee000000000000000000000000000000000005');

export const TX_HASH = Bytes.fromHexString(
  '0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
);
export const TX_HASH_REPLAY = Bytes.fromHexString(
  '0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
);

export function createBlock(number: i32, timestamp: i32): ethereum.Block {
  return new ethereum.Block(
    Bytes.empty(),
    Bytes.empty(),
    Bytes.empty(),
    Address.zero(),
    Bytes.empty(),
    Bytes.empty(),
    Bytes.empty(),
    BigInt.fromI32(number),
    BigInt.zero(),
    BigInt.zero(),
    BigInt.fromI32(timestamp),
    BigInt.zero(),
    BigInt.zero(),
    null,
    null,
  );
}

export function createTransaction(
  hash: Bytes,
  index: i32,
  from: Address,
  value: BigInt,
): ethereum.Transaction {
  return new ethereum.Transaction(
    hash,
    BigInt.fromI32(index),
    from,
    null,
    value,
    BigInt.zero(),
    BigInt.zero(),
    Bytes.empty(),
    BigInt.zero(),
  );
}

function eventParam(name: string, value: ethereum.Value): ethereum.EventParam {
  return new ethereum.EventParam(name, value);
}

export function createAnnouncementEvent(
  schemeId: BigInt,
  stealthAddress: Address,
  caller: Address,
  ephemeralPubKey: Bytes,
  metadata: Bytes,
  blockNumber: i32 = 100,
  blockTimestamp: i32 = 1700000000,
  txHash: Bytes = TX_HASH,
  logIndex: i32 = 0,
): AnnouncementEvent {
  let event = changetype<AnnouncementEvent>(newMockEvent());
  event.parameters = new Array<ethereum.EventParam>();
  event.parameters.push(eventParam('schemeId', ethereum.Value.fromUnsignedBigInt(schemeId)));
  event.parameters.push(eventParam('stealthAddress', ethereum.Value.fromAddress(stealthAddress)));
  event.parameters.push(eventParam('caller', ethereum.Value.fromAddress(caller)));
  event.parameters.push(eventParam('ephemeralPubKey', ethereum.Value.fromBytes(ephemeralPubKey)));
  event.parameters.push(eventParam('metadata', ethereum.Value.fromBytes(metadata)));
  event.address = ANNOUNCER_ADDRESS;
  event.block = createBlock(blockNumber, blockTimestamp);
  event.transaction = createTransaction(txHash, 0, caller, BigInt.zero());
  event.logIndex = BigInt.fromI32(logIndex);
  return event;
}

export function createStealthMetaAddressSetEvent(
  registrant: Address,
  schemeId: BigInt,
  stealthMetaAddress: Bytes,
  blockNumber: i32 = 200,
  blockTimestamp: i32 = 1700001000,
  txHash: Bytes = TX_HASH,
  logIndex: i32 = 0,
): StealthMetaAddressSetEvent {
  let event = changetype<StealthMetaAddressSetEvent>(newMockEvent());
  event.parameters = new Array<ethereum.EventParam>();
  event.parameters.push(eventParam('registrant', ethereum.Value.fromAddress(registrant)));
  event.parameters.push(eventParam('schemeId', ethereum.Value.fromUnsignedBigInt(schemeId)));
  event.parameters.push(
    eventParam('stealthMetaAddress', ethereum.Value.fromBytes(stealthMetaAddress)),
  );
  event.address = REGISTRY_ADDRESS;
  event.block = createBlock(blockNumber, blockTimestamp);
  event.transaction = createTransaction(txHash, 0, registrant, BigInt.zero());
  event.logIndex = BigInt.fromI32(logIndex);
  return event;
}

export function createNonceIncrementedEvent(
  registrant: Address,
  nonce: BigInt,
  blockNumber: i32 = 200,
  blockTimestamp: i32 = 1700001000,
  txHash: Bytes = TX_HASH,
  logIndex: i32 = 0,
): NonceIncrementedEvent {
  let event = changetype<NonceIncrementedEvent>(newMockEvent());
  event.parameters = new Array<ethereum.EventParam>();
  event.parameters.push(eventParam('registrant', ethereum.Value.fromAddress(registrant)));
  event.parameters.push(eventParam('nonce', ethereum.Value.fromUnsignedBigInt(nonce)));
  event.address = REGISTRY_ADDRESS;
  event.block = createBlock(blockNumber, blockTimestamp);
  event.transaction = createTransaction(txHash, 0, registrant, BigInt.zero());
  event.logIndex = BigInt.fromI32(logIndex);
  return event;
}

export function createNameRegisteredEvent(
  nameHash: Bytes,
  name: string,
  stealthMetaAddress: Bytes,
  blockNumber: i32 = 300,
  blockTimestamp: i32 = 1700002000,
  txHash: Bytes = TX_HASH,
  logIndex: i32 = 0,
): NameRegisteredEvent {
  let event = changetype<NameRegisteredEvent>(newMockEvent());
  event.parameters = new Array<ethereum.EventParam>();
  event.parameters.push(eventParam('nameHash', ethereum.Value.fromFixedBytes(nameHash)));
  event.parameters.push(eventParam('name', ethereum.Value.fromString(name)));
  event.parameters.push(
    eventParam('stealthMetaAddress', ethereum.Value.fromBytes(stealthMetaAddress)),
  );
  event.address = NAMES_ADDRESS;
  event.block = createBlock(blockNumber, blockTimestamp);
  event.transaction = createTransaction(txHash, 0, CALLER, BigInt.zero());
  event.logIndex = BigInt.fromI32(logIndex);
  return event;
}

export function createNameReleasedEvent(
  nameHash: Bytes,
  name: string,
  blockNumber: i32 = 400,
  blockTimestamp: i32 = 1700003000,
  txHash: Bytes = TX_HASH,
  logIndex: i32 = 0,
): NameReleasedEvent {
  let event = changetype<NameReleasedEvent>(newMockEvent());
  event.parameters = new Array<ethereum.EventParam>();
  event.parameters.push(eventParam('nameHash', ethereum.Value.fromFixedBytes(nameHash)));
  event.parameters.push(eventParam('name', ethereum.Value.fromString(name)));
  event.address = NAMES_ADDRESS;
  event.block = createBlock(blockNumber, blockTimestamp);
  event.transaction = createTransaction(txHash, 0, CALLER, BigInt.zero());
  event.logIndex = BigInt.fromI32(logIndex);
  return event;
}

function mockCall(
  to: Address,
  from: Address,
  inputs: Array<ethereum.EventParam>,
  blockNumber: i32,
  blockTimestamp: i32,
  txHash: Bytes,
  txIndex: i32,
  value: BigInt,
): ethereum.Call {
  let call = newMockCallWithIO(inputs, new Array<ethereum.EventParam>());
  call.to = to;
  call.from = from;
  call.block = createBlock(blockNumber, blockTimestamp);
  call.transaction = createTransaction(txHash, txIndex, from, value);
  return call;
}

export function createSendETHCall(
  sender: Address,
  schemeId: BigInt,
  stealthAddress: Address,
  ephemeralPubKey: Bytes,
  metadata: Bytes,
  value: BigInt,
  blockNumber: i32 = 500,
  blockTimestamp: i32 = 1700004000,
  txHash: Bytes = TX_HASH,
  txIndex: i32 = 0,
): SendETHCall {
  let inputs = new Array<ethereum.EventParam>();
  inputs.push(eventParam('schemeId', ethereum.Value.fromUnsignedBigInt(schemeId)));
  inputs.push(eventParam('stealthAddress', ethereum.Value.fromAddress(stealthAddress)));
  inputs.push(eventParam('ephemeralPubKey', ethereum.Value.fromBytes(ephemeralPubKey)));
  inputs.push(eventParam('metadata', ethereum.Value.fromBytes(metadata)));
  return changetype<SendETHCall>(
    mockCall(SENDER_ADDRESS, sender, inputs, blockNumber, blockTimestamp, txHash, txIndex, value),
  );
}

export function createSendERC20Call(
  sender: Address,
  token: Address,
  amount: BigInt,
  schemeId: BigInt,
  stealthAddress: Address,
  ephemeralPubKey: Bytes,
  metadata: Bytes,
  value: BigInt = BigInt.zero(),
  blockNumber: i32 = 500,
  blockTimestamp: i32 = 1700004000,
  txHash: Bytes = TX_HASH,
  txIndex: i32 = 0,
): SendERC20Call {
  let inputs = new Array<ethereum.EventParam>();
  inputs.push(eventParam('token', ethereum.Value.fromAddress(token)));
  inputs.push(eventParam('amount', ethereum.Value.fromUnsignedBigInt(amount)));
  inputs.push(eventParam('schemeId', ethereum.Value.fromUnsignedBigInt(schemeId)));
  inputs.push(eventParam('stealthAddress', ethereum.Value.fromAddress(stealthAddress)));
  inputs.push(eventParam('ephemeralPubKey', ethereum.Value.fromBytes(ephemeralPubKey)));
  inputs.push(eventParam('metadata', ethereum.Value.fromBytes(metadata)));
  return changetype<SendERC20Call>(
    mockCall(SENDER_ADDRESS, sender, inputs, blockNumber, blockTimestamp, txHash, txIndex, value),
  );
}

export function createBatchSendETHCall(
  sender: Address,
  schemeId: BigInt,
  stealthAddresses: Array<Address>,
  ephemeralPubKeys: Array<Bytes>,
  metadatas: Array<Bytes>,
  amounts: Array<BigInt>,
  value: BigInt,
  blockNumber: i32 = 500,
  blockTimestamp: i32 = 1700004000,
  txHash: Bytes = TX_HASH,
  txIndex: i32 = 0,
): BatchSendETHCall {
  let inputs = new Array<ethereum.EventParam>();
  inputs.push(eventParam('schemeId', ethereum.Value.fromUnsignedBigInt(schemeId)));
  inputs.push(eventParam('stealthAddresses', ethereum.Value.fromAddressArray(stealthAddresses)));
  inputs.push(eventParam('ephemeralPubKeys', ethereum.Value.fromBytesArray(ephemeralPubKeys)));
  inputs.push(eventParam('metadatas', ethereum.Value.fromBytesArray(metadatas)));
  inputs.push(eventParam('amounts', ethereum.Value.fromUnsignedBigIntArray(amounts)));
  return changetype<BatchSendETHCall>(
    mockCall(SENDER_ADDRESS, sender, inputs, blockNumber, blockTimestamp, txHash, txIndex, value),
  );
}

export function createBatchSendERC20Call(
  sender: Address,
  token: Address,
  schemeId: BigInt,
  stealthAddresses: Array<Address>,
  ephemeralPubKeys: Array<Bytes>,
  metadatas: Array<Bytes>,
  amounts: Array<BigInt>,
  value: BigInt = BigInt.zero(),
  blockNumber: i32 = 500,
  blockTimestamp: i32 = 1700004000,
  txHash: Bytes = TX_HASH,
  txIndex: i32 = 0,
): BatchSendERC20Call {
  let inputs = new Array<ethereum.EventParam>();
  inputs.push(eventParam('token', ethereum.Value.fromAddress(token)));
  inputs.push(eventParam('schemeId', ethereum.Value.fromUnsignedBigInt(schemeId)));
  inputs.push(eventParam('stealthAddresses', ethereum.Value.fromAddressArray(stealthAddresses)));
  inputs.push(eventParam('ephemeralPubKeys', ethereum.Value.fromBytesArray(ephemeralPubKeys)));
  inputs.push(eventParam('metadatas', ethereum.Value.fromBytesArray(metadatas)));
  inputs.push(eventParam('amounts', ethereum.Value.fromUnsignedBigIntArray(amounts)));
  return changetype<BatchSendERC20Call>(
    mockCall(SENDER_ADDRESS, sender, inputs, blockNumber, blockTimestamp, txHash, txIndex, value),
  );
}

export function createWithdrawETHCall(
  sponsor: Address,
  destination: Address,
  sponsorFee: BigInt,
  blockNumber: i32 = 600,
  blockTimestamp: i32 = 1700005000,
  txHash: Bytes = TX_HASH,
  txIndex: i32 = 0,
): WithdrawETHCall {
  let inputs = new Array<ethereum.EventParam>();
  inputs.push(eventParam('destination', ethereum.Value.fromAddress(destination)));
  inputs.push(eventParam('sponsorFee', ethereum.Value.fromUnsignedBigInt(sponsorFee)));
  return changetype<WithdrawETHCall>(
    mockCall(
      WITHDRAWER_ADDRESS,
      sponsor,
      inputs,
      blockNumber,
      blockTimestamp,
      txHash,
      txIndex,
      BigInt.zero(),
    ),
  );
}

export function createWithdrawERC20Call(
  sponsor: Address,
  token: Address,
  destination: Address,
  sponsorFee: BigInt,
  blockNumber: i32 = 600,
  blockTimestamp: i32 = 1700005000,
  txHash: Bytes = TX_HASH,
  txIndex: i32 = 0,
): WithdrawERC20Call {
  let inputs = new Array<ethereum.EventParam>();
  inputs.push(eventParam('token', ethereum.Value.fromAddress(token)));
  inputs.push(eventParam('destination', ethereum.Value.fromAddress(destination)));
  inputs.push(eventParam('sponsorFee', ethereum.Value.fromUnsignedBigInt(sponsorFee)));
  return changetype<WithdrawERC20Call>(
    mockCall(
      WITHDRAWER_ADDRESS,
      sponsor,
      inputs,
      blockNumber,
      blockTimestamp,
      txHash,
      txIndex,
      BigInt.zero(),
    ),
  );
}

export function createWithdrawETHDirectCall(
  sponsor: Address,
  destination: Address,
  blockNumber: i32 = 600,
  blockTimestamp: i32 = 1700005000,
  txHash: Bytes = TX_HASH,
  txIndex: i32 = 0,
): WithdrawETHDirectCall {
  let inputs = new Array<ethereum.EventParam>();
  inputs.push(eventParam('destination', ethereum.Value.fromAddress(destination)));
  return changetype<WithdrawETHDirectCall>(
    mockCall(
      WITHDRAWER_ADDRESS,
      sponsor,
      inputs,
      blockNumber,
      blockTimestamp,
      txHash,
      txIndex,
      BigInt.zero(),
    ),
  );
}

export function createWithdrawERC20DirectCall(
  sponsor: Address,
  token: Address,
  destination: Address,
  blockNumber: i32 = 600,
  blockTimestamp: i32 = 1700005000,
  txHash: Bytes = TX_HASH,
  txIndex: i32 = 0,
): WithdrawERC20DirectCall {
  let inputs = new Array<ethereum.EventParam>();
  inputs.push(eventParam('token', ethereum.Value.fromAddress(token)));
  inputs.push(eventParam('destination', ethereum.Value.fromAddress(destination)));
  return changetype<WithdrawERC20DirectCall>(
    mockCall(
      WITHDRAWER_ADDRESS,
      sponsor,
      inputs,
      blockNumber,
      blockTimestamp,
      txHash,
      txIndex,
      BigInt.zero(),
    ),
  );
}
