pragma solidity ^0.5.12;
//pragma experimental ABIEncoderV2;

contract ERC20Wrapper {
    struct AccountMeta {
        bool need_translate;
        bool is_signer;
        bool is_writable;
        uint256 account;
    }

    uint256 constant system_id = 0x0000000000000000000000000000000000000000000000000000000000000000;  // hex representation of "11111111111111111111111111111111"
    uint256 constant token_id = 0x06ddf6e1d765a193d9cbe146ceeb79ac1cb485ed5f5b37913a8cf5857eff00a9; // hex representation of "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
    address constant solana = 0xfF00000000000000000000000000000000000000;
    uint256 mint_id;
    string str;
    
    function setToken(uint256 _mint, string memory _str) public {
        // TODO: Move to initialize
        mint_id = _mint;
        str = _str;
    }
    
    function packMeta(bool is_signer, bool is_writable, uint256 account) pure private returns(bytes memory) {
        return abi.encodePacked(false, is_signer, is_writable, account);
    }
    function packMeta(bool is_signer, bool is_writable, address account) pure private returns(bytes memory) {
        return abi.encodePacked(true, is_signer, is_writable, uint256(account));
    }
    function packMeta(bool is_signer, bool is_writable, bytes memory account) pure private returns(bytes memory) {
        if(account.length != 32) {revert();}
        return abi.encodePacked(false, is_signer, is_writable, account);
    }
    function packMeta(AccountMeta memory meta) pure private returns(bytes memory) {
        return abi.encodePacked(meta.need_translate, meta.is_signer, meta.is_writable, meta.account);
    }

    function makeBalanceAccount(address acc) public returns(bytes memory) {
        bool status;
        bytes memory result;
        
        bytes memory call_data = abi.encodePacked(uint8(1), true, false, uint256(acc), token_id, str);
        (status, result) = solana.call(call_data);
        if (!status) {
            revert();
        }
        return result;
    }

/*  TODO: need to implements
    function transferFrom(address from, address to, uint amount) public {
        uint8 instr_id = 0x0;

        AccountMeta[] memory accs = new AccountMeta[](2);
        accs[0] = AccountMeta(true, abi.encodePacked(from), true, false);
        accs[1] = AccountMeta(true, abi.encodePacked(to), false, true);

        _callSolana(accs, abi.encodePacked(instr_id, amount));
    }

    function transferFromSol(bytes memory from, bytes memory to, uint amount) public {
        uint8 instr_id = 0x0;

        AccountMeta[] memory accs = new AccountMeta[](2);
        accs[0] = AccountMeta(false, from, true, false);
        accs[1] = AccountMeta(false, to, false, true);

        _callSolana(accs, abi.encodePacked(instr_id, amount));
    }*/

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
    
    function transfer(address to, uint amount) public {
        uint64 val = reverse(amount);
        bytes memory instruction_data = abi.encodePacked(
                    uint8(0),    // external call
                    token_id,    // token contract
                    uint16(4),   // accountMeta count
                        packMeta(false, true,  makeBalanceAccount(msg.sender)),
                        packMeta(false, false, mint_id),
                        packMeta(false, true,  makeBalanceAccount(to)),
                        packMeta(true,  false, msg.sender),
                    abi.encodePacked(
                        uint8(12),        // transferChecked
                        uint64(val),      // amount
                        uint64(9)         // decimals
                    )
                );
                    
        bool status;
        bytes memory result;
        (status, result) = solana.call(instruction_data);
        if (!status) {revert();}
    }
    
    function transfer(uint256 to, uint amount) public {
        uint64 val = reverse(amount);
        bytes memory instruction_data = abi.encodePacked(
                    uint8(0),    // external call
                    token_id,    // token contract
                    uint16(4),   // accountMeta count
                        packMeta(false, true,  makeBalanceAccount(msg.sender)),
                        packMeta(false, false, mint_id),
                        packMeta(false, true,  to),
                        packMeta(true,  false, msg.sender),
                    abi.encodePacked(
                        uint8(12),        // transferChecked
                        uint64(val),      // amount
                        uint8(9)          // decimals
                    )
                );
                    
        bool status;
        bytes memory result;
        (status, result) = solana.call(instruction_data);
        if (!status) {revert();}
    }
    
/*  TODO: need to implements
    function transferSol(bytes memory to, uint amount) public {
        uint8 instr_id = 0x1;

        AccountMeta[] memory accs = new AccountMeta[](2);
        accs[0] = AccountMeta(false, abi.encodePacked(address(this)), true, false);
        accs[1] = AccountMeta(false, to, false, true);

        _callSolana(accs, abi.encodePacked(instr_id, amount));
    }*/

    /*function totalSupply() public returns (uint256) {
        uint8 instr_id = 0x2;

        AccountMeta[] memory accs = new AccountMeta[](0);
        bytes memory result = _callSolana(accs, abi.encodePacked(instr_id));

        return bytesToUint(result);
    }

    function balanceOf(bool holder_nt, bytes memory holder) public returns (uint256) {
        uint8 instr_id = 0x3;

        AccountMeta[] memory accs = new AccountMeta[](0);
        accs[0] = AccountMeta(holder_nt, abi.encodePacked(holder), true, false);
        bytes memory result = _callSolana(accs, abi.encodePacked(instr_id));

        return bytesToUint(result);
    }*/
}
