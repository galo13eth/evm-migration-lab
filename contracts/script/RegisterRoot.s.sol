// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import { Script } from "forge-std/Script.sol";

import { MigrationClaim } from "../src/MigrationClaim.sol";

contract RegisterRoot is Script {
    function run() external {
        MigrationClaim claim = MigrationClaim(vm.envAddress("MIGRATION_CLAIM"));
        vm.startBroadcast();
        claim.setRoot(vm.envBytes32("MERKLE_ROOT"), uint64(vm.envUint("ROOT_VERSION")));
        vm.stopBroadcast();
    }
}
