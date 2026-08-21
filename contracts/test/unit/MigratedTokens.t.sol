// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import { Test } from "forge-std/Test.sol";

import { MigratedERC721 } from "../../src/tokens/MigratedERC721.sol";
import { MigratedERC1155 } from "../../src/tokens/MigratedERC1155.sol";

contract NonReceiver { }

contract MigratedTokensTest is Test {
    function testERC721RecreatesContractOwnershipWithoutReceiverHook() public {
        MigratedERC721 token = new MigratedERC721("Migrated", "MIG", "", address(this));
        NonReceiver recipient = new NonReceiver();
        token.setMinter(address(this));

        token.mint(address(recipient), 42);

        assertEq(token.ownerOf(42), address(recipient));
    }

    function testMetadataCanBePermanentlyFrozen() public {
        MigratedERC721 token721 =
            new MigratedERC721("Migrated", "MIG", "ipfs://before/", address(this));
        MigratedERC1155 token1155 = new MigratedERC1155("ipfs://before/{id}", address(this));

        token721.freezeMetadata();
        token1155.freezeMetadata();

        vm.expectRevert(MigratedERC721.MetadataAlreadyFrozen.selector);
        token721.setBaseURI("ipfs://after/");
        vm.expectRevert(MigratedERC1155.MetadataAlreadyFrozen.selector);
        token1155.setBaseURI("ipfs://after/{id}");
        assertTrue(token721.metadataFrozen());
        assertTrue(token1155.metadataFrozen());
    }
}
