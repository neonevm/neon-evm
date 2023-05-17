// SPDX-License-Identifier: MIT

pragma solidity >= 0.7.0;
pragma abicoder v2;

/**
 * @title QueryAccount
 * @dev Wrappers around QueryAccount operations.
 */
interface QueryAccount {
    struct AccountInfo {
        bytes32 pubkey;
        uint64 lamports;
        bytes32 owner;
        bool executable;
        uint64 rent_epoch;
    }

    /**
     * @dev Returns the account's owner Solana address.
     * @param solana_address Address of an account.
     */
    function owner(bytes32 solana_address) external view returns (uint256);

    /**
     * @dev Returns the funds in lamports of the account.
     * @param solana_address Address of an account.
     */
    function lamports(bytes32 solana_address) external view returns (uint256);

    /**
     * @dev Returns the executable flag of the account.
     * @param solana_address Address of an account.
     */
    function executable(bytes32 solana_address) external view returns (bool);

    /**
     * @dev Returns the rent epoch of the account.
     * @param solana_address Address of an account.
     */
    function rent_epoch(bytes32 solana_address) external view returns (uint256);

    /**
     * @dev Returns full length of the account's data.
     * @param solana_address Address of an account.
     */
    function info(bytes32 solana_address) external view returns (AccountInfo memory);

    /**
     * @dev Returns full length of the account's data.
     * @param solana_address Address of an account.
     */
    function length(bytes32 solana_address) external view returns (uint256);

    /**
     * @dev Returns a chunk of the data.
     * @param solana_address Address of an account.
     * @param offset Offset in bytes from the beginning of the cached segment of data.
     * @param len Length in bytes of the returning chunk.
     */
    function data(bytes32 solana_address, uint64 offset, uint64 len) external view returns (bytes memory);
}
