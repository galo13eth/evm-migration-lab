// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import { Script, console2 } from "forge-std/Script.sol";

import { ERC1155MigrationClaim } from "../src/ERC1155MigrationClaim.sol";
import { ERC721MigrationClaim } from "../src/ERC721MigrationClaim.sol";
import { MigratedERC721 } from "../src/tokens/MigratedERC721.sol";
import { MigratedERC1155 } from "../src/tokens/MigratedERC1155.sol";

contract DeployDestination is Script {
    error InvalidCampaignStandard(uint256 standard);
    error InvalidClaimSchedule(uint256 start, uint256 deadline, uint256 timestamp);
    error TimestampDoesNotFitUint64(uint256 value);

    function run() external returns (address claim, address token) {
        address deployer = vm.envAddress("DEPLOYER_ADDRESS");
        address admin = vm.envAddress("ADMIN_ADDRESS");
        uint256 standard = vm.envUint("CAMPAIGN_STANDARD");
        uint256 startValue = vm.envOr("CLAIM_START", block.timestamp + 1 days);
        uint256 deadlineValue = vm.envOr("CLAIM_DEADLINE", startValue + 30 days);
        if (startValue > type(uint64).max) revert TimestampDoesNotFitUint64(startValue);
        if (deadlineValue > type(uint64).max) revert TimestampDoesNotFitUint64(deadlineValue);
        if (startValue < block.timestamp + 1 hours || deadlineValue < startValue + 7 days) {
            revert InvalidClaimSchedule(startValue, deadlineValue, block.timestamp);
        }
        uint64 claimStart = uint64(startValue);
        uint64 claimDeadline = uint64(deadlineValue);

        vm.startBroadcast(deployer);
        if (standard == 1) {
            (claim, token) = _deploy721(deployer, admin, claimStart, claimDeadline);
        } else if (standard == 2) {
            (claim, token) = _deploy1155(deployer, admin, claimStart, claimDeadline);
        } else {
            revert InvalidCampaignStandard(standard);
        }
        vm.stopBroadcast();

        console2.log("MigrationClaim", claim);
        console2.log("DestinationToken", token);
        console2.log("TokenOwner", deployer);
        console2.log("TokenPendingOwner", admin == deployer ? address(0) : admin);
    }

    function _deploy721(address deployer, address admin, uint64 start, uint64 deadline)
        private
        returns (address claim, address token)
    {
        MigratedERC721 deployedToken = new MigratedERC721(
            "Migrated Demo Relics", "mRELIC", "ipfs://migrated-relics/", deployer
        );
        ERC721MigrationClaim deployedClaim = new ERC721MigrationClaim(
            vm.envBytes32("MIGRATION_ID"),
            vm.envUint("SOURCE_CHAIN_ID"),
            vm.envAddress("SOURCE_CONTRACT"),
            vm.envUint("SNAPSHOT_BLOCK"),
            vm.envBytes32("SOURCE_BLOCK_HASH"),
            block.chainid,
            address(deployedToken),
            start,
            deadline,
            admin
        );
        deployedToken.setMinter(address(deployedClaim));
        if (admin != deployer) deployedToken.transferOwnership(admin);
        return (address(deployedClaim), address(deployedToken));
    }

    function _deploy1155(address deployer, address admin, uint64 start, uint64 deadline)
        private
        returns (address claim, address token)
    {
        MigratedERC1155 deployedToken =
            new MigratedERC1155("ipfs://migrated-relics/{id}.json", deployer);
        ERC1155MigrationClaim deployedClaim = new ERC1155MigrationClaim(
            vm.envBytes32("MIGRATION_ID"),
            vm.envUint("SOURCE_CHAIN_ID"),
            vm.envAddress("SOURCE_CONTRACT"),
            vm.envUint("SNAPSHOT_BLOCK"),
            vm.envBytes32("SOURCE_BLOCK_HASH"),
            block.chainid,
            address(deployedToken),
            start,
            deadline,
            admin
        );
        deployedToken.setMinter(address(deployedClaim));
        if (admin != deployer) deployedToken.transferOwnership(admin);
        return (address(deployedClaim), address(deployedToken));
    }
}
