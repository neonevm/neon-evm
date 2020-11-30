pragma solidity ^0.5.12;

contract helloWorld {

    address solana = 0xfF00000000000000000000000000000000000000;
    function testCall() public { 
        bool status;
        
        bytes memory call_data = hex"002b546f6b656e6b65675166655a79694e77414a624e62474b50465843577542766639537336323356513544410003ff00000000000000000000000000000000000000010000bd770416a3345f91e4b34576cb804a576fa48eb10100017472616e7366657228bd770416a3345f91e4b34576cb804a576fa48eb1bd770416a3345f91e4b34576cb804a576fa48eb1000000000000000029";
        
        bytes memory result;
        (status, result) = solana.call(call_data);
        if (!status) {
            revert();
        }
    }
}