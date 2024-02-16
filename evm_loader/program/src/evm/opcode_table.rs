use crate::error::Result;
use crate::evm::tracing::EventListener;

use super::{database::Database, opcode::Action, Machine};

macro_rules! opcode_table {
    ($( $opcode:literal, $opname:literal, $op:path;)*) => {
        #[cfg(target_os = "solana")]
        type OpCode<B, T> = fn(&mut Machine<B, T>, &mut B) -> Result<Action>;

        #[cfg(target_os = "solana")]
        impl<B: Database, T: EventListener> Machine<B, T> {
            const OPCODES: [OpCode<B, T>; 256] = {
                let mut opcodes: [OpCode<B, T>; 256] = [Self::opcode_unknown; 256];

                $(opcodes[$opcode as usize] = $op;)*

                opcodes
            };

            pub fn execute_opcode(&mut self, backend: &mut B, opcode: u8) -> Result<Action> {
                // SAFETY: OPCODES.len() == 256, opcode <= 255
                let opcode_fn = unsafe { Self::OPCODES.get_unchecked(opcode as usize) };
                opcode_fn(self, backend)
            }
        }

        #[cfg(not(target_os = "solana"))]
        impl<B: Database, T: EventListener> Machine<B, T> {
            pub async fn execute_opcode(&mut self, backend: &mut B, opcode: u8) -> Result<Action> {
                match opcode {
                    $($opcode => $op(self, backend).await,)*
                    _ => Self::opcode_unknown(self, backend).await,
                }
            }
        }

        #[cfg(not(target_os = "solana"))]
        pub const OPNAMES: [&str; 256] = {
            let mut opnames: [&str; 256] = ["<invalid>"; 256];

            $(opnames[$opcode as usize] = $opname;)*

            opnames
        };
    }
}

opcode_table![
        0x00, "STOP", Self::opcode_stop;
        0x01, "ADD", Self::opcode_add;
        0x02, "MUL", Self::opcode_mul;
        0x03, "SUB", Self::opcode_sub;
        0x04, "DIV", Self::opcode_div;
        0x05, "SDIV", Self::opcode_sdiv;
        0x06, "MOD", Self::opcode_mod;
        0x07, "SMOD", Self::opcode_smod;
        0x08, "ADDMOD", Self::opcode_addmod;
        0x09, "MULMOD", Self::opcode_mulmod;
        0x0A, "EXP", Self::opcode_exp;
        0x0B, "SIGNEXTEND", Self::opcode_signextend;

        0x10, "LT", Self::opcode_lt;
        0x11, "GT", Self::opcode_gt;
        0x12, "SLT", Self::opcode_slt;
        0x13, "SGT", Self::opcode_sgt;
        0x14, "EQ", Self::opcode_eq;
        0x15, "ISZERO", Self::opcode_iszero;
        0x16, "AND", Self::opcode_and;
        0x17, "OR", Self::opcode_or;
        0x18, "XOR", Self::opcode_xor;
        0x19, "NOT", Self::opcode_not;
        0x1A, "BYTE", Self::opcode_byte;
        0x1B, "SHL", Self::opcode_shl;
        0x1C, "SHR", Self::opcode_shr;
        0x1D, "SAR", Self::opcode_sar;

        0x20, "KECCAK256", Self::opcode_sha3;

        0x30, "ADDRESS", Self::opcode_address;
        0x31, "BALANCE", Self::opcode_balance;
        0x32, "ORIGIN", Self::opcode_origin;
        0x33, "CALLER", Self::opcode_caller;
        0x34, "CALLVALUE", Self::opcode_callvalue;
        0x35, "CALLDATALOAD", Self::opcode_calldataload;
        0x36, "CALLDATASIZE", Self::opcode_calldatasize;
        0x37, "CALLDATACOPY", Self::opcode_calldatacopy;
        0x38, "CODESIZE", Self::opcode_codesize;
        0x39, "CODECOPY", Self::opcode_codecopy;
        0x3A, "GASPRICE", Self::opcode_gasprice;
        0x3B, "EXTCODESIZE", Self::opcode_extcodesize;
        0x3C, "EXTCODECOPY", Self::opcode_extcodecopy;
        0x3D, "RETURNDATASIZE", Self::opcode_returndatasize;
        0x3E, "RETURNDATACOPY", Self::opcode_returndatacopy;
        0x3F, "EXTCODEHASH", Self::opcode_extcodehash;
        0x40, "BLOCKHASH", Self::opcode_blockhash;
        0x41, "COINBASE", Self::opcode_coinbase;
        0x42, "TIMESTAMP", Self::opcode_timestamp;
        0x43, "NUMBER", Self::opcode_number;
        0x44, "PREVRANDAO", Self::opcode_difficulty;
        0x45, "GASLIMIT", Self::opcode_gaslimit;
        0x46, "CHAINID", Self::opcode_chainid;
        0x47, "SELFBALANCE", Self::opcode_selfbalance;
        0x48, "BASEFEE", Self::opcode_basefee;

        0x50, "POP", Self::opcode_pop;
        0x51, "MLOAD", Self::opcode_mload;
        0x52, "MSTORE", Self::opcode_mstore;
        0x53, "MSTORE8", Self::opcode_mstore8;
        0x54, "SLOAD", Self::opcode_sload;
        0x55, "SSTORE", Self::opcode_sstore;
        0x56, "JUMP", Self::opcode_jump;
        0x57, "JUMPI", Self::opcode_jumpi;
        0x58, "PC", Self::opcode_pc;
        0x59, "MSIZE", Self::opcode_msize;
        0x5A, "GAS", Self::opcode_gas;
        0x5B, "JUMPDEST", Self::opcode_jumpdest;

        0x5F, "PUSH0", Self::opcode_push_0;
        0x60, "PUSH1", Self::opcode_push_1;
        0x61, "PUSH2", Self::opcode_push_2_31::<2>;
        0x62, "PUSH3", Self::opcode_push_2_31::<3>;
        0x63, "PUSH4", Self::opcode_push_2_31::<4>;
        0x64, "PUSH5", Self::opcode_push_2_31::<5>;
        0x65, "PUSH6", Self::opcode_push_2_31::<6>;
        0x66, "PUSH7", Self::opcode_push_2_31::<7>;
        0x67, "PUSH8", Self::opcode_push_2_31::<8>;
        0x68, "PUSH9", Self::opcode_push_2_31::<9>;
        0x69, "PUSH10", Self::opcode_push_2_31::<10>;
        0x6A, "PUSH11", Self::opcode_push_2_31::<11>;
        0x6B, "PUSH12", Self::opcode_push_2_31::<12>;
        0x6C, "PUSH13", Self::opcode_push_2_31::<13>;
        0x6D, "PUSH14", Self::opcode_push_2_31::<14>;
        0x6E, "PUSH15", Self::opcode_push_2_31::<15>;
        0x6F, "PUSH16", Self::opcode_push_2_31::<16>;
        0x70, "PUSH17", Self::opcode_push_2_31::<17>;
        0x71, "PUSH18", Self::opcode_push_2_31::<18>;
        0x72, "PUSH19", Self::opcode_push_2_31::<19>;
        0x73, "PUSH20", Self::opcode_push_2_31::<20>;
        0x74, "PUSH21", Self::opcode_push_2_31::<21>;
        0x75, "PUSH22", Self::opcode_push_2_31::<22>;
        0x76, "PUSH23", Self::opcode_push_2_31::<23>;
        0x77, "PUSH24", Self::opcode_push_2_31::<24>;
        0x78, "PUSH25", Self::opcode_push_2_31::<25>;
        0x79, "PUSH26", Self::opcode_push_2_31::<26>;
        0x7A, "PUSH27", Self::opcode_push_2_31::<27>;
        0x7B, "PUSH28", Self::opcode_push_2_31::<28>;
        0x7C, "PUSH29", Self::opcode_push_2_31::<29>;
        0x7D, "PUSH30", Self::opcode_push_2_31::<30>;
        0x7E, "PUSH31", Self::opcode_push_2_31::<31>;
        0x7F, "PUSH32", Self::opcode_push_32;

        0x80, "DUP1", Self::opcode_dup_1_16::<1>;
        0x81, "DUP2", Self::opcode_dup_1_16::<2>;
        0x82, "DUP3", Self::opcode_dup_1_16::<3>;
        0x83, "DUP4", Self::opcode_dup_1_16::<4>;
        0x84, "DUP5", Self::opcode_dup_1_16::<5>;
        0x85, "DUP6", Self::opcode_dup_1_16::<6>;
        0x86, "DUP7", Self::opcode_dup_1_16::<7>;
        0x87, "DUP8", Self::opcode_dup_1_16::<8>;
        0x88, "DUP9", Self::opcode_dup_1_16::<9>;
        0x89, "DUP10", Self::opcode_dup_1_16::<10>;
        0x8A, "DUP11", Self::opcode_dup_1_16::<11>;
        0x8B, "DUP12", Self::opcode_dup_1_16::<12>;
        0x8C, "DUP13", Self::opcode_dup_1_16::<13>;
        0x8D, "DUP14", Self::opcode_dup_1_16::<14>;
        0x8E, "DUP15", Self::opcode_dup_1_16::<15>;
        0x8F, "DUP16", Self::opcode_dup_1_16::<16>;

        0x90, "SWAP1", Self::opcode_swap_1_16::<1>;
        0x91, "SWAP2", Self::opcode_swap_1_16::<2>;
        0x92, "SWAP3", Self::opcode_swap_1_16::<3>;
        0x93, "SWAP4", Self::opcode_swap_1_16::<4>;
        0x94, "SWAP5", Self::opcode_swap_1_16::<5>;
        0x95, "SWAP6", Self::opcode_swap_1_16::<6>;
        0x96, "SWAP7", Self::opcode_swap_1_16::<7>;
        0x97, "SWAP8", Self::opcode_swap_1_16::<8>;
        0x98, "SWAP9", Self::opcode_swap_1_16::<9>;
        0x99, "SWAP10", Self::opcode_swap_1_16::<10>;
        0x9A, "SWAP11", Self::opcode_swap_1_16::<11>;
        0x9B, "SWAP12", Self::opcode_swap_1_16::<12>;
        0x9C, "SWAP13", Self::opcode_swap_1_16::<13>;
        0x9D, "SWAP14", Self::opcode_swap_1_16::<14>;
        0x9E, "SWAP15", Self::opcode_swap_1_16::<15>;
        0x9F, "SWAP16", Self::opcode_swap_1_16::<16>;

        0xA0, "LOG0", Self::opcode_log_0_4::<0>;
        0xA1, "LOG1", Self::opcode_log_0_4::<1>;
        0xA2, "LOG2", Self::opcode_log_0_4::<2>;
        0xA3, "LOG3", Self::opcode_log_0_4::<3>;
        0xA4, "LOG4", Self::opcode_log_0_4::<4>;

        0xF0, "CREATE", Self::opcode_create;
        0xF1, "CALL", Self::opcode_call;
        0xF2, "CALLCODE", Self::opcode_callcode;
        0xF3, "RETURN", Self::opcode_return;
        0xF4, "DELEGATECALL", Self::opcode_delegatecall;
        0xF5, "CREATE2", Self::opcode_create2;

        0xFA, "STATICCALL", Self::opcode_staticcall;

        0xFD, "REVERT", Self::opcode_revert;
        0xFE, "INVALID", Self::opcode_invalid;

        0xFF, "SELFDESTRUCT", Self::opcode_selfdestruct;
];
