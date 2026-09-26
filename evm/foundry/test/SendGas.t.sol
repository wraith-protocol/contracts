// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

import {Test} from "forge-std/Test.sol";
import {WraithSender} from "@wraith/contracts/WraithSender.sol";
import {ERC5564Announcer} from "@wraith/contracts/ERC5564Announcer.sol";
import {ERC20Mock} from "@wraith/contracts/test/ERC20Mock.sol";

/// @notice Fuzz tests that exercise the hot paths so `forge snapshot` records
///         representative gas costs for the CI diff gate.
contract SendGas is Test {
    WraithSender internal sender;
    ERC20Mock internal token;
    address internal recipient;

    function setUp() public {
        ERC5564Announcer announcer = new ERC5564Announcer();
        sender = new WraithSender(address(announcer));
        token = new ERC20Mock();
        recipient = makeAddr("stealth");
        deal(address(this), 100 ether);
        token.mint(address(this), 100_000 ether);
        token.approve(address(sender), type(uint256).max);
    }

    function testFuzz_sendEth(uint96 amount) public {
        vm.assume(amount > 0 && amount <= address(this).balance);
        sender.sendETH{value: amount}(
            1,
            recipient,
            abi.encodePacked(bytes32(uint256(1)), bytes1(0x02)),
            hex"01"
        );
    }

    function testFuzz_sendErc20(uint96 amount) public {
        vm.assume(amount > 0 && amount <= token.balanceOf(address(this)));
        sender.sendERC20(
            address(token),
            amount,
            1,
            recipient,
            abi.encodePacked(bytes32(uint256(1)), bytes1(0x02)),
            hex"01"
        );
    }

    function testFuzz_batchSendErc20() public {
        address[] memory tos = new address[](2);
        bytes[] memory ephs = new bytes[](2);
        bytes[] memory metas = new bytes[](2);
        uint256[] memory amounts = new uint256[](2);
        tos[0] = recipient;
        tos[1] = makeAddr("stealth-2");
        ephs[0] = hex"010101010101010101010101010101010101010101010101010101010101010101";
        ephs[1] = ephs[0];
        metas[0] = hex"01";
        metas[1] = hex"01";
        amounts[0] = 1 ether;
        amounts[1] = 2 ether;
        sender.batchSendERC20(
            address(token), 1, tos, ephs, metas, amounts
        );
    }
}
