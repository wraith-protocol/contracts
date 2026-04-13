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
    }
}
