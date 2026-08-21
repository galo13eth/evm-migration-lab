// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import { Script } from "forge-std/Script.sol";

import { IMigrationClaim } from "../src/interfaces/IMigrationClaim.sol";

contract RegisterRoot is Script {
    error VersionDoesNotFitUint64(uint256 version);

    function run() external {
        IMigrationClaim claim = IMigrationClaim(vm.envAddress("MIGRATION_CLAIM"));
        uint256 version = vm.envUint("ROOT_VERSION");
        if (version > type(uint64).max) revert VersionDoesNotFitUint64(version);
        vm.startBroadcast(vm.envAddress("ADMIN_ADDRESS"));
        claim.setRoot(
            vm.envBytes32("MERKLE_ROOT"), vm.envBytes32("ARTIFACT_DIGEST"), uint64(version)
        );
        vm.stopBroadcast();
    }
}
