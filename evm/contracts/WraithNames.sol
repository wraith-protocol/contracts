// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

import {ECDSA} from "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";

/// @title WraithNames
/// @notice On-chain name registry for stealth meta-addresses. Maps human-readable names
///         directly to meta-addresses with no wallet address stored. Ownership is proven
///         via the spending public key embedded in the meta-address — the first 33 bytes.
/// @dev Names are lowercase alphanumeric, 3-32 characters. No expiry. Owner can update
///      the meta-address or release the name. Ownership is verified by signing with the
///      spending private key (the key corresponding to the first 33 bytes of the meta-address).
contract WraithNames {
    using ECDSA for bytes32;

    /// @notice Emitted when a name is registered or updated.
    /// @param nameHash The keccak256 hash of the name.
    /// @param name The human-readable name.
    /// @param stealthMetaAddress The stealth meta-address mapped to this name.
    event NameRegistered(
        bytes32 indexed nameHash,
        string name,
        bytes stealthMetaAddress
    );

    /// @notice Emitted when a name is released by its owner.
    /// @param nameHash The keccak256 hash of the name.
    /// @param name The human-readable name.
    event NameReleased(bytes32 indexed nameHash, string name);

    error NameTaken();
    error NameTooShort();
    error NameTooLong();
    error InvalidNameCharacter();
    error InvalidMetaAddress();
    error InvalidSignature();
    error NameNotFound();
    error NotOwner();

    struct NameEntry {
        string name;
        bytes stealthMetaAddress;
        address spendingAddress; // derived from spending public key for ownership verification
    }

    /// @notice name hash → entry
    mapping(bytes32 => NameEntry) private _names;

    /// @notice meta-address hash → name hash (reverse lookup)
    mapping(bytes32 => bytes32) private _reverse;

    /// @notice Nonces for replay protection on registerOnBehalf
    mapping(address => uint256) public nonces;

    bytes32 public constant REGISTER_TYPEHASH =
        keccak256("Register(string name,bytes stealthMetaAddress,uint256 nonce)");

    /// @notice Register a name mapped to a stealth meta-address.
    ///         The caller must sign the registration with the spending private key
    ///         (corresponding to the first 33 bytes of the meta-address).
    /// @param name The human-readable name (lowercase alphanumeric, 3-32 chars).
    /// @param stealthMetaAddress The 66-byte stealth meta-address (spending pubkey + viewing pubkey).
    /// @param signature Signature from the spending private key over keccak256(name, stealthMetaAddress).
    function register(
        string calldata name,
        bytes calldata stealthMetaAddress,
        bytes calldata signature
    ) external {
        _validateName(name);
        _validateMetaAddress(stealthMetaAddress);

        bytes32 nameHash = keccak256(bytes(name));
        if (_names[nameHash].spendingAddress != address(0)) revert NameTaken();

        address spendingAddr = _deriveSpendingAddress(stealthMetaAddress);
        bytes32 digest = keccak256(abi.encodePacked(name, stealthMetaAddress));
        bytes32 ethSignedHash = _toEthSignedMessageHash(digest);
        address recovered = ethSignedHash.recover(signature);
        if (recovered != spendingAddr) revert InvalidSignature();

        _names[nameHash] = NameEntry(name, stealthMetaAddress, spendingAddr);
        _reverse[keccak256(stealthMetaAddress)] = nameHash;

        emit NameRegistered(nameHash, name, stealthMetaAddress);
    }

    /// @notice Register a name on behalf of someone else. The relayer submits the tx
    ///         and pays gas. The spending key holder signs the registration off-chain.
    /// @param name The human-readable name.
    /// @param stealthMetaAddress The 66-byte stealth meta-address.
    /// @param signature Signature from the spending private key over (name, stealthMetaAddress, nonce).
    function registerOnBehalf(
        string calldata name,
        bytes calldata stealthMetaAddress,
        bytes calldata signature
    ) external {
        _validateName(name);
        _validateMetaAddress(stealthMetaAddress);

        bytes32 nameHash = keccak256(bytes(name));
        if (_names[nameHash].spendingAddress != address(0)) revert NameTaken();

        address spendingAddr = _deriveSpendingAddress(stealthMetaAddress);
        uint256 nonce = nonces[spendingAddr];

        bytes32 digest = keccak256(abi.encodePacked(name, stealthMetaAddress, nonce));
        bytes32 ethSignedHash = _toEthSignedMessageHash(digest);
        address recovered = ethSignedHash.recover(signature);
        if (recovered != spendingAddr) revert InvalidSignature();

        nonces[spendingAddr] = nonce + 1;

        _names[nameHash] = NameEntry(name, stealthMetaAddress, spendingAddr);
        _reverse[keccak256(stealthMetaAddress)] = nameHash;

        emit NameRegistered(nameHash, name, stealthMetaAddress);
    }

    /// @notice Update the meta-address for an existing name. Must be signed by the
    ///         current spending key owner.
    /// @param name The name to update.
    /// @param newMetaAddress The new 66-byte stealth meta-address.
    /// @param signature Signature from the current spending private key over (name, newMetaAddress).
    function update(
        string calldata name,
        bytes calldata newMetaAddress,
        bytes calldata signature
    ) external {
        _validateMetaAddress(newMetaAddress);

        bytes32 nameHash = keccak256(bytes(name));
        NameEntry storage entry = _names[nameHash];
        if (entry.spendingAddress == address(0)) revert NameNotFound();

        bytes32 digest = keccak256(abi.encodePacked(name, newMetaAddress));
        bytes32 ethSignedHash = _toEthSignedMessageHash(digest);
        address recovered = ethSignedHash.recover(signature);
        if (recovered != entry.spendingAddress) revert NotOwner();

        // Remove old reverse lookup
        delete _reverse[keccak256(entry.stealthMetaAddress)];

        // Update
        address newSpendingAddr = _deriveSpendingAddress(newMetaAddress);
        entry.stealthMetaAddress = newMetaAddress;
        entry.spendingAddress = newSpendingAddr;
        _reverse[keccak256(newMetaAddress)] = nameHash;

        emit NameRegistered(nameHash, name, newMetaAddress);
    }

    /// @notice Release a name, making it available for registration again.
    ///         Must be signed by the spending key owner.
    /// @param name The name to release.
    /// @param signature Signature from the spending private key over the name.
    function release(
        string calldata name,
        bytes calldata signature
    ) external {
        bytes32 nameHash = keccak256(bytes(name));
        NameEntry storage entry = _names[nameHash];
        if (entry.spendingAddress == address(0)) revert NameNotFound();

        bytes32 digest = keccak256(abi.encodePacked(name));
        bytes32 ethSignedHash = _toEthSignedMessageHash(digest);
        address recovered = ethSignedHash.recover(signature);
        if (recovered != entry.spendingAddress) revert NotOwner();

        delete _reverse[keccak256(entry.stealthMetaAddress)];
        delete _names[nameHash];

        emit NameReleased(nameHash, name);
    }

    /// @notice Resolve a name to its stealth meta-address.
    /// @param name The name to look up.
    /// @return The stealth meta-address, or empty bytes if not registered.
    function resolve(string calldata name) external view returns (bytes memory) {
        return _names[keccak256(bytes(name))].stealthMetaAddress;
    }

    /// @notice Reverse lookup: find the name for a given stealth meta-address.
    /// @param stealthMetaAddress The meta-address to look up.
    /// @return The name, or empty string if not registered.
    function nameOf(bytes calldata stealthMetaAddress) external view returns (string memory) {
        bytes32 nameHash = _reverse[keccak256(stealthMetaAddress)];
        if (nameHash == bytes32(0)) return "";
        return _names[nameHash].name;
    }

    /// @dev Derive an Ethereum address from the spending public key (first 33 bytes of meta-address).
    ///      Decompresses the secp256k1 point and takes keccak256 of the uncompressed x,y coordinates.
    function _deriveSpendingAddress(bytes calldata metaAddress) private view returns (address) {
        bytes memory compressed = metaAddress[:33];
        uint8 prefix = uint8(compressed[0]);
        if (prefix != 0x02 && prefix != 0x03) revert InvalidMetaAddress();

        // Extract x coordinate
        uint256 x;
        assembly {
            x := mload(add(compressed, 33))
        }

        // secp256k1 curve parameters
        uint256 p = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFC2F;

        // y^2 = x^3 + 7 mod p
        uint256 y2 = addmod(mulmod(mulmod(x, x, p), x, p), 7, p);
        uint256 y = _modExp(y2, (p + 1) / 4, p);

        // Choose correct y parity
        if ((y % 2 == 0) != (prefix == 0x02)) {
            y = p - y;
        }

        // Address = last 20 bytes of keccak256(x || y)
        return address(uint160(uint256(keccak256(abi.encodePacked(x, y)))));
    }

    /// @dev Modular exponentiation using the EVM precompile at address 0x05.
    function _modExp(uint256 b, uint256 e, uint256 m) private view returns (uint256 result) {
        assembly {
            let ptr := mload(0x40)
            mstore(ptr, 32)
            mstore(add(ptr, 32), 32)
            mstore(add(ptr, 64), 32)
            mstore(add(ptr, 96), b)
            mstore(add(ptr, 128), e)
            mstore(add(ptr, 160), m)

            if iszero(staticcall(gas(), 0x05, ptr, 192, ptr, 32)) {
                revert(0, 0)
            }

            result := mload(ptr)
        }
    }

    /// @dev Validates name: 3-32 chars, lowercase alphanumeric only.
    function _validateName(string calldata name) private pure {
        bytes memory b = bytes(name);
        if (b.length < 3) revert NameTooShort();
        if (b.length > 32) revert NameTooLong();

        for (uint256 i; i < b.length; ) {
            uint8 c = uint8(b[i]);
            bool isLower = c >= 0x61 && c <= 0x7a; // a-z
            bool isDigit = c >= 0x30 && c <= 0x39; // 0-9
            if (!isLower && !isDigit) revert InvalidNameCharacter();
            unchecked { ++i; }
        }
    }

    /// @dev Validates meta-address is exactly 66 bytes (two 33-byte compressed keys).
    function _validateMetaAddress(bytes calldata metaAddress) private pure {
        if (metaAddress.length != 66) revert InvalidMetaAddress();
    }

    /// @dev Prepends Ethereum signed message prefix.
    function _toEthSignedMessageHash(bytes32 hash) private pure returns (bytes32) {
        return keccak256(abi.encodePacked("\x19Ethereum Signed Message:\n32", hash));
    }
}
