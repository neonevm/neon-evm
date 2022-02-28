// SPDX-License-Identifier: MIT
pragma solidity >=0.5.12;

contract BlockHashTest {
    function getCurrentValues() public view returns (bytes32) {
        uint blockNumber = block.number;
        bytes32 blockHashNow = block.blockhash(blockNumber);
        return blockHashNow;
    }

    function getValues(uint number) public view returns (bytes32) {
        bytes32 blockHash = block.blockhash(number);
        return blockHash;
    }
}
