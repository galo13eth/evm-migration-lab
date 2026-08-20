// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import { Script, console2 } from "forge-std/Script.sol";

import { MigrationClaim } from "../src/MigrationClaim.sol";
import { MigratedERC721 } from "../src/tokens/MigratedERC721.sol";
import { MigratedERC1155 } from "../src/tokens/MigratedERC1155.sol";

contract DeployDestination is Script {
    function run()
        external
        returns (MigrationClaim claim, MigratedERC721 token721, MigratedERC1155 token1155)
    {
        address deployer = vm.envAddress("DEPLOYER_ADDRESS");
        address admin = vm.envAddress("ADMIN_ADDRESS");

        vm.startBroadcast();
        token721 = new MigratedERC721(
            "Migrated Demo Relics", "mRELIC", "ipfs://migrated-relics/", deployer
        );
        token1155 = new MigratedERC1155("ipfs://migrated-relics/{id}.json", deployer);
        claim = new MigrationClaim(
            vm.envBytes32("MIGRATION_ID"),
            vm.envUint("SOURCE_CHAIN_ID"),
            vm.envAddress("SOURCE_CONTRACT"),
            vm.envUint("SNAPSHOT_BLOCK"),
            address(token721),
            address(token1155),
            uint64(vm.envUint("CLAIM_START")),
            uint64(vm.envUint("CLAIM_DEADLINE")),
            admin
        );
        token721.setMinter(address(claim));
        token1155.setMinter(address(claim));
        if (admin != deployer) {
            token721.transferOwnership(admin);
            token1155.transferOwnership(admin);
        }
        vm.stopBroadcast();

        console2.log("MigrationClaim", address(claim));
        console2.log("MigratedERC721", address(token721));
        console2.log("MigratedERC1155", address(token1155));
    }
}
