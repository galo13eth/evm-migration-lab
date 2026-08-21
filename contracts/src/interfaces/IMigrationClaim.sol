// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

interface IMigrationClaim {
    struct ClaimData {
        uint256 tokenId;
        uint256 amount;
        address sourceOwner;
        address claimAuthority;
        address destinationRecipient;
        uint256 leafIndex;
    }

    error AlreadyClaimed(uint256 leafIndex);
    error ClaimWindowClosed(uint64 start, uint64 deadline, uint256 timestamp);
    error EmptyBatch();
    error ExpiredSignature(uint256 deadline, uint256 timestamp);
    error InvalidAddress(address value);
    error InvalidAmount(uint8 standard, uint256 amount);
    error InvalidClaimWindow(uint64 start, uint64 deadline, uint256 timestamp);
    error InvalidDestinationChain(uint256 expected, uint256 actual);
    error InvalidMigrationId();
    error InvalidNonce(address authority, uint256 expected, uint256 actual);
    error InvalidProof();
    error InvalidSignature(address signer);
    error InvalidSourceBlockHash();
    error InvalidVersion(uint64 current, uint64 proposed);
    error RootFrozen();
    error UnauthorizedClaimAuthority(address expected, address caller);
    error UnchangedRoot(bytes32 root);
    error ZeroArtifactDigest();
    error ZeroRoot();

    event Claimed(
        uint64 indexed version,
        uint256 indexed leafIndex,
        address indexed sourceOwner,
        address claimAuthority,
        address destinationRecipient,
        uint8 standard,
        uint256 tokenId,
        uint256 amount
    );
    event RootUpdated(
        bytes32 indexed previousRoot,
        bytes32 indexed newRoot,
        uint64 previousVersion,
        uint64 newVersion,
        bytes32 artifactDigest
    );

    function claim(ClaimData calldata data, bytes32[] calldata proof) external;
    function claimBatch(
        ClaimData[] calldata data,
        bytes32[] calldata proof,
        bool[] calldata proofFlags
    ) external;
    function claimDelegated(
        ClaimData calldata data,
        bytes32[] calldata proof,
        uint256 nonce,
        uint256 deadline,
        bytes calldata signature
    ) external;
    function setRoot(bytes32 root, bytes32 artifactDigest, uint64 version) external;
    function pause() external;
    function unpause() external;
    function claimedCount() external view returns (uint256);
    function isClaimed(uint256 leafIndex) external view returns (bool);
    function hashLeaf(ClaimData calldata data) external view returns (bytes32);
}
