pragma solidity >=0.5.0;

contract SolidityPrecompilesTest {
    function test_01_ecrecover(bytes32 hash, uint8 v, bytes32 r, bytes32 s) public pure returns (address) {
        return ecrecover(hash, v, r, s);
    }

    function test_02_sha256(bytes memory data) public pure returns (bytes32) {
        return sha256(data);
    }

    function test_03_ripemd160(bytes memory data) public pure returns (bytes20) {
        return ripemd160(data);
    }

    function test_04_dataCopy(bytes memory data) public returns (bytes memory) {
        bytes memory ret = new bytes(data.length);
        assembly {
            let len := mload(data)
            if iszero(call(gas(), 0x04, 0, add(data, 0x20), len, add(ret,0x20), len)) {
                invalid()
            }
        }

        return ret;
    }

    function test_05_bigModExp(bytes32 base, bytes32 exponent, bytes32 modulus) public returns (bytes32 result) {
        assembly {
            // free memory pointer
            let memPtr := mload(0x40)

            // length of base, exponent, modulus
            mstore(memPtr, 0x20)
            mstore(add(memPtr, 0x20), 0x20)
            mstore(add(memPtr, 0x40), 0x20)

            // assign base, exponent, modulus
            mstore(add(memPtr, 0x60), base)
            mstore(add(memPtr, 0x80), exponent)
            mstore(add(memPtr, 0xa0), modulus)

            // call the precompiled contract BigModExp (0x05)
            if iszero(call(gas(), 0x05, 0x0, memPtr, 0xc0, memPtr, 0x20)) {
                revert(0x0, 0x0)
            }
            result := mload(memPtr)
        }
    }

    function test_06_bn256Add(bytes32 ax, bytes32 ay, bytes32 bx, bytes32 by) public returns (bytes32[2] memory result) {
        bytes32[4] memory input;
        input[0] = ax;
        input[1] = ay;
        input[2] = bx;
        input[3] = by;
        assembly {
            if iszero(call(gas(), 0x06, 0, input, 0x80, result, 0x40)) {
                revert(0,0)
            }
        }
    }

    function test_07_bn256ScalarMul(bytes32 x, bytes32 y, bytes32 scalar) public returns (bytes32[2] memory result) {
        bytes32[3] memory input;
        input[0] = x;
        input[1] = y;
        input[2] = scalar;
        assembly {
            if iszero(call(gas(), 0x07, 0, input, 0x60, result, 0x40)) {
                revert(0,0)
            }
        }
    }

    function test_08_bn256Pairing(bytes memory input) public returns (bytes32 result) {
        // input is a serialized bytes stream of (a1, b1, a2, b2, ..., ak, bk) from (G_1 x G_2)^k
        uint256 len = input.length;
        require(len % 192 == 0);
        assembly {
            let memPtr := mload(0x40)
            if iszero(call(gas(), 0x08, 0, add(input, 0x20), len, memPtr, 0x20)) {
                revert(0,0)
            }
            result := mload(memPtr)
        }
    }

    function test_09_blake2F(bytes memory input) public returns (bytes32[2] memory output) {
        assembly {
            if iszero(call(gas(), 0x09, 0, add(input, 32), 0xd5, output, 0x40)) {
                revert(0, 0)
            }
        }
    }
}
