import { Address, BigInt } from '@graphprotocol/graph-ts';
import {
  WithdrawETHCall,
  WithdrawERC20Call,
  WithdrawETHDirectCall,
  WithdrawERC20DirectCall,
} from '../generated/WraithWithdrawer/WraithWithdrawer';
import { Withdrawal } from '../generated/schema';

export function handleWithdrawETH(call: WithdrawETHCall): void {
  let id = call.transaction.hash.toHexString() + '-' + call.transaction.index.toString();
  let entity = new Withdrawal(id);
  entity.token = Address.zero();
  entity.destination = call.inputs.destination;
  entity.sponsor = call.from;
  entity.sponsorFee = call.inputs.sponsorFee;
  // The exact amount moved to `destination` (balance - fee) is not observable
  // from the call payload; it is derived from the contract balance at index time.
  entity.amount = BigInt.zero();
  entity.blockNumber = call.block.number;
  entity.blockTimestamp = call.block.timestamp;
  entity.transactionHash = call.transaction.hash;
  entity.save();
}

export function handleWithdrawERC20(call: WithdrawERC20Call): void {
  let id = call.transaction.hash.toHexString() + '-' + call.transaction.index.toString();
  let entity = new Withdrawal(id);
  entity.token = call.inputs.token;
  entity.destination = call.inputs.destination;
  entity.sponsor = call.from;
  entity.sponsorFee = call.inputs.sponsorFee;
  entity.amount = BigInt.zero();
  entity.blockNumber = call.block.number;
  entity.blockTimestamp = call.block.timestamp;
  entity.transactionHash = call.transaction.hash;
  entity.save();
}

export function handleWithdrawETHDirect(call: WithdrawETHDirectCall): void {
  let id = call.transaction.hash.toHexString() + '-' + call.transaction.index.toString();
  let entity = new Withdrawal(id);
  entity.token = Address.zero();
  entity.destination = call.inputs.destination;
  entity.sponsor = call.from;
  entity.sponsorFee = BigInt.zero();
  // Full-balance sweep: amount equals the contract balance, not observable
  // from the call payload alone.
  entity.amount = BigInt.zero();
  entity.blockNumber = call.block.number;
  entity.blockTimestamp = call.block.timestamp;
  entity.transactionHash = call.transaction.hash;
  entity.save();
}

export function handleWithdrawERC20Direct(call: WithdrawERC20DirectCall): void {
  let id = call.transaction.hash.toHexString() + '-' + call.transaction.index.toString();
  let entity = new Withdrawal(id);
  entity.token = call.inputs.token;
  entity.destination = call.inputs.destination;
  entity.sponsor = call.from;
  entity.sponsorFee = BigInt.zero();
  entity.amount = BigInt.zero();
  entity.blockNumber = call.block.number;
  entity.blockTimestamp = call.block.timestamp;
  entity.transactionHash = call.transaction.hash;
  entity.save();
}
