// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

import {IERC5564Announcer} from "./interfaces/IERC5564Announcer.sol";

/// @title ERC5564Announcer
/// @notice Singleton contract for publishing stealth address announcements per ERC-5564.
/// @dev This contract is intentionally minimal — it only emits events. No access control,
///      no storage. Anyone can call announce(). The metadata format (view tag as first byte)
///      is enforced by convention in the SDK, not in this contract.
contract ERC5564Announcer is IERC5564Announcer {
    /// @inheritdoc IERC5564Announcer
    function announce(
        uint256 schemeId,
        address stealthAddress,
        bytes memory ephemeralPubKey,
        bytes memory metadata
    ) external {
        emit Announcement(schemeId, stealthAddress, msg.sender, ephemeralPubKey, metadata);

        // Also emit legacy v1-shaped announcement for indexer compatibility.
        // The legacy shape used topics ("announce", schemeId, stealthAddress)
        // and put `caller` in the data payload (non-indexed), resulting in
        // three topics. Emit here to avoid breaking existing indexers.
        emit LegacyAnnouncement(schemeId, stealthAddress, msg.sender, ephemeralPubKey, metadata);
    }
}
