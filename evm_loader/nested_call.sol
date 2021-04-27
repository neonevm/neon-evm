pragma solidity ^0.5.12;

contract nested_call_Recover{
    event Recovered(address a);


    function recovery_signer (bytes32 hash, bytes memory sig) public returns (address){
        bytes32 r;
        bytes32 s;
        uint8 v;

        if (sig.length != 65) {
          return address(0);
        }

        assembly {
          r := mload(add(sig, 32))
          s := mload(add(sig, 64))
          v := and(mload(add(sig, 65)), 255)
        }

        // https://github.com/ethereum/go-ethereum/issues/2053
        if (v < 27) {
          v += 27;
        }

        if (v != 27 && v != 28) {
          return address(0);
        }

        address a;
        a = ecrecover(hash, v, r, s);
        emit Recovered(a);
        return a;
    }
}

contract nested_call_Receiver {
    event Foo(address caller, uint amount, string message);
    event Response_recovery_signer(bool success, bytes a);

    function foo(string memory _message, uint _x) public payable returns (uint) {
        emit Foo(msg.sender, msg.value, _message);
        return _x + 1;
    }

    function recover (address recover_addr, bytes32 hash, bytes memory sig) public returns (bool) {
        (bool success, bytes memory signer) = recover_addr.call(abi.encodeWithSignature("recovery_signer(bytes32,bytes)", hash, sig));
        emit Response_recovery_signer(success, signer);
        return true;
    }
}

contract nested_call_Caller {
    event Result(bool success, bytes data);

    function callFoo(address payable receiver) public payable {

        (bool success, bytes memory data) = receiver.call(
                abi.encodeWithSignature("foo(string,uint256)", "call foo", 123)
            );
        emit Result(success, data);
    }

    function callRecover(address receiver_addr, address recover_addr, bytes32 hash, bytes memory signature) public {
         (bool success, bytes memory res) = receiver_addr.call(
                        abi.encodeWithSignature("recover(address,bytes32,bytes)",recover_addr, hash, signature)
                    );
         emit Result(success, res);
    }

}