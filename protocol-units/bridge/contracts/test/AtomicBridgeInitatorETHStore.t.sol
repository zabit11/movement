// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.22;
pragma abicoder v2;

import {Test, console} from "forge-std/Test.sol";
import {AtomicBridgeInitiatorETHStore} from "../src/AtomicBridgeInitiatorETHStore.sol";
import {ProxyAdmin} from "@openzeppelin/contracts/proxy/transparent/ProxyAdmin.sol";
import {TransparentUpgradeableProxy} from "@openzeppelin/contracts/proxy/transparent/TransparentUpgradeableProxy.sol";
import {WETH10} from "../src/WETH/WETH10.sol";

contract AtomicBridgeInitiatorETHStoreTest is Test {
    AtomicBridgeInitiatorETHStore public atomicBridgeInitiatorETHStoreImplementation;
    WETH10 public weth;
    ProxyAdmin public proxyAdmin;
    TransparentUpgradeableProxy public proxy;
    AtomicBridgeInitiatorETHStore public atomicBridgeInitiatorETHStore;

    address public originator = address(1);
    // convert to bytes32
    bytes32 public recipient = keccak256(abi.encodePacked(address(2)));
    bytes32 public hashLock = keccak256(abi.encodePacked("secret"));
    uint256 public amount = 1 ether;
    uint256 public timeLock = 100;

    function setUp() public {
        // Deploy the WETH contract

        weth = new WETH10();
        // Deploy the atomicBridgeInitiatorETHStore contract with the WETH address
        atomicBridgeInitiatorETHStoreImplementation = new AtomicBridgeInitiatorETHStore();
        proxyAdmin = new ProxyAdmin(msg.sender);
        proxy = new TransparentUpgradeableProxy(
            address(atomicBridgeInitiatorETHStoreImplementation),
            address(proxyAdmin),
            abi.encodeWithSignature("initialize(address)", address(weth))
        );
        atomicBridgeInitiatorETHStore = AtomicBridgeInitiatorETHStore(payable(address(proxy)));
    }

    function testInitiateBridgeTransferWithEth() public {
        vm.deal(originator, 1 ether);
        vm.startPrank(originator);

        bytes32 bridgeTransferId = atomicBridgeInitiatorETHStore.initiateBridgeTransfer{value: amount}(
            0, // _wethAmount
            recipient,
            hashLock,
            timeLock
        );

        (
            uint256 transferAmount,
            address transferOriginator,
            bytes32 transferRecipient,
            bytes32 transferHashLock,
            uint256 transferTimeLock,
            bool completed
        ) = atomicBridgeInitiatorETHStore.bridgeTransfers(bridgeTransferId);

        assertFalse(completed);
        assertEq(transferAmount, amount);
        assertEq(transferOriginator, originator);
        assertEq(transferRecipient, recipient);
        assertEq(transferHashLock, hashLock);
        assertGt(transferTimeLock, block.timestamp);

        vm.stopPrank();
    }

    function testCompleteBridgeTransfer() public {
        bytes32 secret = "secret";
        bytes32 testHashLock = keccak256(abi.encodePacked(secret));

        vm.deal(originator, 1 ether);
        vm.startPrank(originator);

        bytes32 bridgeTransferId = atomicBridgeInitiatorETHStore.initiateBridgeTransfer{value: amount}(
            0, // _wethAmount is 0
            recipient,
            testHashLock,
            timeLock
        );

        vm.stopPrank();

        // vm.startPrank(msg.sender);
        atomicBridgeInitiatorETHStore.completeBridgeTransfer(bridgeTransferId, secret);

        (,,,,, bool completed1) = atomicBridgeInitiatorETHStore.bridgeTransfers(bridgeTransferId);
        assertTrue(completed1);

        (
            uint256 completedAmount,
            address completedOriginator,
            bytes32 completedRecipient,
            bytes32 completedHashLock,
            uint256 completedTimeLock,
            bool completedCompleted
        ) = atomicBridgeInitiatorETHStore.bridgeTransfers(bridgeTransferId);
        assertTrue(completedCompleted);
        assertEq(completedAmount, amount);
        assertEq(completedOriginator, originator);
        assertEq(completedRecipient, recipient);
        assertEq(completedHashLock, testHashLock);
        assertGt(completedTimeLock, block.timestamp);

        // vm.stopPrank();
    }

    function testInitiateBridgeTransferWithWeth() public {
        uint256 wethAmount = 1 ether; // use ethers unit

        vm.deal(originator, 1 ether);
        vm.startPrank(originator);
        weth.deposit{value: wethAmount}();
        weth.approve(address(atomicBridgeInitiatorETHStore), wethAmount);
        bytes32 bridgeTransferId =
            atomicBridgeInitiatorETHStore.initiateBridgeTransfer(wethAmount, recipient, hashLock, timeLock);

        (
            uint256 transferAmount,
            address transferOriginator,
            bytes32 transferRecipient,
            bytes32 transferHashLock,
            uint256 transferTimeLock,
            bool transferCompleted
        ) = atomicBridgeInitiatorETHStore.bridgeTransfers(bridgeTransferId);

        assertFalse(transferCompleted);
        assertEq(transferAmount, wethAmount);
        assertEq(transferOriginator, originator);
        assertEq(transferRecipient, recipient);
        assertEq(transferHashLock, hashLock);
        assertGt(transferTimeLock, block.timestamp);

        vm.stopPrank();
    }

    function testInitiateBridgeTransferWithEthAndWeth() public {
        uint256 wethAmount = 1 ether;
        uint256 ethAmount = 2 ether;
        uint256 totalAmount = wethAmount + ethAmount;

        // Ensure the originator has sufficient ETH
        vm.deal(originator, 100 ether);

        vm.startPrank(originator);
        // Ensure WETH contract is correctly funded and transfer WETH to originator
        weth.deposit{value: wethAmount}();

        assertEq(weth.balanceOf(originator), wethAmount, "WETH balance mismatch");
        vm.expectRevert();
        // Try to initiate bridge transfer
        atomicBridgeInitiatorETHStore.initiateBridgeTransfer{value: ethAmount}(
            wethAmount, recipient, hashLock, timeLock
        );
        weth.approve(address(atomicBridgeInitiatorETHStore), wethAmount);
        // Try to initiate bridge transfer
        bytes32 bridgeTransferId = atomicBridgeInitiatorETHStore.initiateBridgeTransfer{value: ethAmount}(
            wethAmount, recipient, hashLock, timeLock
        );

        // Fetch the details of the initiated bridge transfer
        (
            uint256 transferAmount,
            address transferOriginator,
            bytes32 transferRecipient,
            bytes32 transferHashLock,
            uint256 transferTimeLock,
            bool completed
        ) = atomicBridgeInitiatorETHStore.bridgeTransfers(bridgeTransferId);

        // Assertions
        assertFalse(completed, "Bridge transfer not completed");
        assertEq(transferAmount, totalAmount, "Transfer amount mismatch");
        assertEq(transferOriginator, originator, "Originator address mismatch");
        assertEq(transferRecipient, recipient, "Recipient address mismatch");
        assertEq(transferHashLock, hashLock, "HashLock mismatch");
        assertGt(transferTimeLock, block.timestamp, "TimeLock is not greater than current block timestamp");

        vm.stopPrank();
    }

    function testRefundBridgeTransfer() public {
        vm.deal(originator, 1 ether);
        vm.startPrank(originator);

        bytes32 bridgeTransferId = atomicBridgeInitiatorETHStore.initiateBridgeTransfer{value: amount}(
            0, // _wethAmount is 0
            recipient,
            hashLock,
            timeLock
        );
        console.log("balance of bridge", address(atomicBridgeInitiatorETHStore).balance);
        console.log("originator balance: ", address(originator).balance);
        // add gas balance to originator?
        vm.deal(originator, 1 ether);
        vm.warp(block.timestamp + timeLock + 1);
        atomicBridgeInitiatorETHStore.refundBridgeTransfer(bridgeTransferId);

        (,,,,, bool completed) = atomicBridgeInitiatorETHStore.bridgeTransfers(bridgeTransferId);
        assertTrue(completed);

        vm.stopPrank();
    }
}