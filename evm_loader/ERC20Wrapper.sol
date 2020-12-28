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
    
/*    function packMeta(bool need_translate, bool is_signer, bool is_writable, uint256 account) private returns(bytes memory) {
        return abi.encodePacked(need_translate, is_signer, is_writable, account);
    }*/
    
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

    function open(uint256 owner_id, address owner) public returns(bytes32) {
        bytes32 created = keccak256(abi.encodePacked(owner, address(this), owner));
        /*bool st;
        bytes memory res;
        (st, res) = solana.call(call_d);
        if(!st) {revert();}*/

        bytes memory accData1 = packMeta(true, true, owner);
        bytes memory accData2 = packMeta(false, true, uint256(created));
        bytes memory accData3 = packMeta(false, true, owner);
        
        bytes memory instruction_data = abi.encodePacked(uint8(3), owner_id, address(this), uint64(1), uint64(165), token_id);
        bytes memory call_data = abi.encodePacked(uint8(0), system_id, uint16(3), accData1, accData2, accData3, instruction_data);
        
        bool status;
        bytes memory result;
        (status, result) = solana.call(call_data);
        if (!status) {
            revert();
        }
    }
    
    function bytesToBytes32(bytes memory b, uint offset) private pure returns (bytes32) {
      bytes32 out;
    
      for (uint i = 0; i < 32; i++) {
        out = out << 8 | b[offset + (31-i)];
      }
      return out;
    }
    
    function makeBalance(address acc) public returns(bytes memory) {
        bool status;
        bytes memory result;
        
        bytes memory call_data = abi.encodePacked(uint8(1), true, false, uint256(acc), token_id, str);
        (status, result) = solana.call(call_data);
        if (!status) {
            revert();
        }
        return result;
    }
    
    function packCallData(AccountMeta[] memory accs, bytes memory instruction_data) pure private returns(bytes memory) {
        if (accs.length == 0) return abi.encodePacked(uint8(0), token_id, uint16(accs.length), instruction_data);
        
        bytes memory accData1 = packMeta(accs[0]);
        if (accs.length == 1) return abi.encodePacked(uint8(0), token_id, uint16(accs.length), accData1, instruction_data);
        
        bytes memory accData2 = packMeta(accs[1]);
        if (accs.length == 2) return abi.encodePacked(uint8(0), token_id, uint16(accs.length), accData1, accData2, instruction_data);
        
        bytes memory accData3 = packMeta(accs[2]);
        if (accs.length == 3) return abi.encodePacked(uint8(0), token_id, uint16(accs.length), accData1, accData2, accData3, instruction_data);
        
        bytes memory accData4 = packMeta(accs[3]);
        if (accs.length == 4) return abi.encodePacked(uint8(0), token_id, uint16(accs.length), accData1, accData2, accData3, accData4, instruction_data);
        
        revert();
    }
    
    function _callSolana(AccountMeta[] memory accs, bytes memory instruction_data) private returns(bytes memory) { 
        bool status;
/*        
        
        //uint256 program_id = 0x06ddf6e1d765a193d9cbe146ceeb79ac1cb485ed5f5b37913a8cf5857eff00a9; // hex representation of "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"


//      bytes memory call_data = abi.encodeWithSignature(
//          "(string, (bytes, bool, bool, bool)[], hex)",
//          program_id, accs, instruction_data);
        
        if (accs.length == 0) {
            call_data = abi.encodePacked(uint8(0), token_id, uint16(accs.length), instruction_data);
        } else {
            bytes memory accData1 = packMeta(accs[0]);
            bytes memory accData = abi.encodePacked(accs[0].need_translate, accs[0].is_signer, accs[0].is_writable, accs[0].account);
        }
        bytes memory accData2 = abi.encodePacked(accs[1].need_translate, accs[1].is_signer, accs[1].is_writable, accs[1].account);
        bytes memory accData3 = abi.encodePacked(accs[2].need_translate, accs[2].is_signer, accs[2].is_writable, accs[2].account);

        bytes memory call_data = abi.encodePacked(uint8(0), token_id, uint16(accs.length), accData, accData2, accData3, instruction_data);

/*function concat(bytes memory self, bytes memory other)
returns (bytes memory) {
     bytes memory ret = new bytes(self.length + other.length);
     var (src, srcLen) = Memory.fromBytes(self);
     var (src2, src2Len) = Memory.fromBytes(other);
     var (dest,) = Memory.fromBytes(ret);
     var dest2 = dest + src2Len;
     Memory.copy(src, dest, srcLen);
     Memory.copy(src2, dest2, src2Len);
     return ret;
}*/

        bytes memory result;
        (status, result) = solana.call(packCallData(accs, instruction_data));
        if (!status) {
            revert();
        }
        return result;
    }

    /*function bytesToUint(bytes memory b) public returns (uint256) {
        uint256 number;
        for(uint i = 0; i < b.length; i++){
            number = number + uint(b[i]) * (2** (8* (b.length - (i+1))));
        }
        return number;
    }*/

/*    function transferFrom(address from, address to, uint amount) public {
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

    function transfer(address to, uint amount) public {
        bytes memory instruction_data = abi.encodePacked(
                    uint8(0),    // external call
                    token_id,    // token contract
                    uint16(4),   // accountMeta count
                        packMeta(false, true,  makeBalance(msg.sender)),
                        packMeta(false, false, mint_id),
                        packMeta(false, true,  makeBalance(to)),
                        packMeta(true,  false, msg.sender),
                    abi.encodePacked(
                        uint8(12),        // transferChecked
                        uint64(amount),   // amount
                        uint64(9)         // decimals
                    )
                );
                    
        bool status;
        bytes memory result;
        (status, result) = solana.call(instruction_data);
        if (!status) {revert();}
    }
    
    function reverse(uint amount) public pure returns(uint64) {
        uint64 val = 0;
        for(uint i = 0; i < 8; i++) {
            val = (val << 8) | (uint64(amount) & 0xff);
            amount >>= 8;
        }
        return val;
    }
    
    function transfer(uint256 to, uint amount) public {
        uint64 val = reverse(amount);
        bytes memory instruction_data = abi.encodePacked(
                    uint8(0),    // external call
                    token_id,    // token contract
                    uint16(4),   // accountMeta count
                        packMeta(false, true,  makeBalance(msg.sender)),
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
    
/*    function transferSol(bytes memory to, uint amount) public {
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
