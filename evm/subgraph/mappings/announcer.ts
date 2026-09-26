import { BigInt } from '@graphprotocol/graph-ts';
import { Announcement as AnnouncementEvent } from '../generated/ERC5564Announcer/ERC5564Announcer';
import { Announcement } from '../generated/schema';

export function handleAnnouncement(event: AnnouncementEvent): void {
  let entity = new Announcement(
    event.transaction.hash.toHexString() + '-' + event.logIndex.toString(),
  );
  entity.schemeId = event.params.schemeId;
  entity.stealthAddress = event.params.stealthAddress;
  entity.caller = event.params.caller;
  entity.ephemeralPubKey = event.params.ephemeralPubKey;
  entity.metadata = event.params.metadata;
  entity.blockNumber = event.block.number;
  entity.blockTimestamp = event.block.timestamp;
  entity.transactionHash = event.transaction.hash;
  entity.logIndex = event.logIndex;
  entity.save();
}
