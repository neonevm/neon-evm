use std::collections::HashMap;
use crate::evm::Buffer;
use super::{eof::Container};
use crate::error::{Error, Result};
use crate::evm::analysis::Bitvec;
use crate::evm::eof::FunctionMetadata;
use crate::evm::opcode_table::OpCode;
use crate::evm::opcode_table::OpCode::*;
use crate::evm::stack::STACK_SIZE;

impl Container {
    pub fn validate_container(&self) -> Result<()> {
        for (section, code) in self.code.iter().enumerate() {
            Self::validate_code(code, section, &self.types)?;
        }

        Ok(())
    }

    pub fn validate_code(code: &Buffer, section: usize, metadata: &Vec<FunctionMetadata>) -> Result<()> {
        let mut i: usize = 0;
        let mut count: u8 = 0;
        let mut analysis: Option<Bitvec> = None;
        let mut opcode: u8 = 0;
        while i < code.len() {
            count += 1;
            opcode = code.get_or_default(i);

            if !OpCode::has_opcode(opcode) {
                return Err(Error::ValidationUndefinedInstruction(
                    opcode,
                    i,
                ));
            }


            if opcode > PUSH0 as u8 && opcode <= PUSH32 as u8 {
                let size = opcode - PUSH0 as u8;
                if code.len() <= i + size as usize {
                    return Err(Error::ValidationTruncatedImmediate(opcode, i));
                }
                i += size as usize;
            }

            if opcode == RJUMP as u8 || opcode == RJUMPI as u8 {
                if code.len() <= i + 2 {
                    return Err(Error::ValidationTruncatedImmediate(opcode, i));
                }
                analysis = Some(Self::check_dest(code, analysis, i + 1, i + 3, code.len())?);
                i += 2;
            }

            if opcode == RJUMPV as u8 {
                if code.len() <= i + 1 {
                    return Err(Error::ValidationTruncatedImmediate(opcode, i));
                }
                count = code.get_or_default(i + 1);
                if count == 0 {
                    return Err(Error::ValidationInvalidBranchCount(i));
                }
                if code.len() <= i + count as usize {
                    return Err(Error::ValidationTruncatedImmediate(opcode, i));
                }
                for j in 0..count {
                    analysis = Some(Self::check_dest(code, analysis, i + 2 + j as usize * 2, i + 2 * count as usize + 2, code.len())?);
                }
                i += 1 + 2 * count as usize;
            }

            if opcode == CALLF as u8 {
                if i + 2 >= code.len() {
                    return Err(Error::ValidationTruncatedImmediate(opcode, i));
                }
                let arg = code.get_u16_or_default(i + 1);

                if arg as usize >= metadata.len() {
                    return Err(Error::ValidationInvalidSectionArgument(arg, metadata.len(), i));
                }
                i += 2
            }

            i += 1;
        };

        if !OpCode::is_terminal_opcode(opcode) {
            return Err(Error::ValidationInvalidCodeTermination(opcode, i));
        }

        let path = Self::validate_control_flow(code, section, metadata)?;
        if path != count as usize {
            return Err(Error::ValidationUnreachableCode);
        }
        Ok(())
    }

    fn check_dest(code: &Buffer, analysis_option: Option<Bitvec>, imm: usize, from: usize, length: usize) -> Result<Bitvec> {
        if code.len() < imm + 2 {
            return Err(Error::UnexpectedEndOfFile);
        }
        let analysis = match analysis_option {
            Some(a) => a,
            None => Bitvec::eof_code_bitmap(code)
        };
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

    fn validate_control_flow(code: &Buffer, section: usize, metadata: &Vec<FunctionMetadata>) -> Result<usize> {
        struct Item {
            pub pos: usize,
            pub height: usize,
        }
        let mut heights: HashMap<usize, usize> = HashMap::new();
        let mut worklist: Vec<Item> = vec![Item { pos: 0, height: metadata[section].input as usize }];
        let mut max_stack_height = metadata[section].input as usize;
        while worklist.len() > 0 {
            let worklist_item = worklist.pop().unwrap();
            let mut pos = worklist_item.pos;
            let mut height = worklist_item.height;

            'outer: while pos < code.len() {
                let op = code.get_or_default(pos);

                let want_option = heights.get(&pos);
                if let Some(want) = want_option {
                    if *want != height {
                        return Err(Error::ValidationConflictingStack(height, *want));
                    }
                    break;
                }

                heights.insert(pos, height);

                let op_code: OpCode = op.try_into()?;
                let opcode_info = OpCode::opcode_info(op_code);
                if opcode_info.min_stack > height {
                    return Err(Error::StackUnderflow);
                }
                if opcode_info.max_stack < height {
                    return Err(Error::StackOverflow);
                }

                height = height + STACK_SIZE - opcode_info.max_stack;

                match op_code {
                    CALLF => {
                        let arg = code.get_u16_or_default(pos + 1) as usize;
                        if metadata[arg].input as usize > height { // TODO: check exists
                            return Err(Error::StackUnderflow);
                        }
                        if metadata[arg].output as usize + height > STACK_SIZE {// TODO: check exists
                            return Err(Error::StackOverflow);
                        }
                        height -= metadata[arg].input as usize;
                        height += metadata[arg].output as usize;
                        pos += 3;
                    }
                    RETF => {
                        if metadata[section].output as usize != height {
                            return Err(Error::ValidationInvalidOutputs(metadata[section].output, height, pos));
                        }
                        break 'outer;
                    }
                    RJUMP => {
                        let arg = code.get_i16_or_default(pos + 1);
                        pos = (pos as isize + 3 + arg as isize) as usize;
                    }
                    RJUMPI => {
                        let arg = code.get_i16_or_default(pos + 1);
                        worklist.push(Item { pos: (pos as isize + 3 + arg as isize) as usize, height });
                        pos += 3;
                    }
                    RJUMPV => {
                        let count = code.get_or_default(pos + 1);
                        for i in 0..count {
                            let arg = code.get_i16_or_default(pos + 2 + 2 * i as usize);
                            worklist.push(Item { pos: (pos as isize + 2 + 2 * count as isize + arg as isize) as usize, height })
                        }
                        pos += 2 + 2 * count as usize;
                    }
                    _ => {
                        if op >= PUSH1.u8() && op <= PUSH32.u8() {
                            pos += 1 + (op - PUSH0.u8()) as usize
                        } else if opcode_info.terminal {
                            break 'outer;
                        } else {
                            // Simple op, no operand.
                            pos += 1
                        }
                    }
                }
                max_stack_height = max_stack_height.max(height)
            }
        };
        if max_stack_height != metadata[section].max_stack_height as usize {
            return Err(Error::ValidationInvalidMaxStackHeight(section, max_stack_height, metadata[section].max_stack_height));
        }
        Ok(heights.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evm::Buffer;

    #[test]
    fn validation_test_1()
    {
        let code = Buffer::from_slice(&[
            CALLER as u8,
            POP as u8,
            STOP as u8]);
        let meta = FunctionMetadata {
            input: 0,
            output: 0,
            max_stack_height: 1,
        };
        let result = Container::validate_code(&code, 0, &vec![meta]);
        assert!(result.is_ok());
    }

    #[test]
    fn validation_test_2()
    {
        let code = Buffer::from_slice(&[
            CALLF as u8, 0x00, 0x00,
            STOP as u8]);
        let meta = FunctionMetadata {
            input: 0,
            output: 0,
            max_stack_height: 0,
        };
        let result = Container::validate_code(&code, 0, &vec![meta]);
        assert!(result.is_ok());
    }

    #[test]
    fn validation_test_3()
    {
        let code = Buffer::from_slice(&[
            ADDRESS as u8,
            CALLF as u8, 0x00, 0x00,
            STOP as u8, ]);
        let meta = FunctionMetadata {
            input: 0,
            output: 0,
            max_stack_height: 1,
        };
        let result = Container::validate_code(&code, 0, &vec![meta]);
        assert!(result.is_ok());
    }

    #[test]
    fn validation_test_4()
    {
        let code = Buffer::from_slice(&[
            CALLER.u8(), POP.u8()]);
        let meta = FunctionMetadata {
            input: 0,
            output: 0,
            max_stack_height: 1,
        };
        let result = Container::validate_code(&code, 0, &vec![meta]);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap().to_string(), Error::ValidationInvalidCodeTermination(POP.u8(), 2).to_string())
    }

    #[test]
    fn validation_test_5()
    {
        let code = Buffer::from_slice(&[
            RJUMP as u8,
            0x00,
            0x01,
            CALLER as u8,
            STOP as u8, ]);
        let meta = FunctionMetadata {
            input: 0,
            output: 0,
            max_stack_height: 0,
        };
        let result = Container::validate_code(&code, 0, &vec![meta]);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap().to_string(), Error::ValidationUnreachableCode.to_string())
    }

    #[test]
    fn validation_test_6()
    {
        let code = Buffer::from_slice(&[PUSH1 as u8,
            0x42,
            ADD as u8,
            STOP as u8, ]);
        let meta = FunctionMetadata {
            input: 0,
            output: 0,
            max_stack_height: 1,
        };
        let result = Container::validate_code(&code, 0, &vec![meta]);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap().to_string(), Error::StackUnderflow.to_string())
    }

    #[test]
    fn validation_test_7()
    {
        let code = Buffer::from_slice(&[PUSH1 as u8,
            0x42,
            POP as u8,
            STOP as u8, ]);
        let meta = FunctionMetadata {
            input: 0,
            output: 0,
            max_stack_height: 2,
        };
        let result = Container::validate_code(&code, 0, &vec![meta]);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap().to_string(), Error::ValidationInvalidMaxStackHeight(0, 1, 2).to_string())
    }

    #[test]
    fn validation_test_8()
    {
        let code = Buffer::from_slice(&[PUSH0 as u8,
            RJUMPI as u8,
            0x00,
            0x01,
            PUSH1 as u8,
            0x42, // jumps to here
            POP as u8,
            STOP as u8, ]);
        let meta = FunctionMetadata {
            input: 0,
            output: 0,
            max_stack_height: 1,
        };
        let result = Container::validate_code(&code, 0, &vec![meta]);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap().to_string(), Error::ValidationInvalidJumpDest(1, 5, 2).to_string())
    }

    #[test]
    fn validation_test_9()
    {
        let code = Buffer::from_slice(&[PUSH0 as u8,
            RJUMPV as u8,
            0x02,
            0x00,
            0x01,
            0x00,
            0x02,
            PUSH1 as u8,
            0x42, // jumps to here
            POP as u8,  // and here
            STOP as u8, ]);
        let meta = FunctionMetadata {
            input: 0,
            output: 0,
            max_stack_height: 1,
        };
        let result = Container::validate_code(&code, 0, &vec![meta]);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap().to_string(), Error::ValidationInvalidJumpDest(1, 8, 3).to_string())
    }

    #[test]
    fn validation_test_10()
    {
        let code = Buffer::from_slice(&[PUSH0 as u8,
            RJUMPV as u8,
            0x00,
            STOP as u8, ]);
        let meta = FunctionMetadata {
            input: 0,
            output: 0,
            max_stack_height: 1,
        };
        let result = Container::validate_code(&code, 0, &vec![meta]);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap().to_string(), Error::ValidationInvalidBranchCount(1).to_string())
    }

    #[test]
    fn validation_test_11()
    {
        let code = Buffer::from_slice(&[RJUMP as u8, 0x00, 0x03,
            JUMPDEST as u8,
            JUMPDEST as u8,
            RETURN as u8,
            PUSH1 as u8, 20,
            PUSH1 as u8, 39,
            PUSH1 as u8, 0x00,
            CODECOPY as u8,
            PUSH1 as u8, 20,
            PUSH1 as u8, 0x00,
            RJUMP as u8, 0xff, 0xef, ]);
        let meta = FunctionMetadata {
            input: 0,
            output: 0,
            max_stack_height: 3,
        };
        let result = Container::validate_code(&code, 0, &vec![meta]);
        assert!(result.is_ok());
    }

    #[test]
    fn validation_test_12()
    {
        let code = Buffer::from_slice(&[PUSH1 as u8, 1,
            RJUMPI as u8, 0x00, 0x03,
            JUMPDEST as u8,
            JUMPDEST as u8,
            STOP as u8,
            PUSH1 as u8, 20,
            PUSH1 as u8, 39,
            PUSH1 as u8, 0x00,
            CODECOPY as u8,
            PUSH1 as u8, 20,
            PUSH1 as u8, 0x00,
            RETURN as u8, ]);
        let meta = FunctionMetadata {
            input: 0,
            output: 0,
            max_stack_height: 3,
        };
        let result = Container::validate_code(&code, 0, &vec![meta]);
        assert!(result.is_ok());
    }

    #[test]
    fn validation_test_13()
    {
        let code = Buffer::from_slice(&[PUSH1 as u8, 1,
            RJUMPV as u8, 0x02, 0x00, 0x03, 0xff, 0xf8,
            JUMPDEST as u8,
            JUMPDEST as u8,
            STOP as u8,
            PUSH1 as u8, 20,
            PUSH1 as u8, 39,
            PUSH1 as u8, 0x00,
            CODECOPY as u8,
            PUSH1 as u8, 20,
            PUSH1 as u8, 0x00,
            RETURN as u8, ]);
        let meta = FunctionMetadata {
            input: 0,
            output: 0,
            max_stack_height: 3,
        };
        let result = Container::validate_code(&code, 0, &vec![meta]);
        assert!(result.is_ok());
    }

    #[test]
    fn validation_test_14()
    {
        let code = Buffer::from_slice(&[STOP as u8,
            STOP as u8,
            INVALID as u8, ]);
        let meta = FunctionMetadata {
            input: 0,
            output: 0,
            max_stack_height: 0,
        };
        let result = Container::validate_code(&code, 0, &vec![meta]);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap().to_string(), Error::ValidationUnreachableCode.to_string())
    }

    #[test]
    fn validation_test_15()
    {
        let code = Buffer::from_slice(&[RETF as u8, ]);
        let meta = FunctionMetadata {
            input: 0,
            output: 1,
            max_stack_height: 0,
        };
        let result = Container::validate_code(&code, 0, &vec![meta]);
        assert!(result.is_err());
        assert_eq!(result.err().unwrap().to_string(), Error::ValidationInvalidOutputs(1, 0, 0).to_string())
    }

    #[test]
    fn validation_test_16()
    {
        let code = Buffer::from_slice(&[RETF as u8, ]);
        let meta = FunctionMetadata {
            input: 3,
            output: 3,
            max_stack_height: 3,
        };
        let result = Container::validate_code(&code, 0, &vec![meta]);
        assert!(result.is_ok());
    }

    #[test]
    fn validation_test_17()
    {
        let code = Buffer::from_slice(&[CALLF as u8, 0x00, 0x01,
            POP as u8,
            STOP as u8, ]);
        let meta1 = FunctionMetadata {
            input: 0,
            output: 0,
            max_stack_height: 1,
        };
        let meta2 = FunctionMetadata {
            input: 0,
            output: 1,
            max_stack_height: 0,
        };
        let result = Container::validate_code(&code, 0, &vec![meta1, meta2]);
        assert!(result.is_ok());
    }

    #[test]
    fn validation_test_18()
    {
        let code = Buffer::from_slice(&[ORIGIN as u8,
            ORIGIN as u8,
            CALLF as u8, 0x00, 0x01,
            POP as u8,
            RETF as u8, ]);
        let meta1 = FunctionMetadata {
            input: 0,
            output: 0,
            max_stack_height: 2,
        };
        let meta2 = FunctionMetadata {
            input: 2,
            output: 1,
            max_stack_height: 2,
        };
        let result = Container::validate_code(&code, 0, &vec![meta1, meta2]);
        assert!(result.is_ok());
    }
}

