// SPDX-License-Identifier: MIT

pragma solidity >=0.8.0;

interface IQueryAccount {
    function metadata(bytes32 solana_address) external view returns (bytes1[]);
    function data(bytes32 solana_address, string key) external view returns (bytes1[]);
}

contract QueryAccount {
    address constant NeonQueryAccount = 0xff00000000000000000000000000000000000002;

    fallback() external {
        bytes memory call_data = abi.encodePacked(msg.data);
        (bool success, bytes memory result) = NeonQueryAccount.delegatecall(call_data);

        require(success, string(result));

        assembly {
            return(add(result, 0x20), mload(result))
        }
    }
}
