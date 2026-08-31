import { BigInt, Address, Bytes, ethereum } from '@graphprotocol/graph-ts';
import {
  SendETHCall,
  SendERC20Call,
  BatchSendETHCall,
  BatchSendERC20Call,
} from '../generated/WraithSender/WraithSender';
import { Send } from '../generated/schema';

function sendId(call: ethereum.Call): string {
  return call.transaction.hash.toHexString() + '-' + call.transaction.index.toString();
}

function toBytes(values: Array<Address>): Array<Bytes> {
  let out = new Array<Bytes>(values.length);
  for (let i = 0; i < values.length; i++) {
    out[i] = values[i];
  }
  return out;
}

export function handleSendETH(call: SendETHCall): void {
  let entity = new Send(sendId(call));
  entity.sender = call.from;
  entity.token = Address.zero();
  entity.schemeId = call.inputs.schemeId;
  entity.stealthAddresses = toBytes([call.inputs.stealthAddress]);
  entity.amounts = [call.transaction.value];
  entity.metadatas = [call.inputs.metadata];
  entity.totalAmount = call.transaction.value;
  entity.blockNumber = call.block.number;
  entity.blockTimestamp = call.block.timestamp;
  entity.transactionHash = call.transaction.hash;
  entity.save();
}

export function handleSendERC20(call: SendERC20Call): void {
  let entity = new Send(sendId(call));
  entity.sender = call.from;
  entity.token = call.inputs.token;
  entity.schemeId = call.inputs.schemeId;
  entity.stealthAddresses = toBytes([call.inputs.stealthAddress]);
  entity.amounts = [call.inputs.amount];
  entity.metadatas = [call.inputs.metadata];
  entity.totalAmount = call.inputs.amount;
  entity.blockNumber = call.block.number;
  entity.blockTimestamp = call.block.timestamp;
  entity.transactionHash = call.transaction.hash;
  entity.save();
}

export function handleBatchSendETH(call: BatchSendETHCall): void {
  let entity = new Send(sendId(call));
  entity.sender = call.from;
  entity.token = Address.zero();
  entity.schemeId = call.inputs.schemeId;
  entity.stealthAddresses = toBytes(call.inputs.stealthAddresses);
  entity.amounts = call.inputs.amounts;
  entity.metadatas = call.inputs.metadatas;
  entity.totalAmount = call.transaction.value;
  entity.blockNumber = call.block.number;
  entity.blockTimestamp = call.block.timestamp;
  entity.transactionHash = call.transaction.hash;
  entity.save();
}

export function handleBatchSendERC20(call: BatchSendERC20Call): void {
  let entity = new Send(sendId(call));
  entity.sender = call.from;
  entity.token = call.inputs.token;
  entity.schemeId = call.inputs.schemeId;
  entity.stealthAddresses = toBytes(call.inputs.stealthAddresses);
  entity.amounts = call.inputs.amounts;
  entity.metadatas = call.inputs.metadatas;
  entity.totalAmount = sumAmounts(call.inputs.amounts);
  entity.blockNumber = call.block.number;
  entity.blockTimestamp = call.block.timestamp;
  entity.transactionHash = call.transaction.hash;
  entity.save();
}

function sumAmounts(values: Array<BigInt>): BigInt {
  let total = BigInt.fromI32(0);
  for (let i = 0; i < values.length; i++) {
    total = total.plus(values[i]);
  }
  return total;
}
