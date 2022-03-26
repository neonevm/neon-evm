// SPDX-License-Identifier: MIT

pragma solidity >=0.7.0;

/**
 * @title QueryAccount
 * @dev Wrappers around QueryAccount operations.
 */
library QueryAccount {
    address constant precompiled = 0xff00000000000000000000000000000000000002;

    /**
     * @dev Puts the metadata and a chunk of data into the cache.
     * @param solana_address Address of an account.
     * @param offset Offset in bytes from the beginning of the data.
     * @param len Length in bytes of the chunk.
     */
    function cache(uint256 solana_address, uint64 offset, uint64 len) internal returns (bool) {
        (bool success, bytes memory _dummy) = precompiled.staticcall(abi.encodeWithSignature("cache(uint256,uint64,uint64)", solana_address, offset, len));
        return success;
    }

    /**
     * @dev Returns the account's owner Solana address.
     * @param solana_address Address of an account.
     */
    function owner(uint256 solana_address) internal view returns (bool, uint256) {
        (bool success, bytes memory result) = precompiled.staticcall(abi.encodeWithSignature("owner(uint256)", solana_address));
        return (success, to_uint256(result));
    }

    /**
     * @dev Returns full length of the account's data.
     * @param solana_address Address of an account.
     */
    function length(uint256 solana_address) internal view returns (bool, uint256) {
        (bool success, bytes memory result) = precompiled.staticcall(abi.encodeWithSignature("length(uint256)", solana_address));
        return (success, to_uint256(result));
    }

    /**
     * @dev Returns the funds in lamports of the account.
     * @param solana_address Address of an account.
     */
    function lamports(uint256 solana_address) internal view returns (bool, uint256) {
        (bool success, bytes memory result) = precompiled.staticcall(abi.encodeWithSignature("lamports(uint256)", solana_address));
        return (success, to_uint256(result));
    }

    /**
     * @dev Returns the executable flag of the account.
     * @param solana_address Address of an account.
     */
    function executable(uint256 solana_address) internal view returns (bool, bool) {
        (bool success, bytes memory result) = precompiled.staticcall(abi.encodeWithSignature("executable(uint256)", solana_address));
        return (success, to_bool(result));
    }

    /**
     * @dev Returns the rent epoch of the account.
     * @param solana_address Address of an account.
     */
    function rent_epoch(uint256 solana_address) internal view returns (bool, uint256) {
        (bool success, bytes memory result) = precompiled.staticcall(abi.encodeWithSignature("rent_epoch(uint256)", solana_address));
        return (success, to_uint256(result));
    }

    /**
     * @dev Returns a chunk of the data.
     * @param solana_address Address of an account.
     * @param offset Offset in bytes from the beginning of the cached segment of data.
     * @param len Length in bytes of the returning chunk.
     */
    function data(uint256 solana_address, uint64 offset, uint64 len) internal view returns (bool, bytes memory) {
        return precompiled.staticcall(abi.encodeWithSignature("data(uint256,uint64,uint64)", solana_address, offset, len));
    }

    function to_uint256(bytes memory bb) private pure returns (uint256 result) {
        assembly {
            result := mload(add(bb, 32))
        }
    }

    function to_bool(bytes memory bb) private pure returns (bool result) {
        assembly {
            result := mload(add(bb, 32))
        }
    }
}
