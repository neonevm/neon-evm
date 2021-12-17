// SPDX-License-Identifier: MIT
pragma solidity >=0.7.0;

contract QueryAccount {
    address constant precompiled = 0xff00000000000000000000000000000000000002;

    // Takes a Solana address, treats it as an address of an account.
    // Puts the metadata and a chunk of data into the cache.
    function cache(uint256 solana_address, uint64 offset, uint64 len) public {
        (bool success, bytes memory _dummy) = precompiled.staticcall(abi.encodeWithSignature("cache(uint256,uint64,uint64)", solana_address, offset, len));
        require(success, "QueryAccount.cache failed");
    }

    // Takes a Solana address, treats it as an address of an account.
    // Returns the account's owner Solana address (32 bytes).
    function owner(uint256 solana_address) public view returns (uint256) {
        (bool success, bytes memory result) = precompiled.staticcall(abi.encodeWithSignature("owner(uint256)", solana_address));
        require(success, "QueryAccount.owner failed");
        return to_uint256(result);
    }

    // Takes a Solana address, treats it as an address of an account.
    // Returns the length of the account's data (8 bytes).
    function length(uint256 solana_address) public view returns (uint256) {
        (bool success, bytes memory result) = precompiled.staticcall(abi.encodeWithSignature("length(uint256)", solana_address));
        require(success, "QueryAccount.length failed");
        return to_uint256(result);
    }

    // Takes a Solana address, treats it as an address of an account.
    // Returns the funds in lamports of the account.
    function lamports(uint256 solana_address) public view returns (uint256) {
        (bool success, bytes memory result) = precompiled.staticcall(abi.encodeWithSignature("lamports(uint256)", solana_address));
        require(success, "QueryAccount.lamports failed");
        return to_uint256(result);
    }

    // Takes a Solana address, treats it as an address of an account.
    // Returns the executable flag of the account.
    function executable(uint256 solana_address) public view returns (bool) {
        (bool success, bytes memory result) = precompiled.staticcall(abi.encodeWithSignature("executable(uint256)", solana_address));
        require(success, "QueryAccount.executable failed");
        return to_bool(result);
    }

    // Takes a Solana address, treats it as an address of an account.
    // Returns the rent epoch of the account.
    function rent_epoch(uint256 solana_address) public view returns (uint256) {
        (bool success, bytes memory result) = precompiled.staticcall(abi.encodeWithSignature("rent_epoch(uint256)", solana_address));
        require(success, "QueryAccount.rent_epoch failed");
        return to_uint256(result);
    }

    // Takes a Solana address, treats it as an address of an account,
    // also takes an offset and length of the account's data.
    // Returns a chunk of the data (length bytes).
    function data(uint256 solana_address, uint64 offset, uint64 len) public view returns (bytes memory) {
        (bool success, bytes memory result) = precompiled.staticcall(abi.encodeWithSignature("data(uint256,uint64,uint64)", solana_address, offset, len));
        require(success, "QueryAccount.data failed");
        return result;
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
