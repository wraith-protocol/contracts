// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

/// @title IERC5564Announcer
/// @notice Interface for the ERC-5564 Stealth Address Messenger contract.
interface IERC5564Announcer {
    /// @notice Emitted when a stealth address announcement is made.
    /// @param schemeId The scheme ID (1 for secp256k1 with view tags).
    /// @param stealthAddress The generated stealth address.
    /// @param caller The address that made the announcement.
    /// @param ephemeralPubKey The ephemeral public key used to generate the stealth address.
    /// @param metadata Arbitrary metadata. The first byte MUST be the view tag.
    event Announcement(
        uint256 indexed schemeId,
        address indexed stealthAddress,
        address indexed caller,
        bytes ephemeralPubKey,
        bytes metadata
    );

    /// @notice Publishes a stealth address announcement.
    /// @param schemeId The scheme ID (1 for secp256k1 with view tags).
    /// @param stealthAddress The generated stealth address.
    /// @param ephemeralPubKey The ephemeral public key used to generate the stealth address.
    /// @param metadata Arbitrary metadata. The first byte MUST be the view tag.
    function announce(
        uint256 schemeId,
        address stealthAddress,
        bytes memory ephemeralPubKey,
        bytes memory metadata
    ) external;
}
