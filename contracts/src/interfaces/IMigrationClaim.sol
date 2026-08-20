// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

interface IMigrationClaim {
    struct ClaimData {
        uint8 standard;
        uint256 tokenId;
        uint256 amount;
        address sourceOwner;
        address recipient;
        uint256 leafIndex;
    }

    error AlreadyClaimed(uint64 version, uint256 leafIndex);
    error ClaimWindowClosed(uint64 start, uint64 deadline, uint256 timestamp);
    error EmptyBatch();
    error ExpiredSignature(uint256 deadline, uint256 timestamp);
    error InvalidAddress(address value);
    error InvalidAmount(uint8 standard, uint256 amount);
    error InvalidMigrationId();
    error InvalidNonce(address owner, uint256 expected, uint256 actual);
    error InvalidProof();
    error InvalidSignature(address signer);
    error InvalidTokenStandard(uint8 standard);
    error InvalidVersion(uint64 current, uint64 proposed);
    error UnauthorizedSourceOwner(address expected, address caller);
    error ZeroRoot();

    event Claimed(
        uint64 indexed version,
        uint256 indexed leafIndex,
        address indexed sourceOwner,
        address recipient,
        uint8 standard,
        uint256 tokenId,
        uint256 amount
    );
    event RootUpdated(
        bytes32 indexed previousRoot,
        bytes32 indexed newRoot,
        uint64 previousVersion,
        uint64 newVersion
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

    function setRoot(bytes32 root, uint64 version) external;
    function pause() external;
    function unpause() external;
    function claimedCount() external view returns (uint256);
    function isClaimed(uint64 version, uint256 leafIndex) external view returns (bool);
    function hashLeaf(ClaimData calldata data) external view returns (bytes32);
}
