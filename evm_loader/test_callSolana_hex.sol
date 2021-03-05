pragma solidity ^0.5.12;

contract callSolanaHex {

    address solana = 0xfF00000000000000000000000000000000000000;
    function testCall() public { 
        bool status;
        
        bytes memory call_data = hex"002b546f6b656e6b65675166655a79694e77414a624e62474b5046584357754276663953733632335651354441000300ff00000000000000000000000000000000000000010001bd770416a3345f91e4b34576cb804a576fa48eb1010001bd770416a3345f91e4b34576cb804a576fa48eb10001000800000000000003e8";
        
        bytes memory result;
        (status, result) = solana.call(call_data);
        if (!status) {
            revert();
        }
    }
}
