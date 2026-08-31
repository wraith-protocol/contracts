// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

import {Test} from "forge-std/Test.sol";
import {StdInvariant} from "forge-std/StdInvariant.sol";
import {WraithSender} from "@wraith/contracts/WraithSender.sol";
import {ERC5564Announcer} from "@wraith/contracts/ERC5564Announcer.sol";
import {ERC20Mock} from "@wraith/contracts/test/ERC20Mock.sol";

/// @notice Invariant suite for WraithSender. Verifies that no value or tokens
///         are ever retained/locked by the sender contract and that ERC-20
///         conservation holds across the whole pool of stealth recipients.
contract SenderBalanceConservation is Test {
    WraithSender internal sender;
    ERC5564Announcer internal announcer;
    ERC20Mock internal token;

    // Pool of stealth recipient addresses.
    address[] internal recipients;
    uint256 internal constant WINDS = 16;

    uint256 internal constant SCHEME_ID = 1;
    bytes internal constant METADATA =
        hex"0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";

    // Track total minted so we can assert conservation.
    uint256 internal totalMinted;

    function setUp() public {
        announcer = new ERC5564Announcer();
        sender = new WraithSender(address(announcer));
        token = new ERC20Mock();
        // The handler functions on this test contract are the fuzz targets; the
        // deployed contracts must not be fuzzed as independent targets.
        targetContract(address(this));
        excludeContract(address(announcer));
        excludeContract(address(sender));
        excludeContract(address(token));

        for (uint256 i = 0; i < WINDS; i++) {
            recipients.push(makeAddr(string.concat("recipient-", vm.toString(i))));
        }

        // Fund the handler so it can send value/tokens in mutators.
        deal(address(this), 1000 ether);
        token.mint(address(this), 1_000_000 ether);
        totalMinted = 1_000_000 ether;

        // Mint to recipients and track supply for conservation.
        for (uint256 i = 0; i < recipients.length; i++) {
            uint256 amount = 1_000_000 * (i + 1);
            token.mint(recipients[i], amount);
            totalMinted += amount;
        }
    }

    /// @notice Handler: send native ETH through the sender to a random recipient.
    function sendEth(uint256 amountSeed) public {
        uint256 bal = address(this).balance;
        uint256 amount = amountSeed % (bal > 0 ? bal : 1 ether);
        address to = recipients[amountSeed % recipients.length];
        bytes memory eph = abi.encodePacked(bytes32(amountSeed), bytes1(0x02));
        sender.sendETH{value: amount}(SCHEME_ID, to, eph, METADATA);
    }

    /// @notice Handler: send ERC-20 through the sender to a random recipient.
    function sendErc20(uint256 amountSeed) public {
        uint256 bal = token.balanceOf(address(this));
        uint256 amount = amountSeed % (bal > 0 ? bal : 1 ether);
        address to = recipients[amountSeed % recipients.length];
        bytes memory eph = abi.encodePacked(bytes32(amountSeed), bytes1(0x02));
        token.approve(address(sender), amount);
        sender.sendERC20(address(token), amount, SCHEME_ID, to, eph, METADATA);
    }

    /// @notice Handler: batch ETH send across several recipients.
    function batchSendEth(uint256 amountSeed) public {
        uint256 bal = address(this).balance;
        if (bal == 0) return;
        uint256 n = (amountSeed % 4) + 2; // 2..5 recipients
        uint256[] memory amounts = new uint256[](n);
        address[] memory tos = new address[](n);
        bytes[] memory ephs = new bytes[](n);
        bytes[] memory metas = new bytes[](n);

        // Bound total to what the handler actually holds so the call can't revert.
        uint256 available = bal / n;
        uint256 total;
        for (uint256 i = 0; i < n; i++) {
            uint256 amt = (amountSeed % available) + 1 wei;
            amounts[i] = amt;
            tos[i] = recipients[(i + amountSeed) % recipients.length];
            ephs[i] = abi.encodePacked(bytes32(amountSeed + i), bytes1(0x02));
            metas[i] = METADATA;
            total += amt;
        }
        sender.batchSendETH{value: total}(
            SCHEME_ID, tos, ephs, metas, amounts
        );
    }

    /// @notice Handler: batch ERC-20 send.
    function batchSendErc20(uint256 amountSeed) public {
        uint256 n = (amountSeed % 4) + 2; // 2..5 recipients
        uint256[] memory amounts = new uint256[](n);
        address[] memory tos = new address[](n);
        bytes[] memory ephs = new bytes[](n);
        bytes[] memory metas = new bytes[](n);

        uint256 available = token.balanceOf(address(this)) / n;
        if (available == 0) return;
        uint256 total;
        for (uint256 i = 0; i < n; i++) {
            uint256 amt = (amountSeed % available) + 1 wei;
            amounts[i] = amt;
            tos[i] = recipients[(i + amountSeed) % recipients.length];
            ephs[i] = abi.encodePacked(bytes32(amountSeed + i), bytes1(0x02));
            metas[i] = METADATA;
            total += amt;
        }
        token.approve(address(sender), total);
        sender.batchSendERC20(address(token), SCHEME_ID, tos, ephs, metas, amounts);
    }

    /// @notice Invariant: the sender contract never retains native value.
    function invariant_senderHoldsNoEth() public view {
        assertEq(address(sender).balance, 0, "sender retained ETH");
    }

    /// @notice Invariant: the sender contract never retains ERC-20.
    function invariant_senderHoldsNoTokens() public view {
        assertEq(token.balanceOf(address(sender)), 0, "sender retained tokens");
    }

    /// @notice Invariant: token supply is conserved across the whole pool.
    function invariant_tokenConservation() public view {
        uint256 held;
        for (uint256 i = 0; i < recipients.length; i++) {
            held += token.balanceOf(recipients[i]);
        }
        held += token.balanceOf(address(this));
        held += token.balanceOf(address(sender));
        assertEq(held, totalMinted, "ERC-20 conservation violated");
    }
}