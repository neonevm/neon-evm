#![allow(clippy::type_complexity)]

type OpCode = &'static str;

const OPCODE_UNKNOWN: OpCode = "<invalid>";
pub const OPCODES: [OpCode; 256] = {
    let mut opcodes: [OpCode; 256] = [OPCODE_UNKNOWN; 256];

    opcodes[0x00] = "STOP";
    opcodes[0x01] = "ADD";
    opcodes[0x02] = "MUL";
    opcodes[0x03] = "SUB";
    opcodes[0x04] = "DIV";
    opcodes[0x05] = "SDIV";
    opcodes[0x06] = "MOD";
    opcodes[0x07] = "SMOD";
    opcodes[0x08] = "ADDMOD";
    opcodes[0x09] = "MULMOD";
    opcodes[0x0A] = "EXP";
    opcodes[0x0B] = "SIGNEXTEND";

    opcodes[0x10] = "LT";
    opcodes[0x11] = "GT";
    opcodes[0x12] = "SLT";
    opcodes[0x13] = "SGT";
    opcodes[0x14] = "EQ";
    opcodes[0x15] = "ISZERO";
    opcodes[0x16] = "AND";
    opcodes[0x17] = "OR";
    opcodes[0x18] = "XOR";
    opcodes[0x19] = "NOT";
    opcodes[0x1A] = "BYTE";
    opcodes[0x1B] = "SHL";
    opcodes[0x1C] = "SHR";
    opcodes[0x1D] = "SAR";

    opcodes[0x20] = "KECCAK256";

    opcodes[0x30] = "ADDRESS";
    opcodes[0x31] = "BALANCE";
    opcodes[0x32] = "ORIGIN";
    opcodes[0x33] = "CALLER";
    opcodes[0x34] = "CALLVALUE";
    opcodes[0x35] = "CALLDATALOAD";
    opcodes[0x36] = "CALLDATASIZE";
    opcodes[0x37] = "CALLDATACOPY";
    opcodes[0x38] = "CODESIZE";
    opcodes[0x39] = "CODECOPY";
    opcodes[0x3A] = "GASPRICE";
    opcodes[0x3B] = "EXTCODESIZE";
    opcodes[0x3C] = "EXTCODECOPY";
    opcodes[0x3D] = "RETURNDATASIZE";
    opcodes[0x3E] = "RETURNDATACOPY";
    opcodes[0x3F] = "EXTCODEHASH";
    opcodes[0x40] = "BLOCKHASH";
    opcodes[0x41] = "COINBASE";
    opcodes[0x42] = "TIMESTAMP";
    opcodes[0x43] = "NUMBER";
    opcodes[0x44] = "PREVRANDAO";
    opcodes[0x45] = "GASLIMIT";
    opcodes[0x46] = "CHAINID";
    opcodes[0x47] = "SELFBALANCE";
    opcodes[0x48] = "BASEFEE";

    opcodes[0x50] = "POP";
    opcodes[0x51] = "MLOAD";
    opcodes[0x52] = "MSTORE";
    opcodes[0x53] = "MSTORE8";
    opcodes[0x54] = "SLOAD";
    opcodes[0x55] = "SSTORE";
    opcodes[0x56] = "JUMP";
    opcodes[0x57] = "JUMPI";
    opcodes[0x58] = "PC";
    opcodes[0x59] = "MSIZE";
    opcodes[0x5A] = "GAS";
    opcodes[0x5B] = "JUMPDEST";

    opcodes[0x5F] = "PUSH0";
    opcodes[0x60] = "PUSH1";
    opcodes[0x61] = "PUSH2";
    opcodes[0x62] = "PUSH3";
    opcodes[0x63] = "PUSH4";
    opcodes[0x64] = "PUSH5";
    opcodes[0x65] = "PUSH6";
    opcodes[0x66] = "PUSH7";
    opcodes[0x67] = "PUSH8";
    opcodes[0x68] = "PUSH9";
    opcodes[0x69] = "PUSH10";
    opcodes[0x6A] = "PUSH11";
    opcodes[0x6B] = "PUSH12";
    opcodes[0x6C] = "PUSH13";
    opcodes[0x6D] = "PUSH14";
    opcodes[0x6E] = "PUSH15";
    opcodes[0x6F] = "PUSH16";
    opcodes[0x70] = "PUSH17";
    opcodes[0x71] = "PUSH18";
    opcodes[0x72] = "PUSH19";
    opcodes[0x73] = "PUSH20";
    opcodes[0x74] = "PUSH21";
    opcodes[0x75] = "PUSH22";
    opcodes[0x76] = "PUSH23";
    opcodes[0x77] = "PUSH24";
    opcodes[0x78] = "PUSH25";
    opcodes[0x79] = "PUSH26";
    opcodes[0x7A] = "PUSH27";
    opcodes[0x7B] = "PUSH28";
    opcodes[0x7C] = "PUSH29";
    opcodes[0x7D] = "PUSH30";
    opcodes[0x7E] = "PUSH31";
    opcodes[0x7F] = "PUSH32";

    opcodes[0x80] = "DUP1";
    opcodes[0x81] = "DUP2";
    opcodes[0x82] = "DUP3";
    opcodes[0x83] = "DUP4";
    opcodes[0x84] = "DUP5";
    opcodes[0x85] = "DUP6";
    opcodes[0x86] = "DUP7";
    opcodes[0x87] = "DUP8";
    opcodes[0x88] = "DUP9";
    opcodes[0x89] = "DUP10";
    opcodes[0x8A] = "DUP11";
    opcodes[0x8B] = "DUP12";
    opcodes[0x8C] = "DUP13";
    opcodes[0x8D] = "DUP14";
    opcodes[0x8E] = "DUP15";
    opcodes[0x8F] = "DUP16";

    opcodes[0x90] = "SWAP1";
    opcodes[0x91] = "SWAP2";
    opcodes[0x92] = "SWAP3";
    opcodes[0x93] = "SWAP4";
    opcodes[0x94] = "SWAP5";
    opcodes[0x95] = "SWAP6";
    opcodes[0x96] = "SWAP7";
    opcodes[0x97] = "SWAP8";
    opcodes[0x98] = "SWAP9";
    opcodes[0x99] = "SWAP10";
    opcodes[0x9A] = "SWAP11";
    opcodes[0x9B] = "SWAP12";
    opcodes[0x9C] = "SWAP13";
    opcodes[0x9D] = "SWAP14";
    opcodes[0x9E] = "SWAP15";
    opcodes[0x9F] = "SWAP16";

    opcodes[0xA0] = "LOG0";
    opcodes[0xA1] = "LOG1";
    opcodes[0xA2] = "LOG2";
    opcodes[0xA3] = "LOG3";
    opcodes[0xA4] = "LOG4";

    opcodes[0xF0] = "CREATE";
    opcodes[0xF1] = "CALL";
    opcodes[0xF2] = "CALLCODE";
    opcodes[0xF3] = "RETURN";
    opcodes[0xF4] = "DELEGATECALL";
    opcodes[0xF5] = "CREATE2";

    opcodes[0xFA] = "STATICCALL";

    opcodes[0xFD] = "REVERT";
    opcodes[0xFE] = "INVALID";

    opcodes[0xFF] = "SELFDESTRUCT";

    opcodes
};
