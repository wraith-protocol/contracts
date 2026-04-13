// SPDX-License-Identifier: MIT
pragma solidity ^0.8.28;

/// @title IERC6538Registry
/// @notice Interface for the ERC-6538 Stealth Meta-Address Registry contract.
interface IERC6538Registry {
    /// @notice Emitted when a stealth meta-address is registered or updated.
    /// @param registrant The address of the registrant.
    /// @param schemeId The scheme ID for the stealth address scheme.
    /// @param stealthMetaAddress The registered stealth meta-address.
    event StealthMetaAddressSet(
        address indexed registrant,
        uint256 indexed schemeId,
        bytes stealthMetaAddress
    );

    /// @notice Emitted when a registrant increments their nonce.
    /// @param registrant The address of the registrant.
    /// @param newNonce The new nonce value.
    event NonceIncremented(
        address indexed registrant,
        uint256 newNonce
    );

    /// @notice Thrown when a signature provided to registerKeysOnBehalf is invalid.
    error ERC6538Registry__InvalidSignature();

    /// @notice Registers the caller's stealth meta-address for the given scheme.
    /// @param schemeId The scheme ID (e.g. 1 for secp256k1).
    /// @param stealthMetaAddress The stealth meta-address to register.
    function registerKeys(uint256 schemeId, bytes calldata stealthMetaAddress) external;

    /// @notice Registers a stealth meta-address on behalf of another address using an EIP-712 signature.
    /// @param registrant The address to register for.
    /// @param schemeId The scheme ID.
    /// @param signature The EIP-712 signature from the registrant.
    /// @param stealthMetaAddress The stealth meta-address to register.
    function registerKeysOnBehalf(
        address registrant,
        uint256 schemeId,
        bytes memory signature,
        bytes calldata stealthMetaAddress
    ) external;

    /// @notice Increments the caller's nonce, invalidating all outstanding signatures.
    function incrementNonce() external;

    /// @notice Returns the stealth meta-address for the given registrant and scheme.
    /// @param registrant The registrant address.
    /// @param schemeId The scheme ID.
    /// @return The registered stealth meta-address, or empty bytes if none.
    function stealthMetaAddressOf(
        address registrant,
        uint256 schemeId
    ) external view returns (bytes memory);

    /// @notice Returns the current nonce for the given registrant.
    /// @param registrant The registrant address.
    /// @return The current nonce.
    function nonceOf(address registrant) external view returns (uint256);

    /// @notice Returns the EIP-712 domain separator.
    /// @return The domain separator bytes32 value.
    function DOMAIN_SEPARATOR() external view returns (bytes32);
}
