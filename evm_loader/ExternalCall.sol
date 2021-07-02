// SPDX-License-Identifier: MIT

pragma solidity >=0.7.0 <0.8.0;

contract ExternalCall {

    uint256 private constant token_id = 0x06ddf6e1d765a193d9cbe146ceeb79ac1cb485ed5f5b37913a8cf5857eff00a9; // "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
    address private constant solana = 0xfF00000000000000000000000000000000000000;

    function transferExt(uint256 token, uint256 from, uint256 to, uint amount, uint256 signer, uint256 owner) public {
        uint64 val = reverse(amount);
        bytes memory instruction_data = abi.encodePacked(
            uint8(0),    // external call
            token_id,    // token contract
            uint16(4),   // accountMeta count
            packMeta(false, true,  from),
            packMeta(false, false, token),
            packMeta(false, true,  to),
            packMeta(true, false, signer),
            packMeta(false, true, owner),
            abi.encodePacked(
                uint8(12),        // transferChecked
                uint64(val),      // amount
                uint8(9)         // decimals
            )
        );

        bool status;
        bytes memory result;
        (status, result) = solana.call(instruction_data);
        if (!status) {revert();}
    }

    function packMeta(bool is_signer, bool is_writable, uint256 account) pure private returns(bytes memory) {
        return abi.encodePacked(false, is_signer, is_writable, account);
    }

    function packMeta(bool is_signer, bool is_writable, address account) pure private returns(bytes memory) {
        return abi.encodePacked(true, is_signer, is_writable, uint256(account));
    }

    function reverse(uint amount) private pure returns(uint64) {
        uint64 val = 0;
        for(uint i = 0; i < 8; i++) {
            val = (val << 8) | (uint64(amount) & 0xff);
            amount >>= 8;
        }
        if (amount != 0) {
            revert();
        }
        return val;
    }
}
