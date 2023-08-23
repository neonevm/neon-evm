#![allow(clippy::type_complexity, clippy::inline_always)]

use super::eof::Container;
use super::{database::Database, opcode::Action, Machine};
use crate::error::{Error, Result};

pub mod opcode {
    #![allow(dead_code)]

    pub const STOP: u8 = 0x00;
    pub const ADD: u8 = 0x01;
    pub const MUL: u8 = 0x02;
    pub const SUB: u8 = 0x03;
    pub const DIV: u8 = 0x04;
    pub const SDIV: u8 = 0x05;
    pub const MOD: u8 = 0x06;
    pub const SMOD: u8 = 0x07;
    pub const ADDMOD: u8 = 0x08;
    pub const MULMOD: u8 = 0x09;
    pub const EXP: u8 = 0x0A;
    pub const SIGNEXTEND: u8 = 0x0B;

    pub const LT: u8 = 0x10;
    pub const GT: u8 = 0x11;
    pub const SLT: u8 = 0x12;
    pub const SGT: u8 = 0x13;
    pub const EQ: u8 = 0x14;
    pub const ISZERO: u8 = 0x15;
    pub const AND: u8 = 0x16;
    pub const OR: u8 = 0x17;
    pub const XOR: u8 = 0x18;
    pub const NOT: u8 = 0x19;
    pub const BYTE: u8 = 0x1A;
    pub const SHL: u8 = 0x1B;
    pub const SHR: u8 = 0x1C;
    pub const SAR: u8 = 0x1D;

    pub const KECCAK256: u8 = 0x20;

    pub const ADDRESS: u8 = 0x30;
    pub const BALANCE: u8 = 0x31;
    pub const ORIGIN: u8 = 0x32;
    pub const CALLER: u8 = 0x33;
    pub const CALLVALUE: u8 = 0x34;
    pub const CALLDATALOAD: u8 = 0x35;
    pub const CALLDATASIZE: u8 = 0x36;
    pub const CALLDATACOPY: u8 = 0x37;
    pub const CODESIZE: u8 = 0x38;
    pub const CODECOPY: u8 = 0x39;
    pub const GASPRICE: u8 = 0x3A;
    pub const EXTCODESIZE: u8 = 0x3B;
    pub const EXTCODECOPY: u8 = 0x3C;
    pub const RETURNDATASIZE: u8 = 0x3D;
    pub const RETURNDATACOPY: u8 = 0x3E;
    pub const EXTCODEHASH: u8 = 0x3F;

    pub const BLOCKHASH: u8 = 0x40;
    pub const COINBASE: u8 = 0x41;
    pub const TIMESTAMP: u8 = 0x42;
    pub const NUMBER: u8 = 0x43;
    pub const DIFFICULTY: u8 = 0x44;
    pub const GASLIMIT: u8 = 0x45;
    pub const CHAINID: u8 = 0x46;
    pub const SELFBALANCE: u8 = 0x47;
    pub const BASEFEE: u8 = 0x48;

    pub const POP: u8 = 0x50;
    pub const MLOAD: u8 = 0x51;
    pub const MSTORE: u8 = 0x52;
    pub const MSTORE8: u8 = 0x53;
    pub const SLOAD: u8 = 0x54;
    pub const SSTORE: u8 = 0x55;
    pub const JUMP: u8 = 0x56;
    pub const JUMPI: u8 = 0x57;
    pub const PC: u8 = 0x58;
    pub const MSIZE: u8 = 0x59;
    pub const GAS: u8 = 0x5A;
    pub const JUMPDEST: u8 = 0x5B;
    pub const RJUMP: u8 = 0x5C;
    pub const RJUMPI: u8 = 0x5D;
    pub const RJUMPV: u8 = 0x5E;

    pub const PUSH0: u8 = 0x5F;
    pub const PUSH1: u8 = 0x60;
    pub const PUSH2: u8 = 0x61;
    pub const PUSH3: u8 = 0x62;
    pub const PUSH4: u8 = 0x63;
    pub const PUSH5: u8 = 0x64;
    pub const PUSH6: u8 = 0x65;
    pub const PUSH7: u8 = 0x66;
    pub const PUSH8: u8 = 0x67;
    pub const PUSH9: u8 = 0x68;
    pub const PUSH10: u8 = 0x69;
    pub const PUSH11: u8 = 0x6A;
    pub const PUSH12: u8 = 0x6B;
    pub const PUSH13: u8 = 0x6C;
    pub const PUSH14: u8 = 0x6D;
    pub const PUSH15: u8 = 0x6E;
    pub const PUSH16: u8 = 0x6F;
    pub const PUSH17: u8 = 0x70;
    pub const PUSH18: u8 = 0x71;
    pub const PUSH19: u8 = 0x72;
    pub const PUSH20: u8 = 0x73;
    pub const PUSH21: u8 = 0x74;
    pub const PUSH22: u8 = 0x75;
    pub const PUSH23: u8 = 0x76;
    pub const PUSH24: u8 = 0x77;
    pub const PUSH25: u8 = 0x78;
    pub const PUSH26: u8 = 0x79;
    pub const PUSH27: u8 = 0x7A;
    pub const PUSH28: u8 = 0x7B;
    pub const PUSH29: u8 = 0x7C;
    pub const PUSH30: u8 = 0x7D;
    pub const PUSH31: u8 = 0x7E;
    pub const PUSH32: u8 = 0x7F;

    pub const DUP1: u8 = 0x80;
    pub const DUP2: u8 = 0x81;
    pub const DUP3: u8 = 0x82;
    pub const DUP4: u8 = 0x83;
    pub const DUP5: u8 = 0x84;
    pub const DUP6: u8 = 0x85;
    pub const DUP7: u8 = 0x86;
    pub const DUP8: u8 = 0x87;
    pub const DUP9: u8 = 0x88;
    pub const DUP10: u8 = 0x89;
    pub const DUP11: u8 = 0x8A;
    pub const DUP12: u8 = 0x8B;
    pub const DUP13: u8 = 0x8C;
    pub const DUP14: u8 = 0x8D;
    pub const DUP15: u8 = 0x8E;
    pub const DUP16: u8 = 0x8F;

    pub const SWAP1: u8 = 0x90;
    pub const SWAP2: u8 = 0x91;
    pub const SWAP3: u8 = 0x92;
    pub const SWAP4: u8 = 0x93;
    pub const SWAP5: u8 = 0x94;
    pub const SWAP6: u8 = 0x95;
    pub const SWAP7: u8 = 0x96;
    pub const SWAP8: u8 = 0x97;
    pub const SWAP9: u8 = 0x98;
    pub const SWAP10: u8 = 0x99;
    pub const SWAP11: u8 = 0x9A;
    pub const SWAP12: u8 = 0x9B;
    pub const SWAP13: u8 = 0x9C;
    pub const SWAP14: u8 = 0x9D;
    pub const SWAP15: u8 = 0x9E;
    pub const SWAP16: u8 = 0x9F;

    pub const LOG0: u8 = 0xA0;
    pub const LOG1: u8 = 0xA1;
    pub const LOG2: u8 = 0xA2;
    pub const LOG3: u8 = 0xA3;
    pub const LOG4: u8 = 0xA4;

    pub const CALLF: u8 = 0xB0;
    pub const RETF: u8 = 0xB1;

    pub const CREATE: u8 = 0xF0;
    pub const CALL: u8 = 0xF1;
    pub const CALLCODE: u8 = 0xF2;
    pub const RETURN: u8 = 0xF3;
    pub const DELEGATECALL: u8 = 0xF4;
    pub const CREATE2: u8 = 0xF5;

    pub const STATICCALL: u8 = 0xFA;
    pub const REVERT: u8 = 0xFD;
    pub const INVALID: u8 = 0xFE;
    pub const SELFDESTRUCT: u8 = 0xFF;

    pub const TLOAD: u8 = 0xB3;
    pub const TSTORE: u8 = 0xB4;
}

#[allow(clippy::wildcard_imports)]
use opcode::*;

#[derive(Debug, Clone, Copy)]
pub struct OpcodeInfo {
    pub min_stack: usize,
    pub max_stack: usize,
    pub terminal: bool,
}

impl OpcodeInfo {
    #[inline]
    #[must_use]
    pub fn is_opcode_valid(op: u8) -> bool {
        Self::OPCODE_INFO[op as usize].is_some()
    }

    pub fn assert_opcode_valid(op: u8) -> Result<()> {
        if Self::OPCODE_INFO[op as usize].is_none() {
            return Err(Error::UnsupportedOpcode(op));
        }

        Ok(())
    }

    pub fn is_terminal_opcode(op: u8) -> bool {
        Self::OPCODE_INFO[op as usize].map_or(false, |info| info.terminal)
    }

    #[inline(always)]
    const fn max_stack(pop: usize, push: usize) -> usize {
        Container::STACK_LIMIT + pop - push
    }

    #[inline(always)]
    const fn min_stack(pops: usize, _push: usize) -> usize {
        pops
    }

    #[inline(always)]
    const fn min_swap_stack(n: usize) -> usize {
        OpcodeInfo::min_stack(n, n)
    }

    #[inline(always)]
    const fn max_swap_stack(n: usize) -> usize {
        OpcodeInfo::max_stack(n, n)
    }

    #[inline(always)]
    const fn min_dup_stack(n: usize) -> usize {
        OpcodeInfo::min_stack(n, n + 1)
    }

    #[inline(always)]
    const fn max_dup_stack(n: usize) -> usize {
        OpcodeInfo::max_stack(n, n + 1)
    }

    #[inline(always)]
    const fn create_dup_opcode_info(n: usize) -> OpcodeInfo {
        OpcodeInfo {
            min_stack: Self::min_dup_stack(n),
            max_stack: Self::max_dup_stack(n),
            terminal: false,
        }
    }

    #[inline(always)]
    const fn create_swap_opcode_info(n: usize) -> OpcodeInfo {
        OpcodeInfo {
            min_stack: Self::min_swap_stack(n),
            max_stack: Self::max_swap_stack(n),
            terminal: false,
        }
    }

    #[inline(always)]
    const fn create_log_opcode_info(n: usize) -> OpcodeInfo {
        OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(n, 0),
            max_stack: OpcodeInfo::max_stack(n, 0),
            terminal: false,
        }
    }

    pub const OPCODE_INFO: [Option<OpcodeInfo>; 256] = {
        let mut opcodes: [Option<OpcodeInfo>; 256] = [None; 256];

        opcodes[STOP as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(0, 0),
            max_stack: OpcodeInfo::max_stack(0, 0),
            terminal: true,
        });
        opcodes[ADD as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 1),
            max_stack: OpcodeInfo::max_stack(2, 1),
            terminal: false,
        });
        opcodes[MUL as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 1),
            max_stack: OpcodeInfo::max_stack(2, 1),
            terminal: false,
        });
        opcodes[SUB as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 1),
            max_stack: OpcodeInfo::max_stack(2, 1),
            terminal: false,
        });
        opcodes[DIV as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 1),
            max_stack: OpcodeInfo::max_stack(2, 1),
            terminal: false,
        });
        opcodes[SDIV as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 1),
            max_stack: OpcodeInfo::max_stack(2, 1),
            terminal: false,
        });
        opcodes[MOD as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 1),
            max_stack: OpcodeInfo::max_stack(2, 1),
            terminal: false,
        });
        opcodes[SMOD as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 1),
            max_stack: OpcodeInfo::max_stack(2, 1),
            terminal: false,
        });
        opcodes[ADDMOD as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(3, 1),
            max_stack: OpcodeInfo::max_stack(3, 1),
            terminal: false,
        });
        opcodes[MULMOD as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(3, 1),
            max_stack: OpcodeInfo::max_stack(3, 1),
            terminal: false,
        });
        opcodes[EXP as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 1),
            max_stack: OpcodeInfo::max_stack(2, 1),
            terminal: false,
        });
        opcodes[SIGNEXTEND as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 1),
            max_stack: OpcodeInfo::max_stack(2, 1),
            terminal: false,
        });

        opcodes[LT as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 1),
            max_stack: OpcodeInfo::max_stack(2, 1),
            terminal: false,
        });
        opcodes[GT as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 1),
            max_stack: OpcodeInfo::max_stack(2, 1),
            terminal: false,
        });
        opcodes[SLT as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 1),
            max_stack: OpcodeInfo::max_stack(2, 1),
            terminal: false,
        });
        opcodes[SGT as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 1),
            max_stack: OpcodeInfo::max_stack(2, 1),
            terminal: false,
        });
        opcodes[EQ as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 1),
            max_stack: OpcodeInfo::max_stack(2, 1),
            terminal: false,
        });
        opcodes[ISZERO as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(1, 1),
            max_stack: OpcodeInfo::max_stack(1, 1),
            terminal: false,
        });
        opcodes[AND as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 1),
            max_stack: OpcodeInfo::max_stack(2, 1),
            terminal: false,
        });
        opcodes[OR as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 1),
            max_stack: OpcodeInfo::max_stack(2, 1),
            terminal: false,
        });
        opcodes[XOR as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 1),
            max_stack: OpcodeInfo::max_stack(2, 1),
            terminal: false,
        });
        opcodes[NOT as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(1, 1),
            max_stack: OpcodeInfo::max_stack(1, 1),
            terminal: false,
        });
        opcodes[BYTE as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 1),
            max_stack: OpcodeInfo::max_stack(2, 1),
            terminal: false,
        });

        opcodes[SHL as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 1),
            max_stack: OpcodeInfo::max_stack(2, 1),
            terminal: false,
        });
        opcodes[SHR as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 1),
            max_stack: OpcodeInfo::max_stack(2, 1),
            terminal: false,
        });
        opcodes[SAR as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 1),
            max_stack: OpcodeInfo::max_stack(2, 1),
            terminal: false,
        });

        opcodes[KECCAK256 as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 1),
            max_stack: OpcodeInfo::max_stack(2, 1),
            terminal: false,
        });

        opcodes[ADDRESS as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(0, 1),
            max_stack: OpcodeInfo::max_stack(0, 1),
            terminal: false,
        });
        opcodes[BALANCE as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(1, 1),
            max_stack: OpcodeInfo::max_stack(1, 1),
            terminal: false,
        });
        opcodes[ORIGIN as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(0, 1),
            max_stack: OpcodeInfo::max_stack(0, 1),
            terminal: false,
        });
        opcodes[CALLER as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(0, 1),
            max_stack: OpcodeInfo::max_stack(0, 1),
            terminal: false,
        });
        opcodes[CALLVALUE as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(0, 1),
            max_stack: OpcodeInfo::max_stack(0, 1),
            terminal: false,
        });
        opcodes[CALLDATALOAD as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(1, 1),
            max_stack: OpcodeInfo::max_stack(1, 1),
            terminal: false,
        });
        opcodes[CALLDATASIZE as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(0, 1),
            max_stack: OpcodeInfo::max_stack(0, 1),
            terminal: false,
        });
        opcodes[CALLDATACOPY as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(3, 0),
            max_stack: OpcodeInfo::max_stack(3, 0),
            terminal: false,
        });
        opcodes[CODESIZE as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(0, 1),
            max_stack: OpcodeInfo::max_stack(0, 1),
            terminal: false,
        });
        opcodes[CODECOPY as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(3, 0),
            max_stack: OpcodeInfo::max_stack(3, 0),
            terminal: false,
        });
        opcodes[GASPRICE as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(0, 1),
            max_stack: OpcodeInfo::max_stack(0, 1),
            terminal: false,
        });
        opcodes[EXTCODESIZE as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(1, 1),
            max_stack: OpcodeInfo::max_stack(1, 1),
            terminal: false,
        });
        opcodes[EXTCODECOPY as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(4, 0),
            max_stack: OpcodeInfo::max_stack(4, 0),
            terminal: false,
        });
        opcodes[RETURNDATASIZE as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(0, 1),
            max_stack: OpcodeInfo::max_stack(0, 1),
            terminal: false,
        });
        opcodes[RETURNDATACOPY as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(3, 0),
            max_stack: OpcodeInfo::max_stack(3, 0),
            terminal: false,
        });
        opcodes[EXTCODEHASH as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(1, 1),
            max_stack: OpcodeInfo::max_stack(1, 1),
            terminal: false,
        });

        opcodes[BLOCKHASH as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(1, 1),
            max_stack: OpcodeInfo::max_stack(1, 1),
            terminal: false,
        });
        opcodes[COINBASE as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(0, 1),
            max_stack: OpcodeInfo::max_stack(0, 1),
            terminal: false,
        });
        opcodes[TIMESTAMP as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(0, 1),
            max_stack: OpcodeInfo::max_stack(0, 1),
            terminal: false,
        });
        opcodes[NUMBER as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(0, 1),
            max_stack: OpcodeInfo::max_stack(0, 1),
            terminal: false,
        });
        opcodes[DIFFICULTY as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(0, 1),
            max_stack: OpcodeInfo::max_stack(0, 1),
            terminal: false,
        });
        opcodes[GASLIMIT as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(0, 1),
            max_stack: OpcodeInfo::max_stack(0, 1),
            terminal: false,
        });
        opcodes[CHAINID as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(0, 1),
            max_stack: OpcodeInfo::max_stack(0, 1),
            terminal: false,
        });
        opcodes[SELFBALANCE as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(0, 1),
            max_stack: OpcodeInfo::max_stack(0, 1),
            terminal: false,
        });
        opcodes[BASEFEE as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(0, 1),
            max_stack: OpcodeInfo::max_stack(0, 1),
            terminal: false,
        });

        opcodes[POP as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(1, 0),
            max_stack: OpcodeInfo::max_stack(1, 0),
            terminal: false,
        });
        opcodes[MLOAD as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(1, 1),
            max_stack: OpcodeInfo::max_stack(1, 1),
            terminal: false,
        });
        opcodes[MSTORE as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 0),
            max_stack: OpcodeInfo::max_stack(2, 0),
            terminal: false,
        });
        opcodes[MSTORE8 as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 0),
            max_stack: OpcodeInfo::max_stack(2, 0),
            terminal: false,
        });
        opcodes[SLOAD as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(1, 1),
            max_stack: OpcodeInfo::max_stack(1, 1),
            terminal: false,
        });
        opcodes[SSTORE as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 0),
            max_stack: OpcodeInfo::max_stack(2, 0),
            terminal: false,
        });
        opcodes[JUMP as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(1, 0),
            max_stack: OpcodeInfo::max_stack(1, 0),
            terminal: false,
        });
        opcodes[JUMPI as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 0),
            max_stack: OpcodeInfo::max_stack(2, 0),
            terminal: false,
        });
        opcodes[PC as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(0, 1),
            max_stack: OpcodeInfo::max_stack(0, 1),
            terminal: false,
        });
        opcodes[MSIZE as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(0, 1),
            max_stack: OpcodeInfo::max_stack(0, 1),
            terminal: false,
        });
        opcodes[GAS as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(0, 1),
            max_stack: OpcodeInfo::max_stack(0, 1),
            terminal: false,
        });
        opcodes[JUMPDEST as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(0, 0),
            max_stack: OpcodeInfo::max_stack(0, 0),
            terminal: false,
        });

        let push_info = OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(0, 1),
            max_stack: OpcodeInfo::max_stack(0, 1),
            terminal: false,
        };

        opcodes[PUSH0 as usize] = Some(push_info);
        opcodes[PUSH1 as usize] = Some(push_info);
        opcodes[PUSH2 as usize] = Some(push_info);
        opcodes[PUSH3 as usize] = Some(push_info);
        opcodes[PUSH4 as usize] = Some(push_info);
        opcodes[PUSH5 as usize] = Some(push_info);
        opcodes[PUSH6 as usize] = Some(push_info);
        opcodes[PUSH7 as usize] = Some(push_info);
        opcodes[PUSH8 as usize] = Some(push_info);
        opcodes[PUSH9 as usize] = Some(push_info);
        opcodes[PUSH10 as usize] = Some(push_info);
        opcodes[PUSH11 as usize] = Some(push_info);
        opcodes[PUSH12 as usize] = Some(push_info);
        opcodes[PUSH13 as usize] = Some(push_info);
        opcodes[PUSH14 as usize] = Some(push_info);
        opcodes[PUSH15 as usize] = Some(push_info);
        opcodes[PUSH16 as usize] = Some(push_info);
        opcodes[PUSH17 as usize] = Some(push_info);
        opcodes[PUSH18 as usize] = Some(push_info);
        opcodes[PUSH19 as usize] = Some(push_info);
        opcodes[PUSH20 as usize] = Some(push_info);
        opcodes[PUSH21 as usize] = Some(push_info);
        opcodes[PUSH22 as usize] = Some(push_info);
        opcodes[PUSH23 as usize] = Some(push_info);
        opcodes[PUSH24 as usize] = Some(push_info);
        opcodes[PUSH25 as usize] = Some(push_info);
        opcodes[PUSH26 as usize] = Some(push_info);
        opcodes[PUSH27 as usize] = Some(push_info);
        opcodes[PUSH28 as usize] = Some(push_info);
        opcodes[PUSH29 as usize] = Some(push_info);
        opcodes[PUSH30 as usize] = Some(push_info);
        opcodes[PUSH31 as usize] = Some(push_info);
        opcodes[PUSH32 as usize] = Some(push_info);

        opcodes[DUP1 as usize] = Some(OpcodeInfo::create_dup_opcode_info(1));
        opcodes[DUP2 as usize] = Some(OpcodeInfo::create_dup_opcode_info(2));
        opcodes[DUP3 as usize] = Some(OpcodeInfo::create_dup_opcode_info(3));
        opcodes[DUP4 as usize] = Some(OpcodeInfo::create_dup_opcode_info(4));
        opcodes[DUP5 as usize] = Some(OpcodeInfo::create_dup_opcode_info(5));
        opcodes[DUP6 as usize] = Some(OpcodeInfo::create_dup_opcode_info(6));
        opcodes[DUP7 as usize] = Some(OpcodeInfo::create_dup_opcode_info(7));
        opcodes[DUP8 as usize] = Some(OpcodeInfo::create_dup_opcode_info(8));
        opcodes[DUP9 as usize] = Some(OpcodeInfo::create_dup_opcode_info(9));
        opcodes[DUP10 as usize] = Some(OpcodeInfo::create_dup_opcode_info(10));
        opcodes[DUP11 as usize] = Some(OpcodeInfo::create_dup_opcode_info(11));
        opcodes[DUP12 as usize] = Some(OpcodeInfo::create_dup_opcode_info(12));
        opcodes[DUP13 as usize] = Some(OpcodeInfo::create_dup_opcode_info(13));
        opcodes[DUP14 as usize] = Some(OpcodeInfo::create_dup_opcode_info(14));
        opcodes[DUP15 as usize] = Some(OpcodeInfo::create_dup_opcode_info(15));
        opcodes[DUP16 as usize] = Some(OpcodeInfo::create_dup_opcode_info(16));

        opcodes[SWAP1 as usize] = Some(OpcodeInfo::create_swap_opcode_info(2));
        opcodes[SWAP2 as usize] = Some(OpcodeInfo::create_swap_opcode_info(3));
        opcodes[SWAP3 as usize] = Some(OpcodeInfo::create_swap_opcode_info(4));
        opcodes[SWAP4 as usize] = Some(OpcodeInfo::create_swap_opcode_info(5));
        opcodes[SWAP5 as usize] = Some(OpcodeInfo::create_swap_opcode_info(6));
        opcodes[SWAP6 as usize] = Some(OpcodeInfo::create_swap_opcode_info(7));
        opcodes[SWAP7 as usize] = Some(OpcodeInfo::create_swap_opcode_info(8));
        opcodes[SWAP8 as usize] = Some(OpcodeInfo::create_swap_opcode_info(9));
        opcodes[SWAP9 as usize] = Some(OpcodeInfo::create_swap_opcode_info(10));
        opcodes[SWAP10 as usize] = Some(OpcodeInfo::create_swap_opcode_info(11));
        opcodes[SWAP11 as usize] = Some(OpcodeInfo::create_swap_opcode_info(12));
        opcodes[SWAP12 as usize] = Some(OpcodeInfo::create_swap_opcode_info(13));
        opcodes[SWAP13 as usize] = Some(OpcodeInfo::create_swap_opcode_info(14));
        opcodes[SWAP14 as usize] = Some(OpcodeInfo::create_swap_opcode_info(15));
        opcodes[SWAP15 as usize] = Some(OpcodeInfo::create_swap_opcode_info(16));
        opcodes[SWAP16 as usize] = Some(OpcodeInfo::create_swap_opcode_info(17));

        opcodes[LOG0 as usize] = Some(OpcodeInfo::create_log_opcode_info(2));
        opcodes[LOG1 as usize] = Some(OpcodeInfo::create_log_opcode_info(3));
        opcodes[LOG2 as usize] = Some(OpcodeInfo::create_log_opcode_info(4));
        opcodes[LOG3 as usize] = Some(OpcodeInfo::create_log_opcode_info(5));
        opcodes[LOG4 as usize] = Some(OpcodeInfo::create_log_opcode_info(6));

        opcodes[CREATE as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(3, 1),
            max_stack: OpcodeInfo::max_stack(3, 1),
            terminal: false,
        });
        opcodes[CALL as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(7, 1),
            max_stack: OpcodeInfo::max_stack(7, 1),
            terminal: false,
        });
        opcodes[CALLCODE as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(7, 1),
            max_stack: OpcodeInfo::max_stack(7, 1),
            terminal: false,
        });
        opcodes[RETURN as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 0),
            max_stack: OpcodeInfo::max_stack(2, 0),
            terminal: true,
        });
        opcodes[DELEGATECALL as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(6, 1),
            max_stack: OpcodeInfo::max_stack(6, 1),
            terminal: false,
        });
        opcodes[CREATE2 as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(4, 1),
            max_stack: OpcodeInfo::max_stack(4, 1),
            terminal: false,
        });

        opcodes[STATICCALL as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(6, 1),
            max_stack: OpcodeInfo::max_stack(6, 1),
            terminal: false,
        });

        opcodes[REVERT as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(2, 0),
            max_stack: OpcodeInfo::max_stack(2, 0),
            terminal: true,
        });
        opcodes[INVALID as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(0, 0),
            max_stack: OpcodeInfo::max_stack(0, 0),
            terminal: true,
        });

        opcodes[SELFDESTRUCT as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(1, 0),
            max_stack: OpcodeInfo::max_stack(1, 0),
            terminal: false,
        });

        opcodes[RJUMP as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(0, 0),
            max_stack: OpcodeInfo::max_stack(0, 0),
            terminal: true,
        });
        opcodes[RJUMPI as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(1, 0),
            max_stack: OpcodeInfo::max_stack(1, 0),
            terminal: false,
        });
        opcodes[RJUMPV as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(1, 0),
            max_stack: OpcodeInfo::max_stack(1, 0),
            terminal: false,
        });

        opcodes[CALLF as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(0, 0),
            max_stack: OpcodeInfo::max_stack(0, 0),
            terminal: false,
        });

        opcodes[RETF as usize] = Some(OpcodeInfo {
            min_stack: OpcodeInfo::min_stack(0, 0),
            max_stack: OpcodeInfo::max_stack(0, 0),
            terminal: true,
        });

        opcodes
    };
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
    use crate::evm::opcode_table::OpcodeInfo;

    #[test]
    fn test() {
        assert!(OpcodeInfo::is_opcode_valid(0x00));
        assert!(OpcodeInfo::is_opcode_valid(0x20));
        assert_eq!(OpcodeInfo::is_opcode_valid(0x21), false);
    }
}
