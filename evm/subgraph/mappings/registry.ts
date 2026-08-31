import { BigInt } from '@graphprotocol/graph-ts';
import {
  StealthMetaAddressSet as StealthMetaAddressSetEvent,
  NonceIncremented as NonceIncrementedEvent,
} from '../generated/ERC6538Registry/ERC6538Registry';
import { StealthMetaAddress } from '../generated/schema';

export function handleStealthMetaAddressSet(event: StealthMetaAddressSetEvent): void {
  let id = event.params.registrant.toHexString() + '-' + event.params.schemeId.toString();
  let entity = StealthMetaAddress.load(id);
  if (entity == null) {
    entity = new StealthMetaAddress(id);
    entity.registrant = event.params.registrant;
    entity.schemeId = event.params.schemeId;
  }
  entity.stealthMetaAddress = event.params.stealthMetaAddress;
  entity.blockNumber = event.block.number;
  entity.blockTimestamp = event.block.timestamp;
  entity.transactionHash = event.transaction.hash;
  entity.save();
}

export function handleNonceIncremented(event: NonceIncrementedEvent): void {
  // Nonce increments are already recorded on-chain; nothing to index beyond the
  // registrant's latest meta-address, which is updated by StealthMetaAddressSet.
  // Kept as a no-op handler so future schemes can attach nonce history.
}
