import { Bytes } from '@graphprotocol/graph-ts';
import {
  NameRegistered as NameRegisteredEvent,
  NameReleased as NameReleasedEvent,
} from '../generated/WraithNames/WraithNames';
import { Name } from '../generated/schema';

export function handleNameRegistered(event: NameRegisteredEvent): void {
  let id = event.params.nameHash.toHexString();
  let entity = Name.load(id);
  if (entity == null) {
    entity = new Name(id);
  }
  entity.name = event.params.name;
  entity.nameHash = event.params.nameHash;
  entity.stealthMetaAddress = event.params.stealthMetaAddress;
  entity.registeredAt = event.block.timestamp;
  entity.releasedAt = null;
  entity.active = true;
  entity.blockNumber = event.block.number;
  entity.transactionHash = event.transaction.hash;
  entity.save();
}

export function handleNameReleased(event: NameReleasedEvent): void {
  let id = event.params.nameHash.toHexString();
  let entity = Name.load(id);
  if (entity == null) {
    entity = new Name(id);
    entity.name = event.params.name;
    entity.nameHash = event.params.nameHash;
    entity.stealthMetaAddress = Bytes.empty();
    entity.registeredAt = event.block.timestamp;
  }
  entity.active = false;
  entity.releasedAt = event.block.timestamp;
  entity.blockNumber = event.block.number;
  entity.transactionHash = event.transaction.hash;
  entity.save();
}
