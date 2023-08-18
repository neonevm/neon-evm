#![allow(clippy::type_complexity)]

use crate::error::{Error, Result};
use std::convert::TryFrom;

use super::eof::Container;
use super::{database::Database, opcode::Action, Machine};

#[allow(clippy::enum_glob_use)]
use OpCode::*;

#[allow(clippy::upper_case_acronyms)]
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum OpCode {
    STOP = 0x00,
    ADD = 0x01,
    MUL = 0x02,
    SUB = 0x03,
    DIV = 0x04,
    SDIV = 0x05,
    MOD = 0x06,
    SMOD = 0x07,
    ADDMOD = 0x08,
    MULMOD = 0x09,
    EXP = 0x0A,
    SIGNEXTEND = 0x0B,

    LT = 0x10,
    GT = 0x11,
    SLT = 0x12,
    SGT = 0x13,
    EQ = 0x14,
    ISZERO = 0x15,
    AND = 0x16,
    OR = 0x17,
    XOR = 0x18,
    NOT = 0x19,
    BYTE = 0x1A,
    SHL = 0x1B,
    SHR = 0x1C,
    SAR = 0x1D,

    KECCAK256 = 0x20,

    ADDRESS = 0x30,
    BALANCE = 0x31,
    ORIGIN = 0x32,
    CALLER = 0x33,
    CALLVALUE = 0x34,
    CALLDATALOAD = 0x35,
    CALLDATASIZE = 0x36,
    CALLDATACOPY = 0x37,
    CODESIZE = 0x38,
    CODECOPY = 0x39,
    GASPRICE = 0x3A,
    EXTCODESIZE = 0x3B,
    EXTCODECOPY = 0x3C,
    RETURNDATASIZE = 0x3D,
    RETURNDATACOPY = 0x3E,
    EXTCODEHASH = 0x3F,

    BLOCKHASH = 0x40,
    COINBASE = 0x41,
    TIMESTAMP = 0x42,
    NUMBER = 0x43,
    DIFFICULTY = 0x44,
    GASLIMIT = 0x45,
    CHAINID = 0x46,
    SELFBALANCE = 0x47,
    BASEFEE = 0x48,

    POP = 0x50,
    MLOAD = 0x51,
    MSTORE = 0x52,
    MSTORE8 = 0x53,
    SLOAD = 0x54,
    SSTORE = 0x55,
    JUMP = 0x56,
    JUMPI = 0x57,
    PC = 0x58,
    MSIZE = 0x59,
    GAS = 0x5A,
    JUMPDEST = 0x5B,
    RJUMP = 0x5C,
    RJUMPI = 0x5D,
    RJUMPV = 0x5E,

    PUSH0 = 0x5F,
    PUSH1 = 0x60,
    PUSH2 = 0x61,
    PUSH3 = 0x62,
    PUSH4 = 0x63,
    PUSH5 = 0x64,
    PUSH6 = 0x65,
    PUSH7 = 0x66,
    PUSH8 = 0x67,
    PUSH9 = 0x68,
    PUSH10 = 0x69,
    PUSH11 = 0x6A,
    PUSH12 = 0x6B,
    PUSH13 = 0x6C,
    PUSH14 = 0x6D,
    PUSH15 = 0x6E,
    PUSH16 = 0x6F,
    PUSH17 = 0x70,
    PUSH18 = 0x71,
    PUSH19 = 0x72,
    PUSH20 = 0x73,
    PUSH21 = 0x74,
    PUSH22 = 0x75,
    PUSH23 = 0x76,
    PUSH24 = 0x77,
    PUSH25 = 0x78,
    PUSH26 = 0x79,
    PUSH27 = 0x7A,
    PUSH28 = 0x7B,
    PUSH29 = 0x7C,
    PUSH30 = 0x7D,
    PUSH31 = 0x7E,
    PUSH32 = 0x7F,

    DUP1 = 0x80,
    DUP2 = 0x81,
    DUP3 = 0x82,
    DUP4 = 0x83,
    DUP5 = 0x84,
    DUP6 = 0x85,
    DUP7 = 0x86,
    DUP8 = 0x87,
    DUP9 = 0x88,
    DUP10 = 0x89,
    DUP11 = 0x8A,
    DUP12 = 0x8B,
    DUP13 = 0x8C,
    DUP14 = 0x8D,
    DUP15 = 0x8E,
    DUP16 = 0x8F,

    SWAP1 = 0x90,
    SWAP2 = 0x91,
    SWAP3 = 0x92,
    SWAP4 = 0x93,
    SWAP5 = 0x94,
    SWAP6 = 0x95,
    SWAP7 = 0x96,
    SWAP8 = 0x97,
    SWAP9 = 0x98,
    SWAP10 = 0x99,
    SWAP11 = 0x9A,
    SWAP12 = 0x9B,
    SWAP13 = 0x9C,
    SWAP14 = 0x9D,
    SWAP15 = 0x9E,
    SWAP16 = 0x9F,

    LOG0 = 0xA0,
    LOG1 = 0xA1,
    LOG2 = 0xA2,
    LOG3 = 0xA3,
    LOG4 = 0xA4,

    CALLF = 0xB0,
    RETF = 0xB1,

    CREATE = 0xF0,
    CALL = 0xF1,
    CALLCODE = 0xF2,
    RETURN = 0xF3,
    DELEGATECALL = 0xF4,
    CREATE2 = 0xF5,

    STATICCALL = 0xFA,
    REVERT = 0xFD,
    INVALID = 0xFE,
    SELFDESTRUCT = 0xFF,

    TLOAD = 0xB3,
    TSTORE = 0xB4,
}

#[derive(Debug)]
pub struct OpcodeInfo {
    pub min_stack: usize,
    pub max_stack: usize,
    pub terminal: bool,
}

impl TryFrom<u8> for OpCode {
    type Error = Error;

    #[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            op if op == STOP as u8 => Ok(STOP),
            op if op == ADD as u8 => Ok(ADD),
            op if op == MUL as u8 => Ok(MUL),
            op if op == SUB as u8 => Ok(SUB),
            op if op == DIV as u8 => Ok(DIV),
            op if op == SDIV as u8 => Ok(SDIV),
            op if op == MOD as u8 => Ok(MOD),
            op if op == SMOD as u8 => Ok(SMOD),
            op if op == ADDMOD as u8 => Ok(ADDMOD),
            op if op == MULMOD as u8 => Ok(MULMOD),
            op if op == EXP as u8 => Ok(EXP),
            op if op == SIGNEXTEND as u8 => Ok(SIGNEXTEND),
            op if op == LT as u8 => Ok(LT),
            op if op == GT as u8 => Ok(GT),
            op if op == SLT as u8 => Ok(SLT),
            op if op == SGT as u8 => Ok(SGT),
            op if op == EQ as u8 => Ok(EQ),
            op if op == ISZERO as u8 => Ok(ISZERO),
            op if op == AND as u8 => Ok(AND),
            op if op == OR as u8 => Ok(OR),
            op if op == XOR as u8 => Ok(XOR),
            op if op == NOT as u8 => Ok(NOT),
            op if op == BYTE as u8 => Ok(BYTE),
            op if op == SHL as u8 => Ok(SHL),
            op if op == SHR as u8 => Ok(SHR),
            op if op == SAR as u8 => Ok(SAR),
            op if op == KECCAK256 as u8 => Ok(KECCAK256),
            op if op == ADDRESS as u8 => Ok(ADDRESS),
            op if op == BALANCE as u8 => Ok(BALANCE),
            op if op == ORIGIN as u8 => Ok(ORIGIN),
            op if op == CALLER as u8 => Ok(CALLER),
            op if op == CALLVALUE as u8 => Ok(CALLVALUE),
            op if op == CALLDATALOAD as u8 => Ok(CALLDATALOAD),
            op if op == CALLDATASIZE as u8 => Ok(CALLDATASIZE),
            op if op == CALLDATACOPY as u8 => Ok(CALLDATACOPY),
            op if op == CODESIZE as u8 => Ok(CODESIZE),
            op if op == CODECOPY as u8 => Ok(CODECOPY),
            op if op == GASPRICE as u8 => Ok(GASPRICE),
            op if op == EXTCODESIZE as u8 => Ok(EXTCODESIZE),
            op if op == EXTCODECOPY as u8 => Ok(EXTCODECOPY),
            op if op == RETURNDATASIZE as u8 => Ok(RETURNDATASIZE),
            op if op == RETURNDATACOPY as u8 => Ok(RETURNDATACOPY),
            op if op == EXTCODEHASH as u8 => Ok(EXTCODEHASH),
            op if op == BLOCKHASH as u8 => Ok(BLOCKHASH),
            op if op == COINBASE as u8 => Ok(COINBASE),
            op if op == TIMESTAMP as u8 => Ok(TIMESTAMP),
            op if op == NUMBER as u8 => Ok(NUMBER),
            op if op == DIFFICULTY as u8 => Ok(DIFFICULTY),
            op if op == GASLIMIT as u8 => Ok(GASLIMIT),
            op if op == CHAINID as u8 => Ok(CHAINID),
            op if op == SELFBALANCE as u8 => Ok(SELFBALANCE),
            op if op == BASEFEE as u8 => Ok(BASEFEE),
            op if op == POP as u8 => Ok(POP),
            op if op == MLOAD as u8 => Ok(MLOAD),
            op if op == MSTORE as u8 => Ok(MSTORE),
            op if op == MSTORE8 as u8 => Ok(MSTORE8),
            op if op == SLOAD as u8 => Ok(SLOAD),
            op if op == SSTORE as u8 => Ok(SSTORE),
            op if op == JUMP as u8 => Ok(JUMP),
            op if op == JUMPI as u8 => Ok(JUMPI),
            op if op == PC as u8 => Ok(PC),
            op if op == MSIZE as u8 => Ok(MSIZE),
            op if op == GAS as u8 => Ok(GAS),
            op if op == JUMPDEST as u8 => Ok(JUMPDEST),
            op if op == RJUMP as u8 => Ok(RJUMP),
            op if op == RJUMPI as u8 => Ok(RJUMPI),
            op if op == RJUMPV as u8 => Ok(RJUMPV),
            op if op == PUSH0 as u8 => Ok(PUSH0),
            op if op == PUSH1 as u8 => Ok(PUSH1),
            op if op == PUSH2 as u8 => Ok(PUSH2),
            op if op == PUSH3 as u8 => Ok(PUSH3),
            op if op == PUSH4 as u8 => Ok(PUSH4),
            op if op == PUSH5 as u8 => Ok(PUSH5),
            op if op == PUSH6 as u8 => Ok(PUSH6),
            op if op == PUSH7 as u8 => Ok(PUSH7),
            op if op == PUSH8 as u8 => Ok(PUSH8),
            op if op == PUSH9 as u8 => Ok(PUSH9),
            op if op == PUSH10 as u8 => Ok(PUSH10),
            op if op == PUSH11 as u8 => Ok(PUSH11),
            op if op == PUSH12 as u8 => Ok(PUSH12),
            op if op == PUSH13 as u8 => Ok(PUSH13),
            op if op == PUSH14 as u8 => Ok(PUSH14),
            op if op == PUSH15 as u8 => Ok(PUSH15),
            op if op == PUSH16 as u8 => Ok(PUSH16),
            op if op == PUSH17 as u8 => Ok(PUSH17),
            op if op == PUSH18 as u8 => Ok(PUSH18),
            op if op == PUSH19 as u8 => Ok(PUSH19),
            op if op == PUSH20 as u8 => Ok(PUSH20),
            op if op == PUSH21 as u8 => Ok(PUSH21),
            op if op == PUSH22 as u8 => Ok(PUSH22),
            op if op == PUSH23 as u8 => Ok(PUSH23),
            op if op == PUSH24 as u8 => Ok(PUSH24),
            op if op == PUSH25 as u8 => Ok(PUSH25),
            op if op == PUSH26 as u8 => Ok(PUSH26),
            op if op == PUSH27 as u8 => Ok(PUSH27),
            op if op == PUSH28 as u8 => Ok(PUSH28),
            op if op == PUSH29 as u8 => Ok(PUSH29),
            op if op == PUSH30 as u8 => Ok(PUSH30),
            op if op == PUSH31 as u8 => Ok(PUSH31),
            op if op == PUSH32 as u8 => Ok(PUSH32),
            op if op == DUP1 as u8 => Ok(DUP1),
            op if op == DUP2 as u8 => Ok(DUP2),
            op if op == DUP3 as u8 => Ok(DUP3),
            op if op == DUP4 as u8 => Ok(DUP4),
            op if op == DUP5 as u8 => Ok(DUP5),
            op if op == DUP6 as u8 => Ok(DUP6),
            op if op == DUP7 as u8 => Ok(DUP7),
            op if op == DUP8 as u8 => Ok(DUP8),
            op if op == DUP9 as u8 => Ok(DUP9),
            op if op == DUP10 as u8 => Ok(DUP10),
            op if op == DUP11 as u8 => Ok(DUP11),
            op if op == DUP12 as u8 => Ok(DUP12),
            op if op == DUP13 as u8 => Ok(DUP13),
            op if op == DUP14 as u8 => Ok(DUP14),
            op if op == DUP15 as u8 => Ok(DUP15),
            op if op == DUP16 as u8 => Ok(DUP16),
            op if op == SWAP1 as u8 => Ok(SWAP1),
            op if op == SWAP2 as u8 => Ok(SWAP2),
            op if op == SWAP3 as u8 => Ok(SWAP3),
            op if op == SWAP4 as u8 => Ok(SWAP4),
            op if op == SWAP5 as u8 => Ok(SWAP5),
            op if op == SWAP6 as u8 => Ok(SWAP6),
            op if op == SWAP7 as u8 => Ok(SWAP7),
            op if op == SWAP8 as u8 => Ok(SWAP8),
            op if op == SWAP9 as u8 => Ok(SWAP9),
            op if op == SWAP10 as u8 => Ok(SWAP10),
            op if op == SWAP11 as u8 => Ok(SWAP11),
            op if op == SWAP12 as u8 => Ok(SWAP12),
            op if op == SWAP13 as u8 => Ok(SWAP13),
            op if op == SWAP14 as u8 => Ok(SWAP14),
            op if op == SWAP15 as u8 => Ok(SWAP15),
            op if op == SWAP16 as u8 => Ok(SWAP16),
            op if op == LOG0 as u8 => Ok(LOG0),
            op if op == LOG1 as u8 => Ok(LOG1),
            op if op == LOG2 as u8 => Ok(LOG2),
            op if op == LOG3 as u8 => Ok(LOG3),
            op if op == LOG4 as u8 => Ok(LOG4),
            op if op == CALLF as u8 => Ok(CALLF),
            op if op == RETF as u8 => Ok(RETF),
            op if op == CREATE as u8 => Ok(CREATE),
            op if op == CALL as u8 => Ok(CALL),
            op if op == CALLCODE as u8 => Ok(CALLCODE),
            op if op == RETURN as u8 => Ok(RETURN),
            op if op == DELEGATECALL as u8 => Ok(DELEGATECALL),
            op if op == CREATE2 as u8 => Ok(CREATE2),
            op if op == STATICCALL as u8 => Ok(STATICCALL),
            op if op == REVERT as u8 => Ok(REVERT),
            op if op == INVALID as u8 => Ok(INVALID),
            op if op == SELFDESTRUCT as u8 => Ok(SELFDESTRUCT),
            op if op == TLOAD as u8 => Ok(TLOAD),
            op if op == TSTORE as u8 => Ok(TSTORE),
            op => Err(Error::UnsupportedOpcode(op)),
        }
    }
}

impl OpCode {
    pub fn has_opcode(op: u8) -> bool {
        OpCode::try_from(op).is_ok()
    }

    pub fn is_terminal_opcode(op: u8) -> bool {
        match OpCode::try_from(op) {
            Ok(opcode) => Self::opcode_info(opcode).terminal,
            _ => false,
        }
    }

    pub const fn u8(self) -> u8 {
        self as u8
    }

    const fn max_stack(pop: usize, push: usize) -> usize {
        Container::STACK_LIMIT + pop - push
    }

    const fn min_stack(pops: usize, _push: usize) -> usize {
        pops
    }

    const fn min_swap_stack(n: usize) -> usize {
        Self::min_stack(n, n)
    }

    const fn max_swap_stack(n: usize) -> usize {
        Self::max_stack(n, n)
    }

    const fn min_dup_stack(n: usize) -> usize {
        Self::min_stack(n, n + 1)
    }

    const fn max_dup_stack(n: usize) -> usize {
        Self::max_stack(n, n + 1)
    }

    const fn create_dup_opcode_info(n: usize) -> OpcodeInfo {
        OpcodeInfo {
            min_stack: Self::min_dup_stack(n),
            max_stack: Self::max_dup_stack(n),
            terminal: false,
        }
    }

    const fn create_swap_opcode_info(n: usize) -> OpcodeInfo {
        OpcodeInfo {
            min_stack: Self::min_swap_stack(n),
            max_stack: Self::max_swap_stack(n),
            terminal: false,
        }
    }

    const fn create_log_opcode_info(n: usize) -> OpcodeInfo {
        OpcodeInfo {
            min_stack: Self::min_stack(n, 0),
            max_stack: Self::max_stack(n, 0),
            terminal: false,
        }
    }

    #[allow(clippy::match_same_arms, clippy::too_many_lines)]
    pub const fn opcode_info(op: OpCode) -> OpcodeInfo {
        match op {
            STOP => OpcodeInfo {
                min_stack: Self::min_stack(0, 0),
                max_stack: Self::max_stack(0, 0),
                terminal: true,
            },
            ADD => OpcodeInfo {
                min_stack: Self::min_stack(2, 1),
                max_stack: Self::max_stack(2, 1),
                terminal: false,
            },
            MUL => OpcodeInfo {
                min_stack: Self::min_stack(2, 1),
                max_stack: Self::max_stack(2, 1),
                terminal: false,
            },
            SUB => OpcodeInfo {
                min_stack: Self::min_stack(2, 1),
                max_stack: Self::max_stack(2, 1),
                terminal: false,
            },
            DIV => OpcodeInfo {
                min_stack: Self::min_stack(2, 1),
                max_stack: Self::max_stack(2, 1),
                terminal: false,
            },
            SDIV => OpcodeInfo {
                min_stack: Self::min_stack(2, 1),
                max_stack: Self::max_stack(2, 1),
                terminal: false,
            },
            MOD => OpcodeInfo {
                min_stack: Self::min_stack(2, 1),
                max_stack: Self::max_stack(2, 1),
                terminal: false,
            },
            SMOD => OpcodeInfo {
                min_stack: Self::min_stack(2, 1),
                max_stack: Self::max_stack(2, 1),
                terminal: false,
            },
            ADDMOD => OpcodeInfo {
                min_stack: Self::min_stack(3, 1),
                max_stack: Self::max_stack(3, 1),
                terminal: false,
            },
            MULMOD => OpcodeInfo {
                min_stack: Self::min_stack(3, 1),
                max_stack: Self::max_stack(3, 1),
                terminal: false,
            },
            EXP => OpcodeInfo {
                min_stack: Self::min_stack(2, 1),
                max_stack: Self::max_stack(2, 1),
                terminal: false,
            },
            SIGNEXTEND => OpcodeInfo {
                min_stack: Self::min_stack(2, 1),
                max_stack: Self::max_stack(2, 1),
                terminal: false,
            },
            LT => OpcodeInfo {
                min_stack: Self::min_stack(2, 1),
                max_stack: Self::max_stack(2, 1),
                terminal: false,
            },
            GT => OpcodeInfo {
                min_stack: Self::min_stack(2, 1),
                max_stack: Self::max_stack(2, 1),
                terminal: false,
            },
            SLT => OpcodeInfo {
                min_stack: Self::min_stack(2, 1),
                max_stack: Self::max_stack(2, 1),
                terminal: false,
            },
            SGT => OpcodeInfo {
                min_stack: Self::min_stack(2, 1),
                max_stack: Self::max_stack(2, 1),
                terminal: false,
            },
            EQ => OpcodeInfo {
                min_stack: Self::min_stack(2, 1),
                max_stack: Self::max_stack(2, 1),
                terminal: false,
            },
            ISZERO => OpcodeInfo {
                min_stack: Self::min_stack(1, 1),
                max_stack: Self::max_stack(1, 1),
                terminal: false,
            },
            AND => OpcodeInfo {
                min_stack: Self::min_stack(2, 1),
                max_stack: Self::max_stack(2, 1),
                terminal: false,
            },
            XOR => OpcodeInfo {
                min_stack: Self::min_stack(2, 1),
                max_stack: Self::max_stack(2, 1),
                terminal: false,
            },
            OR => OpcodeInfo {
                min_stack: Self::min_stack(2, 1),
                max_stack: Self::max_stack(2, 1),
                terminal: false,
            },
            NOT => OpcodeInfo {
                min_stack: Self::min_stack(1, 1),
                max_stack: Self::max_stack(1, 1),
                terminal: false,
            },
            BYTE => OpcodeInfo {
                min_stack: Self::min_stack(2, 1),
                max_stack: Self::max_stack(2, 1),
                terminal: false,
            },
            KECCAK256 => OpcodeInfo {
                min_stack: Self::min_stack(2, 1),
                max_stack: Self::max_stack(2, 1),
                terminal: false,
            },
            ADDRESS => OpcodeInfo {
                min_stack: Self::min_stack(0, 1),
                max_stack: Self::max_stack(0, 1),
                terminal: false,
            },
            BALANCE => OpcodeInfo {
                min_stack: Self::min_stack(1, 1),
                max_stack: Self::max_stack(1, 1),
                terminal: false,
            },
            ORIGIN => OpcodeInfo {
                min_stack: Self::min_stack(0, 1),
                max_stack: Self::max_stack(0, 1),
                terminal: false,
            },
            CALLER => OpcodeInfo {
                min_stack: Self::min_stack(0, 1),
                max_stack: Self::max_stack(0, 1),
                terminal: false,
            },
            CALLVALUE => OpcodeInfo {
                min_stack: Self::min_stack(0, 1),
                max_stack: Self::max_stack(0, 1),
                terminal: false,
            },
            CALLDATALOAD => OpcodeInfo {
                min_stack: Self::min_stack(1, 1),
                max_stack: Self::max_stack(1, 1),
                terminal: false,
            },
            CALLDATASIZE => OpcodeInfo {
                min_stack: Self::min_stack(0, 1),
                max_stack: Self::max_stack(0, 1),
                terminal: false,
            },
            CALLDATACOPY => OpcodeInfo {
                min_stack: Self::min_stack(3, 0),
                max_stack: Self::max_stack(3, 0),
                terminal: false,
            },
            CODESIZE => OpcodeInfo {
                min_stack: Self::min_stack(0, 1),
                max_stack: Self::max_stack(0, 1),
                terminal: false,
            },
            CODECOPY => OpcodeInfo {
                min_stack: Self::min_stack(3, 0),
                max_stack: Self::max_stack(3, 0),
                terminal: false,
            },
            GASPRICE => OpcodeInfo {
                min_stack: Self::min_stack(0, 1),
                max_stack: Self::max_stack(0, 1),
                terminal: false,
            },
            EXTCODESIZE => OpcodeInfo {
                min_stack: Self::min_stack(1, 1),
                max_stack: Self::max_stack(1, 1),
                terminal: false,
            },
            EXTCODECOPY => OpcodeInfo {
                min_stack: Self::min_stack(4, 0),
                max_stack: Self::max_stack(4, 0),
                terminal: false,
            },
            BLOCKHASH => OpcodeInfo {
                min_stack: Self::min_stack(1, 1),
                max_stack: Self::max_stack(1, 1),
                terminal: false,
            },
            COINBASE => OpcodeInfo {
                min_stack: Self::min_stack(0, 1),
                max_stack: Self::max_stack(0, 1),
                terminal: false,
            },
            TIMESTAMP => OpcodeInfo {
                min_stack: Self::min_stack(0, 1),
                max_stack: Self::max_stack(0, 1),
                terminal: false,
            },
            NUMBER => OpcodeInfo {
                min_stack: Self::min_stack(0, 1),
                max_stack: Self::max_stack(0, 1),
                terminal: false,
            },
            DIFFICULTY => OpcodeInfo {
                min_stack: Self::min_stack(0, 1),
                max_stack: Self::max_stack(0, 1),
                terminal: false,
            },
            GASLIMIT => OpcodeInfo {
                min_stack: Self::min_stack(0, 1),
                max_stack: Self::max_stack(0, 1),
                terminal: false,
            },
            POP => OpcodeInfo {
                min_stack: Self::min_stack(1, 0),
                max_stack: Self::max_stack(1, 0),
                terminal: false,
            },
            MLOAD => OpcodeInfo {
                min_stack: Self::min_stack(1, 1),
                max_stack: Self::max_stack(1, 1),
                terminal: false,
            },
            MSTORE => OpcodeInfo {
                min_stack: Self::min_stack(2, 0),
                max_stack: Self::max_stack(2, 0),
                terminal: false,
            },
            MSTORE8 => OpcodeInfo {
                min_stack: Self::min_stack(2, 0),
                max_stack: Self::max_stack(2, 0),
                terminal: false,
            },
            SLOAD => OpcodeInfo {
                min_stack: Self::min_stack(1, 1),
                max_stack: Self::max_stack(1, 1),
                terminal: false,
            },
            SSTORE => OpcodeInfo {
                min_stack: Self::min_stack(2, 0),
                max_stack: Self::max_stack(2, 0),
                terminal: false,
            },
            JUMP => OpcodeInfo {
                min_stack: Self::min_stack(1, 0),
                max_stack: Self::max_stack(1, 0),
                terminal: false,
            },
            JUMPI => OpcodeInfo {
                min_stack: Self::min_stack(2, 0),
                max_stack: Self::max_stack(2, 0),
                terminal: false,
            },
            PC => OpcodeInfo {
                min_stack: Self::min_stack(0, 1),
                max_stack: Self::max_stack(0, 1),
                terminal: false,
            },
            MSIZE => OpcodeInfo {
                min_stack: Self::min_stack(0, 1),
                max_stack: Self::max_stack(0, 1),
                terminal: false,
            },
            GAS => OpcodeInfo {
                min_stack: Self::min_stack(0, 1),
                max_stack: Self::max_stack(0, 1),
                terminal: false,
            },
            JUMPDEST => OpcodeInfo {
                min_stack: Self::min_stack(0, 0),
                max_stack: Self::max_stack(0, 0),
                terminal: false,
            },
            PUSH1 | PUSH2 | PUSH3 | PUSH4 | PUSH5 | PUSH6 | PUSH7 | PUSH8 | PUSH9 | PUSH10
            | PUSH11 | PUSH12 | PUSH13 | PUSH14 | PUSH15 | PUSH16 | PUSH17 | PUSH18 | PUSH19
            | PUSH20 | PUSH21 | PUSH22 | PUSH23 | PUSH24 | PUSH25 | PUSH26 | PUSH27 | PUSH28
            | PUSH29 | PUSH30 | PUSH31 | PUSH32 => OpcodeInfo {
                min_stack: Self::min_stack(0, 1),
                max_stack: Self::max_stack(0, 1),
                terminal: false,
            },
            DUP1 => Self::create_dup_opcode_info(1),
            DUP2 => Self::create_dup_opcode_info(2),
            DUP3 => Self::create_dup_opcode_info(3),
            DUP4 => Self::create_dup_opcode_info(4),
            DUP5 => Self::create_dup_opcode_info(5),
            DUP6 => Self::create_dup_opcode_info(6),
            DUP7 => Self::create_dup_opcode_info(7),
            DUP8 => Self::create_dup_opcode_info(8),
            DUP9 => Self::create_dup_opcode_info(9),
            DUP10 => Self::create_dup_opcode_info(10),
            DUP11 => Self::create_dup_opcode_info(11),
            DUP12 => Self::create_dup_opcode_info(12),
            DUP13 => Self::create_dup_opcode_info(13),
            DUP14 => Self::create_dup_opcode_info(14),
            DUP15 => Self::create_dup_opcode_info(15),
            DUP16 => Self::create_dup_opcode_info(16),
            SWAP1 => Self::create_swap_opcode_info(2),
            SWAP2 => Self::create_swap_opcode_info(3),
            SWAP3 => Self::create_swap_opcode_info(4),
            SWAP4 => Self::create_swap_opcode_info(5),
            SWAP5 => Self::create_swap_opcode_info(6),
            SWAP6 => Self::create_swap_opcode_info(7),
            SWAP7 => Self::create_swap_opcode_info(8),
            SWAP8 => Self::create_swap_opcode_info(9),
            SWAP9 => Self::create_swap_opcode_info(10),
            SWAP10 => Self::create_swap_opcode_info(11),
            SWAP11 => Self::create_swap_opcode_info(12),
            SWAP12 => Self::create_swap_opcode_info(13),
            SWAP13 => Self::create_swap_opcode_info(14),
            SWAP14 => Self::create_swap_opcode_info(15),
            SWAP15 => Self::create_swap_opcode_info(16),
            SWAP16 => Self::create_swap_opcode_info(17),
            LOG0 => Self::create_log_opcode_info(2),
            LOG1 => Self::create_log_opcode_info(3),
            LOG2 => Self::create_log_opcode_info(4),
            LOG3 => Self::create_log_opcode_info(5),
            LOG4 => Self::create_log_opcode_info(6),
            CREATE => OpcodeInfo {
                min_stack: Self::min_stack(3, 1),
                max_stack: Self::max_stack(3, 1),
                terminal: false,
            },
            CALL => OpcodeInfo {
                min_stack: Self::min_stack(7, 1),
                max_stack: Self::max_stack(7, 1),
                terminal: false,
            },
            CALLCODE => OpcodeInfo {
                min_stack: Self::min_stack(7, 1),
                max_stack: Self::max_stack(7, 1),
                terminal: false,
            },
            RETURN => OpcodeInfo {
                min_stack: Self::min_stack(2, 0),
                max_stack: Self::max_stack(2, 0),
                terminal: true,
            },
            SELFDESTRUCT => OpcodeInfo {
                min_stack: Self::min_stack(1, 0),
                max_stack: Self::max_stack(1, 0),
                terminal: false,
            },
            INVALID => OpcodeInfo {
                min_stack: Self::min_stack(0, 0),
                max_stack: Self::max_stack(0, 0),
                terminal: true,
            },
            SHL => OpcodeInfo {
                min_stack: Self::min_stack(2, 1),
                max_stack: Self::max_stack(2, 1),
                terminal: false,
            },
            SHR => OpcodeInfo {
                min_stack: Self::min_stack(2, 1),
                max_stack: Self::max_stack(2, 1),
                terminal: false,
            },
            SAR => OpcodeInfo {
                min_stack: Self::min_stack(2, 1),
                max_stack: Self::max_stack(2, 1),
                terminal: false,
            },
            RETURNDATASIZE => OpcodeInfo {
                min_stack: Self::min_stack(0, 1),
                max_stack: Self::max_stack(0, 1),
                terminal: false,
            },
            RETURNDATACOPY => OpcodeInfo {
                min_stack: Self::min_stack(3, 0),
                max_stack: Self::max_stack(3, 0),
                terminal: false,
            },
            EXTCODEHASH => OpcodeInfo {
                min_stack: Self::min_stack(1, 1),
                max_stack: Self::max_stack(1, 1),
                terminal: false,
            },
            CHAINID => OpcodeInfo {
                min_stack: Self::min_stack(0, 1),
                max_stack: Self::max_stack(0, 1),
                terminal: false,
            },
            SELFBALANCE => OpcodeInfo {
                min_stack: Self::min_stack(0, 1),
                max_stack: Self::max_stack(0, 1),
                terminal: false,
            },
            BASEFEE => OpcodeInfo {
                min_stack: Self::min_stack(0, 1),
                max_stack: Self::max_stack(0, 1),
                terminal: false,
            },
            RJUMP => OpcodeInfo {
                min_stack: Self::min_stack(0, 0),
                max_stack: Self::max_stack(0, 0),
                terminal: true,
            },
            RJUMPI => OpcodeInfo {
                min_stack: Self::min_stack(1, 0),
                max_stack: Self::max_stack(1, 0),
                terminal: false,
            },
            RJUMPV => OpcodeInfo {
                min_stack: Self::min_stack(1, 0),
                max_stack: Self::max_stack(1, 0),
                terminal: false,
            },
            PUSH0 => OpcodeInfo {
                min_stack: Self::min_stack(0, 1),
                max_stack: Self::max_stack(0, 1),
                terminal: false,
            },
            CALLF => OpcodeInfo {
                min_stack: Self::min_stack(0, 0),
                max_stack: Self::max_stack(0, 0),
                terminal: false,
            },
            RETF => OpcodeInfo {
                min_stack: Self::min_stack(0, 0),
                max_stack: Self::max_stack(0, 0),
                terminal: true,
            },
            DELEGATECALL => OpcodeInfo {
                min_stack: Self::min_stack(6, 1),
                max_stack: Self::max_stack(6, 1),
                terminal: false,
            },
            CREATE2 => OpcodeInfo {
                min_stack: Self::min_stack(4, 1),
                max_stack: Self::max_stack(4, 1),
                terminal: false,
            },
            STATICCALL => OpcodeInfo {
                min_stack: Self::min_stack(6, 1),
                max_stack: Self::max_stack(6, 1),
                terminal: false,
            },
            REVERT => OpcodeInfo {
                min_stack: Self::min_stack(2, 0),
                max_stack: Self::max_stack(2, 0),
                terminal: true,
            },
            TLOAD => OpcodeInfo {
                min_stack: Self::min_stack(1, 1),
                max_stack: Self::max_stack(1, 1),
                terminal: false,
            },
            TSTORE => OpcodeInfo {
                min_stack: Self::min_stack(2, 0),
                max_stack: Self::max_stack(2, 0),
                terminal: false,
            },
        }
    }
}

impl<B: Database> Machine<B> {
    pub const OPCODES: [fn(&mut Self, &mut B) -> Result<Action>; 256] = {
        let mut opcodes: [fn(&mut Self, &mut B) -> Result<Action>; 256] =
            [Self::opcode_unknown; 256];

        opcodes[STOP as usize] = Self::opcode_stop;
        opcodes[ADD as usize] = Self::opcode_add;
        opcodes[MUL as usize] = Self::opcode_mul;
        opcodes[SUB as usize] = Self::opcode_sub;
        opcodes[DIV as usize] = Self::opcode_div;
        opcodes[SDIV as usize] = Self::opcode_sdiv;
        opcodes[MOD as usize] = Self::opcode_mod;
        opcodes[SMOD as usize] = Self::opcode_smod;
        opcodes[ADDMOD as usize] = Self::opcode_addmod;
        opcodes[MULMOD as usize] = Self::opcode_mulmod;
        opcodes[EXP as usize] = Self::opcode_exp;
        opcodes[SIGNEXTEND as usize] = Self::opcode_signextend;

        opcodes[LT as usize] = Self::opcode_lt;
        opcodes[GT as usize] = Self::opcode_gt;
        opcodes[SLT as usize] = Self::opcode_slt;
        opcodes[SGT as usize] = Self::opcode_sgt;
        opcodes[EQ as usize] = Self::opcode_eq;
        opcodes[ISZERO as usize] = Self::opcode_iszero;
        opcodes[AND as usize] = Self::opcode_and;
        opcodes[OR as usize] = Self::opcode_or;
        opcodes[XOR as usize] = Self::opcode_xor;
        opcodes[NOT as usize] = Self::opcode_not;
        opcodes[BYTE as usize] = Self::opcode_byte;
        opcodes[SHL as usize] = Self::opcode_shl;
        opcodes[SHR as usize] = Self::opcode_shr;
        opcodes[SAR as usize] = Self::opcode_sar;

        opcodes[KECCAK256 as usize] = Self::opcode_sha3;

        opcodes[ADDRESS as usize] = Self::opcode_address;
        opcodes[BALANCE as usize] = Self::opcode_balance;
        opcodes[ORIGIN as usize] = Self::opcode_origin;
        opcodes[CALLER as usize] = Self::opcode_caller;
        opcodes[CALLVALUE as usize] = Self::opcode_callvalue;
        opcodes[CALLDATALOAD as usize] = Self::opcode_calldataload;
        opcodes[CALLDATASIZE as usize] = Self::opcode_calldatasize;
        opcodes[CALLDATACOPY as usize] = Self::opcode_calldatacopy;
        opcodes[CODESIZE as usize] = Self::opcode_codesize;
        opcodes[CODECOPY as usize] = Self::opcode_codecopy;
        opcodes[GASPRICE as usize] = Self::opcode_gasprice;
        opcodes[EXTCODESIZE as usize] = Self::opcode_extcodesize;
        opcodes[EXTCODECOPY as usize] = Self::opcode_extcodecopy;
        opcodes[RETURNDATASIZE as usize] = Self::opcode_returndatasize;
        opcodes[RETURNDATACOPY as usize] = Self::opcode_returndatacopy;
        opcodes[EXTCODEHASH as usize] = Self::opcode_extcodehash;

        opcodes[BLOCKHASH as usize] = Self::opcode_blockhash;
        opcodes[COINBASE as usize] = Self::opcode_coinbase;
        opcodes[TIMESTAMP as usize] = Self::opcode_timestamp;
        opcodes[NUMBER as usize] = Self::opcode_number;
        opcodes[DIFFICULTY as usize] = Self::opcode_difficulty;
        opcodes[GASLIMIT as usize] = Self::opcode_gaslimit;
        opcodes[CHAINID as usize] = Self::opcode_chainid;
        opcodes[SELFBALANCE as usize] = Self::opcode_selfbalance;
        opcodes[BASEFEE as usize] = Self::opcode_basefee;

        opcodes[POP as usize] = Self::opcode_pop;
        opcodes[MLOAD as usize] = Self::opcode_mload;
        opcodes[MSTORE as usize] = Self::opcode_mstore;
        opcodes[MSTORE8 as usize] = Self::opcode_mstore8;
        opcodes[SLOAD as usize] = Self::opcode_sload;
        opcodes[SSTORE as usize] = Self::opcode_sstore;
        opcodes[JUMP as usize] = Self::opcode_jump;
        opcodes[JUMPI as usize] = Self::opcode_jumpi;
        opcodes[PC as usize] = Self::opcode_pc;
        opcodes[MSIZE as usize] = Self::opcode_msize;
        opcodes[GAS as usize] = Self::opcode_gas;
        opcodes[JUMPDEST as usize] = Self::opcode_jumpdest;

        opcodes[PUSH0 as usize] = Self::opcode_push_0;
        opcodes[PUSH1 as usize] = Self::opcode_push_1;
        opcodes[PUSH2 as usize] = Self::opcode_push_2_31::<2>;
        opcodes[PUSH3 as usize] = Self::opcode_push_2_31::<3>;
        opcodes[PUSH4 as usize] = Self::opcode_push_2_31::<4>;
        opcodes[PUSH5 as usize] = Self::opcode_push_2_31::<5>;
        opcodes[PUSH6 as usize] = Self::opcode_push_2_31::<6>;
        opcodes[PUSH7 as usize] = Self::opcode_push_2_31::<7>;
        opcodes[PUSH8 as usize] = Self::opcode_push_2_31::<8>;
        opcodes[PUSH9 as usize] = Self::opcode_push_2_31::<9>;
        opcodes[PUSH10 as usize] = Self::opcode_push_2_31::<10>;
        opcodes[PUSH11 as usize] = Self::opcode_push_2_31::<11>;
        opcodes[PUSH12 as usize] = Self::opcode_push_2_31::<12>;
        opcodes[PUSH13 as usize] = Self::opcode_push_2_31::<13>;
        opcodes[PUSH14 as usize] = Self::opcode_push_2_31::<14>;
        opcodes[PUSH15 as usize] = Self::opcode_push_2_31::<15>;
        opcodes[PUSH16 as usize] = Self::opcode_push_2_31::<16>;
        opcodes[PUSH17 as usize] = Self::opcode_push_2_31::<17>;
        opcodes[PUSH18 as usize] = Self::opcode_push_2_31::<18>;
        opcodes[PUSH19 as usize] = Self::opcode_push_2_31::<19>;
        opcodes[PUSH20 as usize] = Self::opcode_push_2_31::<20>;
        opcodes[PUSH21 as usize] = Self::opcode_push_2_31::<21>;
        opcodes[PUSH22 as usize] = Self::opcode_push_2_31::<22>;
        opcodes[PUSH23 as usize] = Self::opcode_push_2_31::<23>;
        opcodes[PUSH24 as usize] = Self::opcode_push_2_31::<24>;
        opcodes[PUSH25 as usize] = Self::opcode_push_2_31::<25>;
        opcodes[PUSH26 as usize] = Self::opcode_push_2_31::<26>;
        opcodes[PUSH27 as usize] = Self::opcode_push_2_31::<27>;
        opcodes[PUSH28 as usize] = Self::opcode_push_2_31::<28>;
        opcodes[PUSH29 as usize] = Self::opcode_push_2_31::<29>;
        opcodes[PUSH30 as usize] = Self::opcode_push_2_31::<30>;
        opcodes[PUSH31 as usize] = Self::opcode_push_2_31::<31>;
        opcodes[PUSH32 as usize] = Self::opcode_push_32;

        opcodes[DUP1 as usize] = Self::opcode_dup_1_16::<1>;
        opcodes[DUP2 as usize] = Self::opcode_dup_1_16::<2>;
        opcodes[DUP3 as usize] = Self::opcode_dup_1_16::<3>;
        opcodes[DUP4 as usize] = Self::opcode_dup_1_16::<4>;
        opcodes[DUP5 as usize] = Self::opcode_dup_1_16::<5>;
        opcodes[DUP6 as usize] = Self::opcode_dup_1_16::<6>;
        opcodes[DUP7 as usize] = Self::opcode_dup_1_16::<7>;
        opcodes[DUP8 as usize] = Self::opcode_dup_1_16::<8>;
        opcodes[DUP9 as usize] = Self::opcode_dup_1_16::<9>;
        opcodes[DUP10 as usize] = Self::opcode_dup_1_16::<10>;
        opcodes[DUP11 as usize] = Self::opcode_dup_1_16::<11>;
        opcodes[DUP12 as usize] = Self::opcode_dup_1_16::<12>;
        opcodes[DUP13 as usize] = Self::opcode_dup_1_16::<13>;
        opcodes[DUP14 as usize] = Self::opcode_dup_1_16::<14>;
        opcodes[DUP15 as usize] = Self::opcode_dup_1_16::<15>;
        opcodes[DUP16 as usize] = Self::opcode_dup_1_16::<16>;

        opcodes[SWAP1 as usize] = Self::opcode_swap_1_16::<1>;
        opcodes[SWAP2 as usize] = Self::opcode_swap_1_16::<2>;
        opcodes[SWAP3 as usize] = Self::opcode_swap_1_16::<3>;
        opcodes[SWAP4 as usize] = Self::opcode_swap_1_16::<4>;
        opcodes[SWAP5 as usize] = Self::opcode_swap_1_16::<5>;
        opcodes[SWAP6 as usize] = Self::opcode_swap_1_16::<6>;
        opcodes[SWAP7 as usize] = Self::opcode_swap_1_16::<7>;
        opcodes[SWAP8 as usize] = Self::opcode_swap_1_16::<8>;
        opcodes[SWAP9 as usize] = Self::opcode_swap_1_16::<9>;
        opcodes[SWAP10 as usize] = Self::opcode_swap_1_16::<10>;
        opcodes[SWAP11 as usize] = Self::opcode_swap_1_16::<11>;
        opcodes[SWAP12 as usize] = Self::opcode_swap_1_16::<12>;
        opcodes[SWAP13 as usize] = Self::opcode_swap_1_16::<13>;
        opcodes[SWAP14 as usize] = Self::opcode_swap_1_16::<14>;
        opcodes[SWAP15 as usize] = Self::opcode_swap_1_16::<15>;
        opcodes[SWAP16 as usize] = Self::opcode_swap_1_16::<16>;

        opcodes[LOG0 as usize] = Self::opcode_log_0_4::<0>;
        opcodes[LOG1 as usize] = Self::opcode_log_0_4::<1>;
        opcodes[LOG2 as usize] = Self::opcode_log_0_4::<2>;
        opcodes[LOG3 as usize] = Self::opcode_log_0_4::<3>;
        opcodes[LOG4 as usize] = Self::opcode_log_0_4::<4>;

        opcodes[CREATE as usize] = Self::opcode_create;
        opcodes[CALL as usize] = Self::opcode_call;
        opcodes[CALLCODE as usize] = Self::opcode_callcode;
        opcodes[RETURN as usize] = Self::opcode_return;
        opcodes[DELEGATECALL as usize] = Self::opcode_delegatecall;
        opcodes[CREATE2 as usize] = Self::opcode_create2;

        opcodes[STATICCALL as usize] = Self::opcode_staticcall;

        opcodes[REVERT as usize] = Self::opcode_revert;
        opcodes[INVALID as usize] = Self::opcode_invalid;

        opcodes[SELFDESTRUCT as usize] = Self::opcode_selfdestruct;

        opcodes
    };

    pub const EOF_OPCODES: [fn(&mut Self, &mut B) -> Result<Action>; 256] = {
        let mut opcodes: [fn(&mut Self, &mut B) -> Result<Action>; 256] =
            [Self::opcode_unknown; 256];

        let mut i: usize = 0;
        while i < 256 {
            opcodes[i] = Self::OPCODES[i];
            i += 1;
        }

        // EOF opcodes
        opcodes[RJUMP as usize] = Self::opcode_rjump;
        opcodes[RJUMPI as usize] = Self::opcode_rjumpi;
        opcodes[RJUMPV as usize] = Self::opcode_rjumpv;
        opcodes[CALLF as usize] = Self::opcode_callf;
        opcodes[RETF as usize] = Self::opcode_retf;

        // Deprecated opcodes
        let mut i = 0;
        while i < Container::DEPRECATED_OPCODES.len() {
            opcodes[Container::DEPRECATED_OPCODES[i] as usize] = Self::opcode_deprecated;
            i += 1;
        }

        opcodes
    };
}

#[cfg(test)]
mod tests {
    use crate::evm::opcode_table::OpCode;

    #[test]
    fn test() {
        assert!(OpCode::has_opcode(0x00));
        assert!(OpCode::has_opcode(0x20));
        assert_eq!(OpCode::has_opcode(0x21), false);
    }
}
