#![allow(clippy::type_complexity)]

use crate::error::Result;

use super::{
    Machine, 
    opcode::Action, 
    database::Database
};


impl<B: Database> Machine<B> {
    pub const OPCODES: [fn(&mut Self, &mut B) -> Result<Action>; 256] = {
        let mut opcodes: [fn(&mut Self, &mut B) -> Result<Action>; 256] = [Self::opcode_unknown; 256];

        opcodes[0x00] = Self::opcode_stop;
        opcodes[0x01] = Self::opcode_add;
        opcodes[0x02] = Self::opcode_mul;
        opcodes[0x03] = Self::opcode_sub;
        opcodes[0x04] = Self::opcode_div;
        opcodes[0x05] = Self::opcode_sdiv;
        opcodes[0x06] = Self::opcode_mod;
        opcodes[0x07] = Self::opcode_smod;
        opcodes[0x08] = Self::opcode_addmod;
        opcodes[0x09] = Self::opcode_mulmod;
        opcodes[0x0A] = Self::opcode_exp;
        opcodes[0x0B] = Self::opcode_signextend;

        opcodes[0x10] = Self::opcode_lt;
        opcodes[0x11] = Self::opcode_gt;
        opcodes[0x12] = Self::opcode_slt;
        opcodes[0x13] = Self::opcode_sgt;
        opcodes[0x14] = Self::opcode_eq;
        opcodes[0x15] = Self::opcode_iszero;
        opcodes[0x16] = Self::opcode_and;
        opcodes[0x17] = Self::opcode_or;
        opcodes[0x18] = Self::opcode_xor;
        opcodes[0x19] = Self::opcode_not;
        opcodes[0x1A] = Self::opcode_byte;
        opcodes[0x1B] = Self::opcode_shl;
        opcodes[0x1C] = Self::opcode_shr;
        opcodes[0x1D] = Self::opcode_sar;

        opcodes[0x20] = Self::opcode_sha3;

        opcodes[0x30] = Self::opcode_address;
        opcodes[0x31] = Self::opcode_balance;
        opcodes[0x32] = Self::opcode_origin;
        opcodes[0x33] = Self::opcode_caller;
        opcodes[0x34] = Self::opcode_callvalue;
        opcodes[0x35] = Self::opcode_calldataload;
        opcodes[0x36] = Self::opcode_calldatasize;
        opcodes[0x37] = Self::opcode_calldatacopy;
        opcodes[0x38] = Self::opcode_codesize;
        opcodes[0x39] = Self::opcode_codecopy;
        opcodes[0x3A] = Self::opcode_gasprice;
        opcodes[0x3B] = Self::opcode_extcodesize;
        opcodes[0x3C] = Self::opcode_extcodecopy;
        opcodes[0x3D] = Self::opcode_returndatasize;
        opcodes[0x3E] = Self::opcode_returndatacopy;
        opcodes[0x3F] = Self::opcode_extcodehash;
        opcodes[0x40] = Self::opcode_blockhash;
        opcodes[0x41] = Self::opcode_coinbase;
        opcodes[0x42] = Self::opcode_timestamp;
        opcodes[0x43] = Self::opcode_number;
        opcodes[0x44] = Self::opcode_difficulty;
        opcodes[0x45] = Self::opcode_gaslimit;
        opcodes[0x46] = Self::opcode_chainid;
        opcodes[0x47] = Self::opcode_selfbalance;
        opcodes[0x48] = Self::opcode_basefee;

        opcodes[0x50] = Self::opcode_pop;
        opcodes[0x51] = Self::opcode_mload;
        opcodes[0x52] = Self::opcode_mstore;
        opcodes[0x53] = Self::opcode_mstore8;
        opcodes[0x54] = Self::opcode_sload;
        opcodes[0x55] = Self::opcode_sstore;
        opcodes[0x56] = Self::opcode_jump;
        opcodes[0x57] = Self::opcode_jumpi;
        opcodes[0x58] = Self::opcode_pc;
        opcodes[0x59] = Self::opcode_msize;
        opcodes[0x5A] = Self::opcode_gas;
        opcodes[0x5B] = Self::opcode_jumpdest;

        opcodes[0x60] = Self::opcode_push_1;
        opcodes[0x61] = Self::opcode_push_2_31::<2>;
        opcodes[0x62] = Self::opcode_push_2_31::<3>;
        opcodes[0x63] = Self::opcode_push_2_31::<4>;
        opcodes[0x64] = Self::opcode_push_2_31::<5>;
        opcodes[0x65] = Self::opcode_push_2_31::<6>;
        opcodes[0x66] = Self::opcode_push_2_31::<7>;
        opcodes[0x67] = Self::opcode_push_2_31::<8>;
        opcodes[0x68] = Self::opcode_push_2_31::<9>;
        opcodes[0x69] = Self::opcode_push_2_31::<10>;
        opcodes[0x6A] = Self::opcode_push_2_31::<11>;
        opcodes[0x6B] = Self::opcode_push_2_31::<12>;
        opcodes[0x6C] = Self::opcode_push_2_31::<13>;
        opcodes[0x6D] = Self::opcode_push_2_31::<14>;
        opcodes[0x6E] = Self::opcode_push_2_31::<15>;
        opcodes[0x6F] = Self::opcode_push_2_31::<16>;
        opcodes[0x70] = Self::opcode_push_2_31::<17>;
        opcodes[0x71] = Self::opcode_push_2_31::<18>;
        opcodes[0x72] = Self::opcode_push_2_31::<19>;
        opcodes[0x73] = Self::opcode_push_2_31::<20>;
        opcodes[0x74] = Self::opcode_push_2_31::<21>;
        opcodes[0x75] = Self::opcode_push_2_31::<22>;
        opcodes[0x76] = Self::opcode_push_2_31::<23>;
        opcodes[0x77] = Self::opcode_push_2_31::<24>;
        opcodes[0x78] = Self::opcode_push_2_31::<25>;
        opcodes[0x79] = Self::opcode_push_2_31::<26>;
        opcodes[0x7A] = Self::opcode_push_2_31::<27>;
        opcodes[0x7B] = Self::opcode_push_2_31::<28>;
        opcodes[0x7C] = Self::opcode_push_2_31::<29>;
        opcodes[0x7D] = Self::opcode_push_2_31::<30>;
        opcodes[0x7E] = Self::opcode_push_2_31::<31>;
        opcodes[0x7F] = Self::opcode_push_32;

        opcodes[0x80] = Self::opcode_dup_1_16::<1>;
        opcodes[0x81] = Self::opcode_dup_1_16::<2>;
        opcodes[0x82] = Self::opcode_dup_1_16::<3>;
        opcodes[0x83] = Self::opcode_dup_1_16::<4>;
        opcodes[0x84] = Self::opcode_dup_1_16::<5>;
        opcodes[0x85] = Self::opcode_dup_1_16::<6>;
        opcodes[0x86] = Self::opcode_dup_1_16::<7>;
        opcodes[0x87] = Self::opcode_dup_1_16::<8>;
        opcodes[0x88] = Self::opcode_dup_1_16::<9>;
        opcodes[0x89] = Self::opcode_dup_1_16::<10>;
        opcodes[0x8A] = Self::opcode_dup_1_16::<11>;
        opcodes[0x8B] = Self::opcode_dup_1_16::<12>;
        opcodes[0x8C] = Self::opcode_dup_1_16::<13>;
        opcodes[0x8D] = Self::opcode_dup_1_16::<14>;
        opcodes[0x8E] = Self::opcode_dup_1_16::<15>;
        opcodes[0x8F] = Self::opcode_dup_1_16::<16>;

        opcodes[0x90] = Self::opcode_swap_1_16::<1>;
        opcodes[0x91] = Self::opcode_swap_1_16::<2>;
        opcodes[0x92] = Self::opcode_swap_1_16::<3>;
        opcodes[0x93] = Self::opcode_swap_1_16::<4>;
        opcodes[0x94] = Self::opcode_swap_1_16::<5>;
        opcodes[0x95] = Self::opcode_swap_1_16::<6>;
        opcodes[0x96] = Self::opcode_swap_1_16::<7>;
        opcodes[0x97] = Self::opcode_swap_1_16::<8>;
        opcodes[0x98] = Self::opcode_swap_1_16::<9>;
        opcodes[0x99] = Self::opcode_swap_1_16::<10>;
        opcodes[0x9A] = Self::opcode_swap_1_16::<11>;
        opcodes[0x9B] = Self::opcode_swap_1_16::<12>;
        opcodes[0x9C] = Self::opcode_swap_1_16::<13>;
        opcodes[0x9D] = Self::opcode_swap_1_16::<14>;
        opcodes[0x9E] = Self::opcode_swap_1_16::<15>;
        opcodes[0x9F] = Self::opcode_swap_1_16::<16>;

        opcodes[0xA0] = Self::opcode_log_0_4::<0>;
        opcodes[0xA1] = Self::opcode_log_0_4::<1>;
        opcodes[0xA2] = Self::opcode_log_0_4::<2>;
        opcodes[0xA3] = Self::opcode_log_0_4::<3>;
        opcodes[0xA4] = Self::opcode_log_0_4::<4>;

        opcodes[0xF0] = Self::opcode_create;
        opcodes[0xF1] = Self::opcode_call;
        opcodes[0xF2] = Self::opcode_callcode;
        opcodes[0xF3] = Self::opcode_return;
        opcodes[0xF4] = Self::opcode_delegatecall;
        opcodes[0xF5] = Self::opcode_create2;

        opcodes[0xFA] = Self::opcode_staticcall;

        opcodes[0xFD] = Self::opcode_revert;
        opcodes[0xFE] = Self::opcode_invalid;

        opcodes[0xFF] = Self::opcode_selfdestruct;

        opcodes
    };
}