pragma solidity ^0.5.12;
pragma experimental ABIEncoderV2;

contract ERC20Wrapper {
    struct AccountMeta {
        bool need_translate;
        bytes name;
        bool is_signer;
        bool is_writable;
    }

    address solana = 0xfF00000000000000000000000000000000000000;

    function _callSolana(AccountMeta[] memory accs, bytes memory instruction_data) private returns(bytes memory) { 
        bool status;
        
        string memory program_id = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

        bytes memory call_data = abi.encodeWithSignature(
            "(string, (bytes, bool, bool, bool)[], hex)",
            program_id, accs, instruction_data);

        bytes memory result;
        (status, result) = solana.call(call_data);
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
    
    function accMetas(uint size) private view returns(AccountMeta[] memory) {
        AccountMeta[] memory accs = new AccountMeta[](size);
        accs[0] = AccountMeta(false, abi.encodePacked(solana), true, false);
        return accs;
    }

    function transferFrom(address from, address to, uint amount) public {
        uint8 instr_id = 0x0;

        AccountMeta[] memory accs = accMetas(3);
        accs[1] = AccountMeta(true, abi.encodePacked(from), true, false);
        accs[2] = AccountMeta(true, abi.encodePacked(to), false, true);

        _callSolana(accs, abi.encodePacked(instr_id, amount));
    }

    function transferFromSol(bytes memory from, bytes memory to, uint amount) public {
        uint8 instr_id = 0x0;

        AccountMeta[] memory accs = accMetas(3);
        accs[1] = AccountMeta(false, from, true, false);
        accs[2] = AccountMeta(false, to, false, true);

        _callSolana(accs, abi.encodePacked(instr_id, amount));
    }

    function transfer(address to, uint amount) public {
        uint8 instr_id = 0x0;

        AccountMeta[] memory accs = accMetas(3);
        accs[1] = AccountMeta(true, abi.encodePacked(address(this)), true, false);
        accs[2] = AccountMeta(true, abi.encodePacked(to), false, true);

        _callSolana(accs, abi.encodePacked(instr_id, amount));
    }

    function transferSol(bytes memory to, uint amount) public {
        uint8 instr_id = 0x1;

        AccountMeta[] memory accs = accMetas(3);
        accs[1] = AccountMeta(false, abi.encodePacked(address(this)), true, false);
        accs[2] = AccountMeta(false, to, false, true);

        _callSolana(accs, abi.encodePacked(instr_id, amount));
    }

    /*function totalSupply() public returns (uint256) {
        uint8 instr_id = 0x2;

        AccountMeta[] memory accs = accMetas(1);
        bytes memory result = _callSolana(accs, abi.encodePacked(instr_id));

        return bytesToUint(result);
    }

    function balanceOf(bool holder_nt, bytes memory holder) public returns (uint256) {
        uint8 instr_id = 0x3;

        AccountMeta[] memory accs = accMetas(2);
        accs[1] = AccountMeta(holder_nt, abi.encodePacked(holder), true, false);
        bytes memory result = _callSolana(accs, abi.encodePacked(instr_id));

        return bytesToUint(result);
    }*/
}