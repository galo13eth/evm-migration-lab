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

abstract contract MigrationClaimBase is
    IMigrationClaim,
    Ownable2Step,
    Pausable,
    ReentrancyGuard,
    EIP712
{
    using BitMaps for BitMaps.BitMap;

    bytes32 public constant DELEGATED_CLAIM_TYPEHASH = keccak256(
        "DelegatedClaim(bytes32 leafHash,bytes32 merkleRoot,uint64 rootVersion,address destinationRecipient,uint256 nonce,uint256 deadline)"
    );

    bytes32 public immutable migrationId;
    uint256 public immutable sourceChainId;
    address public immutable sourceContract;
    uint256 public immutable snapshotBlock;
    bytes32 public immutable sourceBlockHash;
    uint256 public immutable destinationChainId;
    address public immutable destinationToken;
    uint64 public immutable claimStart;
    uint64 public immutable claimDeadline;

    bytes32 public merkleRoot;
    bytes32 public artifactDigest;
    uint64 public rootVersion;
    uint256 public claimedCount;
    mapping(address authority => uint256 nonce) public nonces;
    BitMaps.BitMap private _claimed;

    constructor(
        bytes32 migrationId_,
        uint256 sourceChainId_,
        address sourceContract_,
        uint256 snapshotBlock_,
        bytes32 sourceBlockHash_,
        uint256 destinationChainId_,
        address destinationToken_,
        uint64 claimStart_,
        uint64 claimDeadline_,
        address owner_
    ) Ownable(owner_) EIP712("EVM Migration Claim", "2") {
        if (migrationId_ == bytes32(0)) revert InvalidMigrationId();
        if (sourceContract_ == address(0)) revert InvalidAddress(sourceContract_);
        if (sourceBlockHash_ == bytes32(0)) revert InvalidSourceBlockHash();
        if (destinationChainId_ != block.chainid) {
            revert InvalidDestinationChain(destinationChainId_, block.chainid);
        }
        if (destinationToken_ == address(0)) revert InvalidAddress(destinationToken_);
        if (owner_ == address(0)) revert InvalidAddress(owner_);
        // slither-disable-next-line timestamp
        if (claimStart_ <= block.timestamp || claimStart_ >= claimDeadline_) {
            revert InvalidClaimWindow(claimStart_, claimDeadline_, block.timestamp);
        }

        migrationId = migrationId_;
        sourceChainId = sourceChainId_;
        sourceContract = sourceContract_;
        snapshotBlock = snapshotBlock_;
        sourceBlockHash = sourceBlockHash_;
        destinationChainId = destinationChainId_;
        destinationToken = destinationToken_;
        claimStart = claimStart_;
        claimDeadline = claimDeadline_;
    }

    modifier duringClaimWindow() {
        // slither-disable-next-line timestamp
        if (block.timestamp < claimStart || block.timestamp > claimDeadline) {
            revert ClaimWindowClosed(claimStart, claimDeadline, block.timestamp);
        }
        if (merkleRoot == bytes32(0)) revert ZeroRoot();
        _;
    }

    function campaignStandard() public pure returns (uint8) {
        return _campaignStandard();
    }

    function setRoot(bytes32 root, bytes32 digest, uint64 version) external onlyOwner {
        // slither-disable-next-line timestamp
        if (block.timestamp >= claimStart || claimedCount != 0) revert RootFrozen();
        if (root == bytes32(0)) revert ZeroRoot();
        if (digest == bytes32(0)) revert ZeroArtifactDigest();
        if (root == merkleRoot) revert UnchangedRoot(root);
        if (version <= rootVersion) revert InvalidVersion(rootVersion, version);

        bytes32 previousRoot = merkleRoot;
        uint64 previousVersion = rootVersion;
        merkleRoot = root;
        artifactDigest = digest;
        rootVersion = version;
        emit RootUpdated(previousRoot, root, previousVersion, version, digest);
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
        _requireAuthority(data.claimAuthority);
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
            _requireAuthority(data[i].claimAuthority);
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
        // slither-disable-next-line timestamp
        if (block.timestamp > deadline) revert ExpiredSignature(deadline, block.timestamp);
        uint256 expectedNonce = nonces[data.claimAuthority];
        if (nonce != expectedNonce) {
            revert InvalidNonce(data.claimAuthority, expectedNonce, nonce);
        }

        bytes32 leaf = _validatedLeaf(data);
        if (!MerkleProof.verifyCalldata(proof, merkleRoot, leaf)) revert InvalidProof();
        bytes32 structHash = keccak256(
            abi.encode(
                DELEGATED_CLAIM_TYPEHASH,
                leaf,
                merkleRoot,
                rootVersion,
                data.destinationRecipient,
                nonce,
                deadline
            )
        );
        if (
            !SignatureChecker.isValidSignatureNow(
                data.claimAuthority, _hashTypedDataV4(structHash), signature
            )
        ) {
            revert InvalidSignature(data.claimAuthority);
        }

        nonces[data.claimAuthority] = expectedNonce + 1;
        _completeClaim(data);
    }

    function hashLeaf(ClaimData calldata data) external view returns (bytes32) {
        return _validatedLeaf(data);
    }

    function isClaimed(uint256 leafIndex) external view returns (bool) {
        return _claimed.get(leafIndex);
    }

    function _validatedLeaf(ClaimData calldata data) internal view returns (bytes32) {
        uint8 standard = _campaignStandard();
        if (data.sourceOwner == address(0)) revert InvalidAddress(data.sourceOwner);
        if (data.claimAuthority == address(0)) revert InvalidAddress(data.claimAuthority);
        if (data.destinationRecipient == address(0)) {
            revert InvalidAddress(data.destinationRecipient);
        }
        if (standard == 1 && data.amount != 1) revert InvalidAmount(standard, data.amount);
        if (standard == 2 && data.amount == 0) revert InvalidAmount(standard, data.amount);

        return keccak256(
            bytes.concat(
                keccak256(
                    abi.encode(
                        migrationId,
                        sourceChainId,
                        sourceContract,
                        snapshotBlock,
                        sourceBlockHash,
                        destinationChainId,
                        standard,
                        data.tokenId,
                        data.amount,
                        data.sourceOwner,
                        data.claimAuthority,
                        data.destinationRecipient,
                        data.leafIndex
                    )
                )
            )
        );
    }

    function _requireAuthority(address authority) private view {
        if (authority != msg.sender) {
            revert UnauthorizedClaimAuthority(authority, msg.sender);
        }
    }

    function _completeClaim(ClaimData calldata data) private {
        if (_claimed.get(data.leafIndex)) revert AlreadyClaimed(data.leafIndex);
        _claimed.set(data.leafIndex);
        // slither-disable-next-line costly-loop
        ++claimedCount;

        _mint(data.destinationRecipient, data.tokenId, data.amount);
        emit Claimed(
            rootVersion,
            data.leafIndex,
            data.sourceOwner,
            data.claimAuthority,
            data.destinationRecipient,
            _campaignStandard(),
            data.tokenId,
            data.amount
        );
    }

    function _campaignStandard() internal pure virtual returns (uint8);
    function _mint(address recipient, uint256 tokenId, uint256 amount) internal virtual;
}
