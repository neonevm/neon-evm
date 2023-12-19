#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

use super::eof::Container;
use super::opcode_table::OpcodeInfo;
use crate::error::{Error, Result};
use crate::evm::analysis::Bitvec;
use crate::evm::eof::FunctionMetadata;

#[allow(clippy::wildcard_imports)]
use crate::evm::opcode_table::opcode::*;

use crate::evm::Buffer;

impl Container {
    /// [Specification](https://eips.ethereum.org/EIPS/eip-4750#:~:text=The%20return%20stack%20is%20limited%20to%20a%20maximum%201024%20items.)
    pub const STACK_LIMIT: usize = 1024;
    pub const LOC_UNVISITED: isize = -1;

    pub const DEPRECATED_OPCODES: [u8; 5] = [CALLCODE, SELFDESTRUCT, JUMP, JUMPI, PC];

    pub fn validate_container(&self) -> Result<()> {
        for (section, code) in self.code.iter().enumerate() {
            Self::validate_code(code, section, &self.types)?;
        }

        Ok(())
    }

    pub fn validate_code(
        code: &Buffer,
        section: usize,
        metadata: &Vec<FunctionMetadata>,
    ) -> Result<()> {
        let mut i: usize = 0;
        // Tracks the number of actual instructions in the code (e.g.
        // non-immediate values). This is used at the end to determine
        // if each instruction is reachable.
        let mut instruction_count: usize = 0;
        let mut analysis: Option<Bitvec> = None;
        let mut opcode: u8 = 0;

        // This loop visits every single instruction and verifies:
        // * if the instruction is valid for the given jump table.
        // * if the instruction has an immediate value, it is not truncated.
        // * if performing a relative jump, all jump destinations are valid.
        // * if changing code sections, the new code section index is valid and
        //   will not cause a stack overflow.
        while i < code.len() {
            instruction_count += 1;
            opcode = code.get_or_default(i);

            if !OpcodeInfo::is_opcode_valid(opcode) && Self::DEPRECATED_OPCODES.contains(&opcode) {
                return Err(Error::ValidationUndefinedInstruction(opcode, i));
            }

            if opcode > PUSH0 && opcode <= PUSH32 {
                let size = opcode - PUSH0;
                if code.len() <= i + size as usize {
                    return Err(Error::ValidationTruncatedImmediate(opcode, i));
                }
                i += size as usize;
            }

            if opcode == RJUMP || opcode == RJUMPI {
                if code.len() <= i + 2 {
                    return Err(Error::ValidationTruncatedImmediate(opcode, i));
                }
                analysis = Some(Self::check_dest(code, analysis, i + 1, i + 3, code.len())?);
                i += 2;
            }

            if opcode == RJUMPV {
                if code.len() <= i + 1 {
                    return Err(Error::ValidationTruncatedImmediate(opcode, i));
                }
                instruction_count = code.get_or_default(i + 1) as usize;
                if instruction_count == 0 {
                    return Err(Error::ValidationInvalidBranchCount(i));
                }
                if code.len() <= i + instruction_count {
                    return Err(Error::ValidationTruncatedImmediate(opcode, i));
                }
                for j in 0..instruction_count {
                    analysis = Some(Self::check_dest(
                        code,
                        analysis,
                        i + 2 + j * 2,
                        i + 2 * instruction_count + 2,
                        code.len(),
                    )?);
                }
                i += 1 + 2 * instruction_count;
            }

            if opcode == CALLF {
                if i + 2 >= code.len() {
                    return Err(Error::ValidationTruncatedImmediate(opcode, i));
                }
                let arg = code.get_u16_or_default(i + 1);

                if arg as usize >= metadata.len() {
                    return Err(Error::ValidationInvalidSectionArgument(
                        arg,
                        metadata.len(),
                        i,
                    ));
                }
                i += 2;
            }

            i += 1;
        }

        // Code sections may not "fall through" and require proper termination.
        // Therefore, the last instruction must be considered terminal.
        if !OpcodeInfo::is_terminal_opcode(opcode) {
            return Err(Error::ValidationInvalidCodeTermination(opcode, i));
        }

        let path = Self::validate_control_flow(code, section, metadata)?;
        if path != instruction_count {
            return Err(Error::ValidationUnreachableCode);
        }
        Ok(())
    }

    /// checkDest parses a relative offset at code[0:2] and checks if it is a valid jump destination.
    fn check_dest(
        code: &Buffer,
        analysis_option: Option<Bitvec>,
        imm: usize,
        from: usize,
        length: usize,
    ) -> Result<Bitvec> {
        if code.len() < imm + 2 {
            return Err(Error::UnexpectedEndOfFile);
        }
        let analysis = analysis_option.map_or_else(|| Bitvec::eof_code_bitmap(code), |a| a);
        let offset = code.get_i16_or_default(imm);
        let dest = (from as isize + offset as isize) as usize;
        if dest >= length {
            return Err(Error::ValidationInvalidJumpDest(offset, dest, imm));
        }
        if !analysis.is_code_segment(dest) {
            return Err(Error::ValidationInvalidJumpDest(offset, dest, imm));
        }
        Ok(analysis)
    }

    /// `validate_control_flow` iterates through all possible branches the provided code
    /// value and determines if it is valid per EOF v1.
    #[allow(clippy::too_many_lines)]
    fn validate_control_flow(
        code: &Buffer,
        section: usize,
        metadata: &[FunctionMetadata],
    ) -> Result<usize> {
        struct Item {
            pub pos: usize,
            pub height: usize,
        }
        let mut stack_heights_per_opcode: Vec<isize> = vec![Self::LOC_UNVISITED; code.len()];
        let mut height_update = 0;

        let current_section = metadata
            .get(section)
            .ok_or(Error::FunctionMetadataNotFound(section))?;

        let mut worklist: Vec<Item> = vec![Item {
            pos: 0,
            height: current_section.input as usize,
        }];
        let mut max_stack_height = current_section.input as usize;

        while let Some(worklist_item) = worklist.pop() {
            let mut pos = worklist_item.pos;
            let mut height = worklist_item.height;

            while pos < code.len() {
                let op = code.get_unchecked_at(pos);

                let want_option = stack_heights_per_opcode[pos];

                // Check if pos has already be visited; if so, the stack heights should be the same.
                if want_option != Self::LOC_UNVISITED {
                    if want_option as usize != height {
                        return Err(Error::ValidationConflictingStack(
                            height,
                            want_option as usize,
                        ));
                    }
                    // Already visited this path and stack height matches.
                    break;
                }

                stack_heights_per_opcode[pos] = height as isize;

                height_update += 1;

                OpcodeInfo::assert_opcode_valid(op)?;

                // SAFETY: `op` is already checked for a valid opcode, which means we shouldn't get None or "out of bounds"
                let opcode_info = unsafe { OpcodeInfo::OPCODE_INFO.get_unchecked(op as usize) };
                let opcode_info = opcode_info.as_ref().unwrap();

                // Validate height for current op and update as needed.
                if opcode_info.min_stack > height {
                    return Err(Error::StackUnderflow);
                }
                if opcode_info.max_stack < height {
                    return Err(Error::StackOverflow);
                }

                height = height + Self::STACK_LIMIT - opcode_info.max_stack;

                match op {
                    CALLF => {
                        let arg = code.get_u16_or_default(pos + 1) as usize;

                        let metadata = metadata
                            .get(arg)
                            .ok_or(Error::FunctionMetadataNotFound(arg))?;

                        let input = metadata.input as usize;
                        let output = metadata.output as usize;

                        if input > height {
                            return Err(Error::StackUnderflow);
                        }
                        if output + height > Self::STACK_LIMIT {
                            return Err(Error::StackOverflow);
                        }

                        height -= input;
                        height += output;
                        pos += 3;
                    }
                    RETF => {
                        if current_section.output as usize != height {
                            return Err(Error::ValidationInvalidOutputs(
                                current_section.output,
                                height,
                                pos,
                            ));
                        }
                        break;
                    }
                    RJUMP => {
                        let arg = code.get_i16_or_default(pos + 1);
                        pos = (pos as isize + 3 + arg as isize) as usize;
                    }
                    RJUMPI => {
                        let arg = code.get_i16_or_default(pos + 1);
                        worklist.push(Item {
                            pos: (pos as isize + 3 + arg as isize) as usize,
                            height,
                        });
                        pos += 3;
                    }
                    RJUMPV => {
                        let count = code.get_or_default(pos + 1);
                        for i in 0..count {
                            let arg = code.get_i16_or_default(pos + 2 + 2 * i as usize);
                            worklist.push(Item {
                                pos: (pos as isize + 2 + 2 * count as isize + arg as isize)
                                    as usize,
                                height,
                            });
                        }
                        pos += 2 + 2 * count as usize;
                    }
                    PUSH1..=PUSH32 => {
                        pos += 1 + (op - PUSH0) as usize;
                    }
                    _ if opcode_info.terminal => {
                        break;
                    }
                    _ => {
                        // Simple op, no operand.
                        pos += 1;
                    }
                }
                max_stack_height = max_stack_height.max(height);
            }
        }

        if max_stack_height != current_section.max_stack_height as usize {
            return Err(Error::ValidationInvalidMaxStackHeight(
                section,
                max_stack_height,
                current_section.max_stack_height,
            ));
        }
        Ok(height_update)
    }
}

#[allow(clippy::enum_glob_use)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::evm::Buffer;

    #[test]
    fn hello_world_validation() {
        let bytecode = r#"
        ef0001 010004 020001 0097 030263 00 00000008
        608080604052345d00896000600581556001908154908282811c921680155d006a60208310145d004f601f82115d0022505060186b48656c6c6f20576f726c642160a01b01905561026390816100aa8239f382815282601f60208320930160051c8301928381105d000550505cffc18281550183905cffec602490634e487b7160e01b81526022600452fd91607f16915cff8e600080fd
    
        ef0001 01000c 020003 009a 00fc 0041 03006d 00 00000005 0001000c 0201000a
    
        608060405260043610155d0004600080fd6000803560e01c80631f1bd692145d005680634e70b1dc145d002e63fc6492bc145d0004505cffd4345d001b806003193601125d000fb00001604051809181b000020390f380fd80fd50345d0017806003193601125d000b60209054604051908152f380fd80fd50345d001b806003193601125d000fb00001604051809181b000020390f380fd80fd
        6040516000600180549081811c908083169283155d00dd60209384841081145d00bf83875290816000145d0098506001145d0039505050601f82811992030116810181811067ffffffffffffffff8211175d0004604052b1634e487b7160e01b600052604160045260246000fd809293506000527fb10e2d527612073b26eecdfd717e6a320cf44b4afac2b0732d9fcbe2b7fa0cf691836000938385105d000d505050508201013880805cff879182819495935483858a01015201910192919084905cffd592939450505060ff191682840152151560051b8201013880805cff53602486634e487b7160e01b81526022600452fd91607f16915cff1b
        602091828252805190818484015260008281105d00185050604092506000838284010152601f80199101160101b1808580928401015160408287010152015cffcf
    
        a364697066735822122096760ffdb7239f731b9acc9ffa81afe01fa6d1b6d1e80121c1aaeabae13cb3cd6c6578706572696d656e74616cf564736f6c63782c302e382e31382d646576656c6f702e323032332e382e31352b636f6d6d69742e34363964366434642e6d6f64006b
        "#.replace(" ", "").replace("\n", "");

        let bytecode = (0..bytecode.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&bytecode[i..i + 2], 16).unwrap())
            .collect::<Vec<_>>();

        let buffer_bytecode = Buffer::from_slice(bytecode.as_slice());
        let container = Container::unmarshal_binary(&buffer_bytecode).unwrap();

        container.validate_container().unwrap();
    }

    #[test]
    fn validation_test() {
        let codes = vec![
            (
                Buffer::from_slice(&[CALLER, POP, STOP]),
                vec![FunctionMetadata {
                    input: 0,
                    output: 0,
                    max_stack_height: 1,
                }],
            ),
            (
                Buffer::from_slice(&[CALLF, 0x00, 0x00, STOP]),
                vec![FunctionMetadata {
                    input: 0,
                    output: 0,
                    max_stack_height: 0,
                }],
            ),
            (
                Buffer::from_slice(&[ADDRESS, CALLF, 0x00, 0x00, STOP]),
                vec![FunctionMetadata {
                    input: 0,
                    output: 0,
                    max_stack_height: 1,
                }],
            ),
            (
                Buffer::from_slice(&[
                    RJUMP, 0x00, 0x03, JUMPDEST, JUMPDEST, RETURN, PUSH1, 20, PUSH1, 39, PUSH1,
                    0x00, CODECOPY, PUSH1, 20, PUSH1, 0x00, RJUMP, 0xff, 0xef,
                ]),
                vec![FunctionMetadata {
                    input: 0,
                    output: 0,
                    max_stack_height: 3,
                }],
            ),
            (
                Buffer::from_slice(&[
                    PUSH1, 1, RJUMPI, 0x00, 0x03, JUMPDEST, JUMPDEST, STOP, PUSH1, 20, PUSH1, 39,
                    PUSH1, 0x00, CODECOPY, PUSH1, 20, PUSH1, 0x00, RETURN,
                ]),
                vec![FunctionMetadata {
                    input: 0,
                    output: 0,
                    max_stack_height: 3,
                }],
            ),
            (
                Buffer::from_slice(&[
                    PUSH1, 1, RJUMPV, 0x02, 0x00, 0x03, 0xff, 0xf8, JUMPDEST, JUMPDEST, STOP,
                    PUSH1, 20, PUSH1, 39, PUSH1, 0x00, CODECOPY, PUSH1, 20, PUSH1, 0x00, RETURN,
                ]),
                vec![FunctionMetadata {
                    input: 0,
                    output: 0,
                    max_stack_height: 3,
                }],
            ),
            (
                Buffer::from_slice(&[RETF]),
                vec![FunctionMetadata {
                    input: 3,
                    output: 3,
                    max_stack_height: 3,
                }],
            ),
            (
                Buffer::from_slice(&[CALLF, 0x00, 0x01, POP, STOP]),
                vec![
                    FunctionMetadata {
                        input: 0,
                        output: 0,
                        max_stack_height: 1,
                    },
                    FunctionMetadata {
                        input: 0,
                        output: 1,
                        max_stack_height: 0,
                    },
                ],
            ),
            (
                Buffer::from_slice(&[ORIGIN, ORIGIN, CALLF, 0x00, 0x01, POP, RETF]),
                vec![
                    FunctionMetadata {
                        input: 0,
                        output: 0,
                        max_stack_height: 2,
                    },
                    FunctionMetadata {
                        input: 2,
                        output: 1,
                        max_stack_height: 2,
                    },
                ],
            ),
        ];

        for (code, meta) in codes {
            assert!(Container::validate_code(&code, 0, &meta).is_ok());
        }
    }

    #[test]
    #[should_panic(expected = "FunctionMetadataNotFound(0)")]
    fn validation_test_with_function_metadata_not_found() {
        let code = Buffer::from_slice(&[RETF]);
        let metas = vec![];

        Container::validate_code(&code, 0, &metas).unwrap();
    }

    #[test]
    #[should_panic(expected = "ValidationInvalidCodeTermination(80, 2)")]
    fn validation_test_with_invalid_code_termination() {
        let code = Buffer::from_slice(&[CALLER, POP]);
        let meta = FunctionMetadata {
            input: 0,
            output: 0,
            max_stack_height: 1,
        };
        Container::validate_code(&code, 0, &vec![meta]).unwrap();
    }

    #[test]
    #[should_panic(expected = "ValidationUnreachableCode")]
    fn validation_test_with_unreachable_code_1() {
        let code = Buffer::from_slice(&[RJUMP, 0x00, 0x01, CALLER, STOP]);
        let meta = FunctionMetadata {
            input: 0,
            output: 0,
            max_stack_height: 0,
        };
        Container::validate_code(&code, 0, &vec![meta]).unwrap();
    }

    #[test]
    #[should_panic(expected = "ValidationUnreachableCode")]
    fn validation_test_with_unreachable_code_2() {
        let code = Buffer::from_slice(&[STOP, STOP, INVALID]);
        let meta = FunctionMetadata {
            input: 0,
            output: 0,
            max_stack_height: 0,
        };
        Container::validate_code(&code, 0, &vec![meta]).unwrap();
    }

    #[test]
    #[should_panic(expected = "StackUnderflow")]
    fn validation_test_stack_underflow() {
        let code = Buffer::from_slice(&[PUSH1, 0x42, ADD, STOP]);
        let meta = FunctionMetadata {
            input: 0,
            output: 0,
            max_stack_height: 1,
        };

        Container::validate_code(&code, 0, &vec![meta]).unwrap();
    }

    #[test]
    #[should_panic(expected = "ValidationInvalidMaxStackHeight(0, 1, 2)")]
    fn validation_test_with_invalid_max_stack_height() {
        let code = Buffer::from_slice(&[PUSH1, 0x42, POP, STOP]);
        let meta = FunctionMetadata {
            input: 0,
            output: 0,
            max_stack_height: 2,
        };
        Container::validate_code(&code, 0, &vec![meta]).unwrap();
    }

    #[test]
    #[should_panic(expected = "ValidationInvalidJumpDest(1, 5, 2)")]
    fn validation_test_with_invalid_jump_dest_1() {
        let code = Buffer::from_slice(&[
            PUSH0, RJUMPI, 0x00, 0x01, PUSH1, 0x42, // jumps to here
            POP, STOP,
        ]);
        let meta = FunctionMetadata {
            input: 0,
            output: 0,
            max_stack_height: 1,
        };
        Container::validate_code(&code, 0, &vec![meta]).unwrap();
    }

    #[test]
    #[should_panic(expected = "ValidationInvalidJumpDest(1, 8, 3)")]
    fn validation_test_with_invalid_jump_dest_2() {
        let code = Buffer::from_slice(&[
            PUSH0, RJUMPV, 0x02, 0x00, 0x01, 0x00, 0x02, PUSH1, 0x42, // jumps to here
            POP,  // and here
            STOP,
        ]);
        let meta = FunctionMetadata {
            input: 0,
            output: 0,
            max_stack_height: 1,
        };
        Container::validate_code(&code, 0, &vec![meta]).unwrap();
    }

    #[test]
    #[should_panic(expected = "ValidationInvalidBranchCount(1)")]
    fn validation_test_with_invalid_branch_count() {
        let code = Buffer::from_slice(&[PUSH0, RJUMPV, 0x00, STOP]);
        let meta = FunctionMetadata {
            input: 0,
            output: 0,
            max_stack_height: 1,
        };
        Container::validate_code(&code, 0, &vec![meta]).unwrap();
    }

    #[test]
    #[should_panic(expected = "ValidationInvalidOutputs(1, 0, 0)")]
    fn validation_test_with_invalid_outputs() {
        let code = Buffer::from_slice(&[RETF]);
        let meta = FunctionMetadata {
            input: 0,
            output: 1,
            max_stack_height: 0,
        };
        Container::validate_code(&code, 0, &vec![meta]).unwrap();
    }
}
