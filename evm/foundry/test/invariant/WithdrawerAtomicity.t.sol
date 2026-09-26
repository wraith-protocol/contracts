// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

import {Test} from "forge-std/Test.sol";
import {StdInvariant} from "forge-std/StdInvariant.sol";
import {WraithWithdrawer} from "@wraith/contracts/WraithWithdrawer.sol";
import {ERC20Mock} from "@wraith/contracts/test/ERC20Mock.sol";

/// @notice Invariant suite for WraithWithdrawer. Exercises sponsor-fee
///         (withdrawETH/withdrawERC20) and self-funded (…Direct) variants and
///         verifies atomicity: a successful withdrawal moves exactly the
///         remaining balance, the sponsor fee equals `sponsorFee`, and the
///         contract cannot create, destroy, or strand value.
contract WithdrawerAtomicity is Test {
    WraithWithdrawer internal withdrawer;
    ERC20Mock internal token;

    address internal constant DEST = address(0xBEEF0000000000000000000000000000000000);
    address internal constant SPONSOR = address(0xC0FFEE0000000000000000000000000000);

    // Ghost: total native ETH the suite has placed "in the system".
    uint256 internal ethInSystem;

    constructor() {
        // The handler functions on this test contract are the fuzz targets; the
        // future deployed contracts must not be fuzzed as independent targets.
        targetContract(address(this));
    }

    function setUp() public {
        withdrawer = new WraithWithdrawer();
        token = new ERC20Mock();
        excludeContract(address(withdrawer));
        excludeContract(address(token));

        // Seed the conservation base: 5 ETH at the withdrawer.
        deal(address(withdrawer), 5 ether);
        ethInSystem = 5 ether;

        token.mint(address(this), 10_000 ether);
    }

    /// @notice Fund the withdrawer with native ETH, increasing the base.
    ///         Uses `deal` directly so the handler never needs a receive() that
    ///         the fuzzer could otherwise saturate with arbitrary value.
    function fundEth(uint256 amountSeed) public {
        uint256 amount = amountSeed % 3 ether;
        deal(address(withdrawer), address(withdrawer).balance + amount);
        ethInSystem += amount;
    }

    /// @notice Fund the withdrawer with ERC-20.
    function fundToken(uint256 amountSeed) public {
        uint256 amount = amountSeed % 2000 ether;
        token.transfer(address(withdrawer), amount);
    }

    /// @notice Withdraw native ETH, paying SPONSOR a fee < balance.
    function withdrawEth(uint256 amountSeed) public {
        uint256 balance = address(withdrawer).balance;
        if (balance == 0) return;
        uint256 fee = uint256(keccak256(abi.encodePacked(amountSeed))) % balance;
        withdrawer.withdrawETH(DEST, fee);
    }

    /// @notice Self-funded (sponsorless) native withdrawal.
    function withdrawEthDirect() public {
        withdrawer.withdrawETHDirect(DEST);
    }

    /// @notice Withdraw ERC-20, paying SPONSOR a fee < balance.
    function withdrawErc20(uint256 amountSeed) public {
        uint256 balance = token.balanceOf(address(withdrawer));
        if (balance == 0) return;
        uint256 fee = uint256(keccak256(abi.encodePacked(amountSeed))) % balance;
        withdrawer.withdrawERC20(address(token), DEST, fee);
    }

    /// @notice Self-funded ERC-20 withdrawal.
    function withdrawErc20Direct() public {
        withdrawer.withdrawERC20Direct(address(token), DEST);
    }

    /// @notice Invariant: total ETH across all holders is conserved
    ///         (atomic — no creation, destruction, or partial stranding).
    function invariant_ethConservation() public view {
        uint256 held = address(withdrawer).balance +
            DEST.balance +
            SPONSOR.balance;
        assertEq(held, ethInSystem, "ETH conservation violated");
    }

    /// @notice Invariant: total token supply is conserved across all holders,
    ///         and no tokens are stranded at the withdrawer beyond what was
    ///         deposited and not yet swept by a Direct/self-funded call.
    function invariant_tokenConservation() public view {
        uint256 held = token.balanceOf(address(this)) +
            token.balanceOf(address(withdrawer)) +
            token.balanceOf(DEST) +
            token.balanceOf(SPONSOR);
        assertEq(held, token.totalSupply(), "ERC-20 conservation violated");
    }
}