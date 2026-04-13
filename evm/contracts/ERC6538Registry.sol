// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

import {IERC6538Registry} from "./interfaces/IERC6538Registry.sol";
import {ECDSA} from "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";
import {IERC1271} from "@openzeppelin/contracts/interfaces/IERC1271.sol";

/// @title ERC6538Registry
/// @notice Singleton registry for stealth meta-addresses per ERC-6538.
/// @dev Supports direct registration and delegated registration via EIP-712 signatures.
///      Signature verification supports both EOA (ecrecover) and EIP-1271 (smart contract wallets).
contract ERC6538Registry is IERC6538Registry {
    using ECDSA for bytes32;

    /// @notice EIP-712 type hash for the registration entry.
    bytes32 public constant ERC6538REGISTRY_ENTRY_TYPE_HASH =
        keccak256("Erc6538RegistryEntry(uint256 schemeId,bytes stealthMetaAddress,uint256 nonce)");

    /// @notice The chain ID at deployment, used to detect forks.
    uint256 private immutable _INITIAL_CHAIN_ID;

    /// @notice The domain separator computed at deployment.
    bytes32 private immutable _INITIAL_DOMAIN_SEPARATOR;

    /// @notice Mapping: registrant => schemeId => stealth meta-address.
    mapping(address => mapping(uint256 => bytes)) private _stealthMetaAddresses;

    /// @notice Mapping: registrant => nonce (for replay protection).
    mapping(address => uint256) private _nonces;

    constructor() {
        _INITIAL_CHAIN_ID = block.chainid;
        _INITIAL_DOMAIN_SEPARATOR = _computeDomainSeparator();
    }

    /// @inheritdoc IERC6538Registry
    function registerKeys(uint256 schemeId, bytes calldata stealthMetaAddress) external {
        _stealthMetaAddresses[msg.sender][schemeId] = stealthMetaAddress;
        emit StealthMetaAddressSet(msg.sender, schemeId, stealthMetaAddress);
    }

    /// @inheritdoc IERC6538Registry
    function registerKeysOnBehalf(
        address registrant,
        uint256 schemeId,
        bytes memory signature,
        bytes calldata stealthMetaAddress
    ) external {
        // Build the EIP-712 digest
        bytes32 digest = _hashTypedData(
            keccak256(
                abi.encode(
                    ERC6538REGISTRY_ENTRY_TYPE_HASH,
                    schemeId,
                    keccak256(stealthMetaAddress),
                    _nonces[registrant]
                )
            )
        );

        // Verify the signature — supports both EOA and EIP-1271
        _verifySignature(registrant, digest, signature);

        // Increment nonce to prevent replay
        unchecked {
            _nonces[registrant]++;
        }

        // Store and emit
        _stealthMetaAddresses[registrant][schemeId] = stealthMetaAddress;
        emit StealthMetaAddressSet(registrant, schemeId, stealthMetaAddress);
    }

    /// @inheritdoc IERC6538Registry
    function incrementNonce() external {
        uint256 newNonce;
        unchecked {
            newNonce = ++_nonces[msg.sender];
        }
        emit NonceIncremented(msg.sender, newNonce);
    }

    /// @inheritdoc IERC6538Registry
    function stealthMetaAddressOf(
        address registrant,
        uint256 schemeId
    ) external view returns (bytes memory) {
        return _stealthMetaAddresses[registrant][schemeId];
    }

    /// @inheritdoc IERC6538Registry
    function nonceOf(address registrant) external view returns (uint256) {
        return _nonces[registrant];
    }

    /// @inheritdoc IERC6538Registry
    function DOMAIN_SEPARATOR() public view returns (bytes32) {
        return block.chainid == _INITIAL_CHAIN_ID
            ? _INITIAL_DOMAIN_SEPARATOR
            : _computeDomainSeparator();
    }

    /// @dev Computes the EIP-712 domain separator.
    function _computeDomainSeparator() private view returns (bytes32) {
        return keccak256(
            abi.encode(
                keccak256("EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"),
                keccak256("ERC6538Registry"),
                keccak256("1"),
                block.chainid,
                address(this)
            )
        );
    }

    /// @dev Hashes typed data according to EIP-712.
    function _hashTypedData(bytes32 structHash) private view returns (bytes32) {
        return keccak256(abi.encodePacked("\x19\x01", DOMAIN_SEPARATOR(), structHash));
    }

    /// @dev Verifies a signature against a digest for the given signer.
    ///      Supports EOA signatures (ecrecover) and EIP-1271 smart contract wallets.
    function _verifySignature(
        address signer,
        bytes32 digest,
        bytes memory signature
    ) private view {
        // Try ecrecover first
        (address recovered, ECDSA.RecoverError err, ) = ECDSA.tryRecover(digest, signature);

        if (err == ECDSA.RecoverError.NoError && recovered == signer) {
            return;
        }

        // Fall back to EIP-1271 for smart contract wallets
        if (signer.code.length > 0) {
            try IERC1271(signer).isValidSignature(digest, signature) returns (bytes4 magicValue) {
                if (magicValue == IERC1271.isValidSignature.selector) {
                    return;
                }
            } catch {} // solhint-disable-line no-empty-blocks
        }

        revert ERC6538Registry__InvalidSignature();
    }
}
