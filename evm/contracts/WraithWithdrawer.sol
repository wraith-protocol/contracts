// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

/// @title WraithWithdrawer
/// @notice Delegation target for EIP-7702 gas-sponsored stealth address withdrawals.
///         A stealth EOA signs an EIP-7702 authorization delegating to this contract,
///         then a sponsor submits the transaction and pays gas. The contract transfers
///         assets from the stealth address to the destination and pays the sponsor
///         a gas fee from the transferred amount.
/// @dev This contract is meant to be delegated to, not called directly.
///      When delegated via EIP-7702, `address(this)` is the stealth EOA itself.
contract WraithWithdrawer is ReentrancyGuard {
    using SafeERC20 for IERC20;

    error InsufficientBalance();
    error TransferFailed();
    error FeeTooHigh();

    /// @notice Withdraw native ETH from the stealth address to a destination.
    ///         A portion is sent to the sponsor (msg.sender) as a gas fee.
    /// @param destination The address to receive the withdrawal.
    /// @param sponsorFee The amount of ETH to pay the sponsor for gas.
    function withdrawETH(
        address destination,
        uint256 sponsorFee
    ) external nonReentrant {
        uint256 balance = address(this).balance;
        if (balance == 0) revert InsufficientBalance();
        if (sponsorFee >= balance) revert FeeTooHigh();

        uint256 amount = balance - sponsorFee;

        if (sponsorFee > 0) {
            (bool feeSent, ) = msg.sender.call{value: sponsorFee}("");
            if (!feeSent) revert TransferFailed();
        }

        (bool sent, ) = destination.call{value: amount}("");
        if (!sent) revert TransferFailed();
    }

    /// @notice Withdraw ERC-20 tokens from the stealth address to a destination.
    ///         A portion is sent to the sponsor (msg.sender) as a gas fee.
    /// @param token The ERC-20 token contract address.
    /// @param destination The address to receive the withdrawal.
    /// @param sponsorFee The amount of tokens to pay the sponsor for gas.
    function withdrawERC20(
        address token,
        address destination,
        uint256 sponsorFee
    ) external nonReentrant {
        uint256 balance = IERC20(token).balanceOf(address(this));
        if (balance == 0) revert InsufficientBalance();
        if (sponsorFee >= balance) revert FeeTooHigh();

        uint256 amount = balance - sponsorFee;

        if (sponsorFee > 0) {
            IERC20(token).safeTransfer(msg.sender, sponsorFee);
        }

        IERC20(token).safeTransfer(destination, amount);
    }

    /// @notice Withdraw full native ETH balance without sponsor fee (self-funded gas).
    /// @param destination The address to receive the withdrawal.
    function withdrawETHDirect(address destination) external nonReentrant {
        uint256 balance = address(this).balance;
        if (balance == 0) revert InsufficientBalance();

        (bool sent, ) = destination.call{value: balance}("");
        if (!sent) revert TransferFailed();
    }

    /// @notice Withdraw full ERC-20 balance without sponsor fee (self-funded gas).
    /// @param token The ERC-20 token contract address.
    /// @param destination The address to receive the withdrawal.
    function withdrawERC20Direct(
        address token,
        address destination
    ) external nonReentrant {
        uint256 balance = IERC20(token).balanceOf(address(this));
        if (balance == 0) revert InsufficientBalance();

        IERC20(token).safeTransfer(destination, balance);
    }
}
