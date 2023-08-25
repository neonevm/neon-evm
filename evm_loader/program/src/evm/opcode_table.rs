#![allow(clippy::type_complexity)]

use crate::error::Result;

use super::{database::Database, opcode::Action, Machine};

type OpCode<B> = (&'static str, fn(&mut Machine<B>, &mut B) -> Result<Action>);

impl<B: Database> Machine<B> {
    const OPCODE_UNKNOWN: OpCode<B> = ("<invalid>", Self::opcode_unknown);
    pub const OPCODES: [OpCode<B>; 256] = {
        let mut opcodes: [OpCode<B>; 256] = [Self::OPCODE_UNKNOWN; 256];

        opcodes[0x00] = ("STOP", Self::opcode_stop);
        opcodes[0x01] = ("ADD", Self::opcode_add);
        opcodes[0x02] = ("MUL", Self::opcode_mul);
        opcodes[0x03] = ("SUB", Self::opcode_sub);
        opcodes[0x04] = ("DIV", Self::opcode_div);
        opcodes[0x05] = ("SDIV", Self::opcode_sdiv);
        opcodes[0x06] = ("MOD", Self::opcode_mod);
        opcodes[0x07] = ("SMOD", Self::opcode_smod);
        opcodes[0x08] = ("ADDMOD", Self::opcode_addmod);
        opcodes[0x09] = ("MULMOD", Self::opcode_mulmod);
        opcodes[0x0A] = ("EXP", Self::opcode_exp);
        opcodes[0x0B] = ("SIGNEXTEND", Self::opcode_signextend);

        opcodes[0x10] = ("LT", Self::opcode_lt);
        opcodes[0x11] = ("GT", Self::opcode_gt);
        opcodes[0x12] = ("SLT", Self::opcode_slt);
        opcodes[0x13] = ("SGT", Self::opcode_sgt);
        opcodes[0x14] = ("EQ", Self::opcode_eq);
        opcodes[0x15] = ("ISZERO", Self::opcode_iszero);
        opcodes[0x16] = ("AND", Self::opcode_and);
        opcodes[0x17] = ("OR", Self::opcode_or);
        opcodes[0x18] = ("XOR", Self::opcode_xor);
        opcodes[0x19] = ("NOT", Self::opcode_not);
        opcodes[0x1A] = ("BYTE", Self::opcode_byte);
        opcodes[0x1B] = ("SHL", Self::opcode_shl);
        opcodes[0x1C] = ("SHR", Self::opcode_shr);
        opcodes[0x1D] = ("SAR", Self::opcode_sar);

        opcodes[0x20] = ("KECCAK256", Self::opcode_sha3);

        opcodes[0x30] = ("ADDRESS", Self::opcode_address);
        opcodes[0x31] = ("BALANCE", Self::opcode_balance);
        opcodes[0x32] = ("ORIGIN", Self::opcode_origin);
        opcodes[0x33] = ("CALLER", Self::opcode_caller);
        opcodes[0x34] = ("CALLVALUE", Self::opcode_callvalue);
        opcodes[0x35] = ("CALLDATALOAD", Self::opcode_calldataload);
        opcodes[0x36] = ("CALLDATASIZE", Self::opcode_calldatasize);
        opcodes[0x37] = ("CALLDATACOPY", Self::opcode_calldatacopy);
        opcodes[0x38] = ("CODESIZE", Self::opcode_codesize);
        opcodes[0x39] = ("CODECOPY", Self::opcode_codecopy);
        opcodes[0x3A] = ("GASPRICE", Self::opcode_gasprice);
        opcodes[0x3B] = ("EXTCODESIZE", Self::opcode_extcodesize);
        opcodes[0x3C] = ("EXTCODECOPY", Self::opcode_extcodecopy);
        opcodes[0x3D] = ("RETURNDATASIZE", Self::opcode_returndatasize);
        opcodes[0x3E] = ("RETURNDATACOPY", Self::opcode_returndatacopy);
        opcodes[0x3F] = ("EXTCODEHASH", Self::opcode_extcodehash);
        opcodes[0x40] = ("BLOCKHASH", Self::opcode_blockhash);
        opcodes[0x41] = ("COINBASE", Self::opcode_coinbase);
        opcodes[0x42] = ("TIMESTAMP", Self::opcode_timestamp);
        opcodes[0x43] = ("NUMBER", Self::opcode_number);
        opcodes[0x44] = ("PREVRANDAO", Self::opcode_difficulty);
        opcodes[0x45] = ("GASLIMIT", Self::opcode_gaslimit);
        opcodes[0x46] = ("CHAINID", Self::opcode_chainid);
        opcodes[0x47] = ("SELFBALANCE", Self::opcode_selfbalance);
        opcodes[0x48] = ("BASEFEE", Self::opcode_basefee);

        opcodes[0x50] = ("POP", Self::opcode_pop);
        opcodes[0x51] = ("MLOAD", Self::opcode_mload);
        opcodes[0x52] = ("MSTORE", Self::opcode_mstore);
        opcodes[0x53] = ("MSTORE8", Self::opcode_mstore8);
        opcodes[0x54] = ("SLOAD", Self::opcode_sload);
        opcodes[0x55] = ("SSTORE", Self::opcode_sstore);
        opcodes[0x56] = ("JUMP", Self::opcode_jump);
        opcodes[0x57] = ("JUMPI", Self::opcode_jumpi);
        opcodes[0x58] = ("PC", Self::opcode_pc);
        opcodes[0x59] = ("MSIZE", Self::opcode_msize);
        opcodes[0x5A] = ("GAS", Self::opcode_gas);
        opcodes[0x5B] = ("JUMPDEST", Self::opcode_jumpdest);

        opcodes[0x5F] = ("PUSH0", Self::opcode_push_0);
        opcodes[0x60] = ("PUSH1", Self::opcode_push_1);
        opcodes[0x61] = ("PUSH2", Self::opcode_push_2_31::<2>);
        opcodes[0x62] = ("PUSH3", Self::opcode_push_2_31::<3>);
        opcodes[0x63] = ("PUSH4", Self::opcode_push_2_31::<4>);
        opcodes[0x64] = ("PUSH5", Self::opcode_push_2_31::<5>);
        opcodes[0x65] = ("PUSH6", Self::opcode_push_2_31::<6>);
        opcodes[0x66] = ("PUSH7", Self::opcode_push_2_31::<7>);
        opcodes[0x67] = ("PUSH8", Self::opcode_push_2_31::<8>);
        opcodes[0x68] = ("PUSH9", Self::opcode_push_2_31::<9>);
        opcodes[0x69] = ("PUSH10", Self::opcode_push_2_31::<10>);
        opcodes[0x6A] = ("PUSH11", Self::opcode_push_2_31::<11>);
        opcodes[0x6B] = ("PUSH12", Self::opcode_push_2_31::<12>);
        opcodes[0x6C] = ("PUSH13", Self::opcode_push_2_31::<13>);
        opcodes[0x6D] = ("PUSH14", Self::opcode_push_2_31::<14>);
        opcodes[0x6E] = ("PUSH15", Self::opcode_push_2_31::<15>);
        opcodes[0x6F] = ("PUSH16", Self::opcode_push_2_31::<16>);
        opcodes[0x70] = ("PUSH17", Self::opcode_push_2_31::<17>);
        opcodes[0x71] = ("PUSH18", Self::opcode_push_2_31::<18>);
        opcodes[0x72] = ("PUSH19", Self::opcode_push_2_31::<19>);
        opcodes[0x73] = ("PUSH20", Self::opcode_push_2_31::<20>);
        opcodes[0x74] = ("PUSH21", Self::opcode_push_2_31::<21>);
        opcodes[0x75] = ("PUSH22", Self::opcode_push_2_31::<22>);
        opcodes[0x76] = ("PUSH23", Self::opcode_push_2_31::<23>);
        opcodes[0x77] = ("PUSH24", Self::opcode_push_2_31::<24>);
        opcodes[0x78] = ("PUSH25", Self::opcode_push_2_31::<25>);
        opcodes[0x79] = ("PUSH26", Self::opcode_push_2_31::<26>);
        opcodes[0x7A] = ("PUSH27", Self::opcode_push_2_31::<27>);
        opcodes[0x7B] = ("PUSH28", Self::opcode_push_2_31::<28>);
        opcodes[0x7C] = ("PUSH29", Self::opcode_push_2_31::<29>);
        opcodes[0x7D] = ("PUSH30", Self::opcode_push_2_31::<30>);
        opcodes[0x7E] = ("PUSH31", Self::opcode_push_2_31::<31>);
        opcodes[0x7F] = ("PUSH32", Self::opcode_push_32);

        opcodes[0x80] = ("DUP1", Self::opcode_dup_1_16::<1>);
        opcodes[0x81] = ("DUP2", Self::opcode_dup_1_16::<2>);
        opcodes[0x82] = ("DUP3", Self::opcode_dup_1_16::<3>);
        opcodes[0x83] = ("DUP4", Self::opcode_dup_1_16::<4>);
        opcodes[0x84] = ("DUP5", Self::opcode_dup_1_16::<5>);
        opcodes[0x85] = ("DUP6", Self::opcode_dup_1_16::<6>);
        opcodes[0x86] = ("DUP7", Self::opcode_dup_1_16::<7>);
        opcodes[0x87] = ("DUP8", Self::opcode_dup_1_16::<8>);
        opcodes[0x88] = ("DUP9", Self::opcode_dup_1_16::<9>);
        opcodes[0x89] = ("DUP10", Self::opcode_dup_1_16::<10>);
        opcodes[0x8A] = ("DUP11", Self::opcode_dup_1_16::<11>);
        opcodes[0x8B] = ("DUP12", Self::opcode_dup_1_16::<12>);
        opcodes[0x8C] = ("DUP13", Self::opcode_dup_1_16::<13>);
        opcodes[0x8D] = ("DUP14", Self::opcode_dup_1_16::<14>);
        opcodes[0x8E] = ("DUP15", Self::opcode_dup_1_16::<15>);
        opcodes[0x8F] = ("DUP16", Self::opcode_dup_1_16::<16>);

        opcodes[0x90] = ("SWAP1", Self::opcode_swap_1_16::<1>);
        opcodes[0x91] = ("SWAP2", Self::opcode_swap_1_16::<2>);
        opcodes[0x92] = ("SWAP3", Self::opcode_swap_1_16::<3>);
        opcodes[0x93] = ("SWAP4", Self::opcode_swap_1_16::<4>);
        opcodes[0x94] = ("SWAP5", Self::opcode_swap_1_16::<5>);
        opcodes[0x95] = ("SWAP6", Self::opcode_swap_1_16::<6>);
        opcodes[0x96] = ("SWAP7", Self::opcode_swap_1_16::<7>);
        opcodes[0x97] = ("SWAP8", Self::opcode_swap_1_16::<8>);
        opcodes[0x98] = ("SWAP9", Self::opcode_swap_1_16::<9>);
        opcodes[0x99] = ("SWAP10", Self::opcode_swap_1_16::<10>);
        opcodes[0x9A] = ("SWAP11", Self::opcode_swap_1_16::<11>);
        opcodes[0x9B] = ("SWAP12", Self::opcode_swap_1_16::<12>);
        opcodes[0x9C] = ("SWAP13", Self::opcode_swap_1_16::<13>);
        opcodes[0x9D] = ("SWAP14", Self::opcode_swap_1_16::<14>);
        opcodes[0x9E] = ("SWAP15", Self::opcode_swap_1_16::<15>);
        opcodes[0x9F] = ("SWAP16", Self::opcode_swap_1_16::<16>);

        opcodes[0xA0] = ("LOG0", Self::opcode_log_0_4::<0>);
        opcodes[0xA1] = ("LOG1", Self::opcode_log_0_4::<1>);
        opcodes[0xA2] = ("LOG2", Self::opcode_log_0_4::<2>);
        opcodes[0xA3] = ("LOG3", Self::opcode_log_0_4::<3>);
        opcodes[0xA4] = ("LOG4", Self::opcode_log_0_4::<4>);

        opcodes[0xF0] = ("CREATE", Self::opcode_create);
        opcodes[0xF1] = ("CALL", Self::opcode_call);
        opcodes[0xF2] = ("CALLCODE", Self::opcode_callcode);
        opcodes[0xF3] = ("RETURN", Self::opcode_return);
        opcodes[0xF4] = ("DELEGATECALL", Self::opcode_delegatecall);
        opcodes[0xF5] = ("CREATE2", Self::opcode_create2);

        opcodes[0xFA] = ("STATICCALL", Self::opcode_staticcall);

        opcodes[0xFD] = ("REVERT", Self::opcode_revert);
        opcodes[0xFE] = ("INVALID", Self::opcode_invalid);

        opcodes[0xFF] = ("SELFDESTRUCT", Self::opcode_selfdestruct);

        opcodes
    };
}
