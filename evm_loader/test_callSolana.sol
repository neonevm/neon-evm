pragma solidity ^0.5.12;
pragma experimental ABIEncoderV2;

contract helloWorld {
    struct AccountMeta {
        address name;
        bool is_signer;
        bool is_writable;
        bool need_translate;
    }

    address solana = 0xfF00000000000000000000000000000000000000;

    function testCall() public { 
        bool status;
        
        string memory program_id = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
        
        AccountMeta[] memory accs = new AccountMeta[](3);
        accs[0] = AccountMeta(solana, true, false, false);
        accs[1] = AccountMeta(0xBd770416a3345F91E4B34576cb804a576fa48EB1, true, false, true);
        accs[2] = AccountMeta(address(this), false, true, true);
        
        bytes memory instruction_data = hex"7472616E7366657228Bd770416a3345F91E4B34576cb804a576fa48EB1Bd770416a3345F91E4B34576cb804a576fa48EB1000000000000000029";
        
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