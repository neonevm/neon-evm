pragma solidity ^0.5.12;
pragma experimental ABIEncoderV2;

contract helloWorld {
    struct AccountMeta {
        bool need_translate;
        address name;
        bool is_signer;
        bool is_writable;
    }

    address solana = 0xfF00000000000000000000000000000000000000;

    function testCall() public { 
        bool status;
        
        string memory program_id = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
        
        AccountMeta[] memory accs = new AccountMeta[](3);
        accs[0] = AccountMeta(false, solana, true, false);
        accs[1] = AccountMeta(true, 0xBd770416a3345F91E4B34576cb804a576fa48EB1, true, false);
        accs[2] = AccountMeta(true, address(this), false, true);
        
        bytes memory instruction_data = hex"00000000000003e8"; // 1000
        
        bytes memory call_data = abi.encodeWithSignature(
            "(string, (address, bool, bool, bool)[], hex)",
            program_id, accs, instruction_data);

        bytes memory result;
        (status, result) = solana.call(call_data);
        if (!status) {
            revert();
        }
    }
}