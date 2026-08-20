// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import { Ownable } from "@openzeppelin/contracts/access/Ownable.sol";
import { Ownable2Step } from "@openzeppelin/contracts/access/Ownable2Step.sol";
import { EIP712 } from "@openzeppelin/contracts/utils/cryptography/EIP712.sol";
import { MerkleProof } from "@openzeppelin/contracts/utils/cryptography/MerkleProof.sol";
import { SignatureChecker } from "@openzeppelin/contracts/utils/cryptography/SignatureChecker.sol";
import { Pausable } from "@openzeppelin/contracts/utils/Pausable.sol";
import { ReentrancyGuard } from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import { BitMaps } from "@openzeppelin/contracts/utils/structs/BitMaps.sol";

import { IMigrationClaim } from "./interfaces/IMigrationClaim.sol";

interface IMigratedERC721 {
    function mint(address to, uint256 tokenId) external;
}

interface IMigratedERC1155 {
    function mint(address to, uint256 tokenId, uint256 amount) external;
}

contract MigrationClaim is IMigrationClaim, Ownable2Step, Pausable, ReentrancyGuard, EIP712 {
    using BitMaps for BitMaps.BitMap;

    uint8 public constant ERC721_STANDARD = 1;
    uint8 public constant ERC1155_STANDARD = 2;

    bytes32 public constant DELEGATED_CLAIM_TYPEHASH = keccak256(
        "DelegatedClaim(bytes32 leafHash,address recipient,uint256 nonce,uint256 deadline)"
    );

    bytes32 public immutable migrationId;
    uint256 public immutable sourceChainId;
    address public immutable sourceContract;
    uint256 public immutable snapshotBlock;
    IMigratedERC721 public immutable migratedERC721;
    IMigratedERC1155 public immutable migratedERC1155;
    uint64 public immutable claimStart;
    uint64 public immutable claimDeadline;

    bytes32 public merkleRoot;
    uint64 public rootVersion;
    mapping(address owner => uint256 nonce) public nonces;
    mapping(uint64 version => BitMaps.BitMap bitmap) private _claimed;
    mapping(uint64 version => uint256 count) private _claimedCounts;

    constructor(
        bytes32 migrationId_,
        uint256 sourceChainId_,
        address sourceContract_,
        uint256 snapshotBlock_,
        address migratedERC721_,
        address migratedERC1155_,
        uint64 claimStart_,
        uint64 claimDeadline_,
        address owner_
    ) Ownable(owner_) EIP712("EVM Migration Claim", "1") {
        if (migrationId_ == bytes32(0)) revert InvalidMigrationId();
        if (sourceContract_ == address(0)) revert InvalidAddress(sourceContract_);
        if (migratedERC721_ == address(0)) revert InvalidAddress(migratedERC721_);
        if (migratedERC1155_ == address(0)) revert InvalidAddress(migratedERC1155_);
        if (claimStart_ >= claimDeadline_) {
            revert ClaimWindowClosed(claimStart_, claimDeadline_, block.timestamp);
        }
        migrationId = migrationId_;
        sourceChainId = sourceChainId_;
        sourceContract = sourceContract_;
        snapshotBlock = snapshotBlock_;
        migratedERC721 = IMigratedERC721(migratedERC721_);
        migratedERC1155 = IMigratedERC1155(migratedERC1155_);
        claimStart = claimStart_;
        claimDeadline = claimDeadline_;
    }

    modifier duringClaimWindow() {
        if (block.timestamp < claimStart || block.timestamp > claimDeadline) {
            revert ClaimWindowClosed(claimStart, claimDeadline, block.timestamp);
        }
        if (merkleRoot == bytes32(0)) revert ZeroRoot();
        _;
    }

    function setRoot(bytes32 root, uint64 version) external onlyOwner {
        if (root == bytes32(0)) revert ZeroRoot();
        if (version <= rootVersion) revert InvalidVersion(rootVersion, version);
        bytes32 previousRoot = merkleRoot;
        uint64 previousVersion = rootVersion;
        merkleRoot = root;
        rootVersion = version;
        emit RootUpdated(previousRoot, root, previousVersion, version);
    }

    function pause() external onlyOwner {
        _pause();
    }

    function unpause() external onlyOwner {
        _unpause();
    }

    function claim(ClaimData calldata data, bytes32[] calldata proof)
        external
        nonReentrant
        whenNotPaused
        duringClaimWindow
    {
        _requireOwner(data.sourceOwner);
        bytes32 leaf = _validatedLeaf(data);
        if (!MerkleProof.verifyCalldata(proof, merkleRoot, leaf)) revert InvalidProof();
        _completeClaim(data);
    }

    function claimBatch(
        ClaimData[] calldata data,
        bytes32[] calldata proof,
        bool[] calldata proofFlags
    ) external nonReentrant whenNotPaused duringClaimWindow {
        uint256 length = data.length;
        if (length == 0) revert EmptyBatch();

        bytes32[] memory leaves = new bytes32[](length);
        for (uint256 i; i < length; ++i) {
            _requireOwner(data[i].sourceOwner);
            leaves[i] = _validatedLeaf(data[i]);
        }
        if (!MerkleProof.multiProofVerifyCalldata(proof, proofFlags, merkleRoot, leaves)) {
            revert InvalidProof();
        }
        for (uint256 i; i < length; ++i) {
            _completeClaim(data[i]);
        }
    }

    function claimDelegated(
        ClaimData calldata data,
        bytes32[] calldata proof,
        uint256 nonce,
        uint256 deadline,
        bytes calldata signature
    ) external nonReentrant whenNotPaused duringClaimWindow {
        if (block.timestamp > deadline) revert ExpiredSignature(deadline, block.timestamp);
        uint256 expectedNonce = nonces[data.sourceOwner];
        if (nonce != expectedNonce) revert InvalidNonce(data.sourceOwner, expectedNonce, nonce);

        bytes32 leaf = _validatedLeaf(data);
        if (!MerkleProof.verifyCalldata(proof, merkleRoot, leaf)) revert InvalidProof();
        bytes32 structHash =
            keccak256(abi.encode(DELEGATED_CLAIM_TYPEHASH, leaf, data.recipient, nonce, deadline));
        if (
            !SignatureChecker.isValidSignatureNow(
                data.sourceOwner, _hashTypedDataV4(structHash), signature
            )
        ) {
            revert InvalidSignature(data.sourceOwner);
        }

        nonces[data.sourceOwner] = expectedNonce + 1;
        _completeClaim(data);
    }

    function hashLeaf(ClaimData calldata data) external view returns (bytes32) {
        return _validatedLeaf(data);
    }

    function claimedCount() external view returns (uint256) {
        return _claimedCounts[rootVersion];
    }

    function isClaimed(uint64 version, uint256 leafIndex) external view returns (bool) {
        return _claimed[version].get(leafIndex);
    }

    function _requireOwner(address sourceOwner) private view {
        if (sourceOwner != msg.sender) revert UnauthorizedSourceOwner(sourceOwner, msg.sender);
    }

    function _validatedLeaf(ClaimData calldata data) private view returns (bytes32) {
        if (data.standard == ERC721_STANDARD) {
            if (data.amount != 1) revert InvalidAmount(data.standard, data.amount);
        } else if (data.standard == ERC1155_STANDARD) {
            if (data.amount == 0) revert InvalidAmount(data.standard, data.amount);
        } else {
            revert InvalidTokenStandard(data.standard);
        }

        return keccak256(
            bytes.concat(
                keccak256(
                    abi.encode(
                        migrationId,
                        sourceChainId,
                        sourceContract,
                        snapshotBlock,
                        data.standard,
                        data.tokenId,
                        data.amount,
                        data.sourceOwner,
                        data.recipient,
                        data.leafIndex
                    )
                )
            )
        );
    }

    function _completeClaim(ClaimData calldata data) private {
        BitMaps.BitMap storage bitmap = _claimed[rootVersion];
        if (bitmap.get(data.leafIndex)) revert AlreadyClaimed(rootVersion, data.leafIndex);
        bitmap.set(data.leafIndex);
        ++_claimedCounts[rootVersion];

        if (data.standard == ERC721_STANDARD) {
            migratedERC721.mint(data.recipient, data.tokenId);
        } else {
            migratedERC1155.mint(data.recipient, data.tokenId, data.amount);
        }
        emit Claimed(
            rootVersion,
            data.leafIndex,
            data.sourceOwner,
            data.recipient,
            data.standard,
            data.tokenId,
            data.amount
        );
    }
}
