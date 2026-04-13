// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

import {IERC5564Announcer} from "./interfaces/IERC5564Announcer.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

/// @title WraithSender
/// @notice Atomically transfers assets to stealth addresses and publishes announcements.
///         Supports single and batch sends for both native ETH and ERC-20 tokens.
///         ERC-20 sends accept an optional ETH gas tip that is forwarded to the stealth
///         address so the recipient can self-fund a withdrawal.
///         Prevents the failure case where funds land but the announcement is never published.
contract WraithSender is ReentrancyGuard {
    using SafeERC20 for IERC20;

    IERC5564Announcer public immutable announcer;

    error LengthMismatch();
    error InsufficientValue();
    error TipTransferFailed();

    constructor(address _announcer) {
        announcer = IERC5564Announcer(_announcer);
    }

    /// @notice Send native ETH to a stealth address and announce atomically.
    /// @param schemeId The stealth address scheme ID (1 for secp256k1).
    /// @param stealthAddress The generated stealth address.
    /// @param ephemeralPubKey The ephemeral public key used in generation.
    /// @param metadata Announcement metadata (first byte is view tag).
    function sendETH(
        uint256 schemeId,
        address stealthAddress,
        bytes calldata ephemeralPubKey,
        bytes calldata metadata
    ) external payable nonReentrant {
        (bool sent, ) = stealthAddress.call{value: msg.value}("");
        require(sent);
        announcer.announce(schemeId, stealthAddress, ephemeralPubKey, metadata);
    }

    /// @notice Send ERC-20 tokens to a stealth address and announce atomically.
    ///         Caller must approve this contract for the token amount first.
    ///         If msg.value > 0, the ETH is forwarded to the stealth address as a gas tip.
    /// @param token The ERC-20 token contract address.
    /// @param amount The amount of tokens to send.
    /// @param schemeId The stealth address scheme ID.
    /// @param stealthAddress The generated stealth address.
    /// @param ephemeralPubKey The ephemeral public key used in generation.
    /// @param metadata Announcement metadata (first byte is view tag).
    function sendERC20(
        address token,
        uint256 amount,
        uint256 schemeId,
        address stealthAddress,
        bytes calldata ephemeralPubKey,
        bytes calldata metadata
    ) external payable nonReentrant {
        IERC20(token).safeTransferFrom(msg.sender, stealthAddress, amount);

        if (msg.value > 0) {
            (bool sent, ) = stealthAddress.call{value: msg.value}("");
            if (!sent) revert TipTransferFailed();
        }

        announcer.announce(schemeId, stealthAddress, ephemeralPubKey, metadata);
    }

    /// @notice Batch send native ETH to multiple stealth addresses and announce each.
    ///         msg.value must equal the sum of all amounts.
    /// @param schemeId The stealth address scheme ID (same for all recipients).
    /// @param stealthAddresses Array of generated stealth addresses.
    /// @param ephemeralPubKeys Array of ephemeral public keys.
    /// @param metadatas Array of announcement metadata.
    /// @param amounts Array of ETH amounts for each recipient.
    function batchSendETH(
        uint256 schemeId,
        address[] calldata stealthAddresses,
        bytes[] calldata ephemeralPubKeys,
        bytes[] calldata metadatas,
        uint256[] calldata amounts
    ) external payable nonReentrant {
        uint256 len = stealthAddresses.length;
        if (
            ephemeralPubKeys.length != len ||
            metadatas.length != len ||
            amounts.length != len
        ) revert LengthMismatch();

        uint256 totalSent;
        for (uint256 i; i < len; ) {
            (bool sent, ) = stealthAddresses[i].call{value: amounts[i]}("");
            require(sent);
            announcer.announce(schemeId, stealthAddresses[i], ephemeralPubKeys[i], metadatas[i]);
            totalSent += amounts[i];
            unchecked { ++i; }
        }

        if (totalSent != msg.value) revert InsufficientValue();
    }

    /// @notice Batch send ERC-20 tokens to multiple stealth addresses and announce each.
    ///         Caller must approve this contract for the total amount first.
    ///         If msg.value > 0, the ETH is split equally across all stealth addresses as gas tips.
    /// @param token The ERC-20 token contract address (same token for all recipients).
    /// @param schemeId The stealth address scheme ID.
    /// @param stealthAddresses Array of generated stealth addresses.
    /// @param ephemeralPubKeys Array of ephemeral public keys.
    /// @param metadatas Array of announcement metadata.
    /// @param amounts Array of token amounts for each recipient.
    function batchSendERC20(
        address token,
        uint256 schemeId,
        address[] calldata stealthAddresses,
        bytes[] calldata ephemeralPubKeys,
        bytes[] calldata metadatas,
        uint256[] calldata amounts
    ) external payable nonReentrant {
        uint256 len = stealthAddresses.length;
        if (
            ephemeralPubKeys.length != len ||
            metadatas.length != len ||
            amounts.length != len
        ) revert LengthMismatch();

        uint256 tipPerRecipient = len > 0 ? msg.value / len : 0;

        for (uint256 i; i < len; ) {
            IERC20(token).safeTransferFrom(msg.sender, stealthAddresses[i], amounts[i]);

            if (tipPerRecipient > 0) {
                (bool sent, ) = stealthAddresses[i].call{value: tipPerRecipient}("");
                if (!sent) revert TipTransferFailed();
            }

            announcer.announce(schemeId, stealthAddresses[i], ephemeralPubKeys[i], metadatas[i]);
            unchecked { ++i; }
        }
    }
}
