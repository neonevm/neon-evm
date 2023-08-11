use std::collections::HashMap;
use crate::evm::Buffer;
use super::{database::Database, Machine, eof::Container};
use crate::error::{Error, Result};
use crate::evm::analysis::Bitvec;
use crate::evm::eof::FunctionMetadata;
use crate::evm::opcode_table::OpCode;
use crate::evm::opcode_table::OpCode::*;
use crate::evm::stack::STACK_SIZE;

impl<B: Database> Machine<B> {
    pub fn validate_container_code(&self, container: &Container) -> Result<()> {
        for (section, code) in container.code.iter().enumerate() {
            self.validate_code(code, section, &container.types)?;
        }

        Ok(())
    }

    pub fn validate_code(&self, code: &Buffer, section: usize, metadata: &Vec<FunctionMetadata>) -> Result<()> {
        let mut i: usize = 0;
        let mut count: u8 = 0;
        let mut analysis: Option<Bitvec> = None;
        let mut opcode: u8 = 0;
        while i < code.len() {
            count += 1;
            opcode = code.get_or_default(i);

            if !OpCode::has_opcode(opcode) {
                return Err(Error::UnknownOpcode(
                    self.context.contract,
                    code[i],
                ));
            }


            if opcode > PUSH0 as u8 && opcode <= PUSH32 as u8 {
                let size = opcode - PUSH0 as u8;
                if code.len() <= i + size as usize {
                    return Err(Error::PushOutOfBounds(self.context.contract));
                }
                i += size as usize;
            }

            if opcode == RJUMP as u8 || opcode == RJUMPI as u8 {
                if code.len() <= i + 2 {
                    return Err(Error::JumpTableSizeMissing(self.context.contract, i));
                }
                analysis = Some(self.check_dest(code, analysis, i + 1, i + 3, code.len())?);
                i += 2;
            }

            if opcode == RJUMPV as u8 {
                if code.len() <= i + 1 {
                    return Err(Error::PushOutOfBounds(self.context.contract));
                }
                count = code.get_or_default(i + 1);
                if count == 0 {
                    return Err(Error::InvalidBranchCount(self.context.contract, i + 1));
                }
                if code.len() <= i + count as usize {
                    return Err(Error::JumpTableTruncated(self.context.contract, i));
                }
                for j in 0..count {
                    analysis = Some(self.check_dest(code, analysis, i + 2 + j as usize * 2, i + 2 * count as usize + 2, code.len())?);
                }
                i += 1 + 2 * count as usize;
            }

            if opcode == CALLF as u8 {
                if i + 2 >= code.len() {
                    return Err(Error::TruncatedImmediate(opcode, i));
                }
                let arg = code.get_u16_or_default(i + 1);

                if arg as usize >= metadata.len() {
                    return Err(Error::InvalidSectionArgument(arg, metadata.len(), i));
                }
                i += 2
            }

            i += 1;
        };

        if !OpCode::is_terminal_opcode(opcode) {
            return Err(Error::InvalidCodeTermination(opcode, i - 1));
        }

        let path = self.validate_control_flow(code, section, metadata)?;
        if path != count as usize {
            return Err(Error::UnreachableCode);
        }
        Ok(())
    }

    fn check_dest(&self, code: &Buffer, analysis_option: Option<Bitvec>, imm: usize, from: usize, length: usize) -> Result<Bitvec> {
        if code.len() < imm + 2 {
            return Err(Error::UnexpectedEOF);
        }
        let analysis = match analysis_option {
            Some(a) => a,
            None => Bitvec::eof_code_bitmap(code)
        };
        let offset = code.get_i16_or_default(imm);
        let dest = (from as isize + offset as isize) as usize;
        if dest >= length {
            return Err(Error::InvalidJump(self.context.contract, dest));
        }
        if !analysis.is_code_segment(dest) {
            return Err(Error::InvalidJump(self.context.contract, dest));
        }
        Ok(analysis)
    }

    fn validate_control_flow(&self, code: &Buffer, section: usize, metadata: &Vec<FunctionMetadata>) -> Result<usize> {
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
                        return Err(Error::ConflictingStack(height, *want));
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
                            return Err(Error::InvalidOutputs(metadata[section].output, height, pos));
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
            return Err(Error::InvalidMaxStackHeight(section, max_stack_height, metadata[section].max_stack_height as usize));
        }
        Ok(heights.len())
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use solana_program::account_info::AccountInfo;
    use solana_program::pubkey::Pubkey;
    use crate::account::Operator;
    use crate::account_storage::ProgramAccountStorage;
    use crate::executor::ExecutorState;
    use crate::types::{Address, Transaction};
    use super::*;
    use crate::evm::Buffer;

    #[test]
    fn test() {
        let tx = Transaction::default();
        let address = Address::default();
        let program_id = Pubkey::new_unique();

        let pa = Pubkey::from_str("9kPRbbwKL5SYELF4cZqWWFmP88QkKys51DoaUBx8eK73").unwrap();
        let oa = Pubkey::from([0; 32]);
        let la = &mut 0;
        let da = &mut [];
        let a = AccountInfo::new(&pa, true, false, la, da, &oa, false, 0);

        let pb = Pubkey::from_str("A9Hbf8q2BN3NcbWLVmNXA6EML2BWxZEcP93h5p5DvqEV").unwrap();
        let ob = Pubkey::from([0; 32]);
        let lb = &mut 0;
        let db = &mut [];
        let b = AccountInfo::new(&pb, true, false, lb, db, &ob, false, 0);

        let pc = Pubkey::from_str("5qZYTbMBvbNntsfQjg58vcmVwJKeRCP4MiDtPooJ2bM8").unwrap();
        let oc = Pubkey::from([0; 32]);
        let lc = &mut 0;
        let dc = &mut [];
        let c = AccountInfo::new(&pc, true, false, lc, dc, &oc, false, 0);

        let pd = Pubkey::from_str("SysvarC1ock11111111111111111111111111111111").unwrap();
        let od = Pubkey::from([0; 32]);
        let ld = &mut 0;
        let dd = &mut [];
        let d = AccountInfo::new(&pd, true, false, ld, dd, &od, false, 0);

        let operator = Operator::from_account(&a).unwrap();
        let accounts = [a.clone(), b, c, d];

        let storage = ProgramAccountStorage::new(&program_id, &operator, None, &accounts).unwrap();
        let mut backend = ExecutorState::new(&storage);
        let machine = Machine::new(tx, address, &mut backend).unwrap();
        println!("Test 1");

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
            let result = machine.validate_code(&code, 0, &vec![meta]);
            assert!(result.is_ok());
        }
        println!("Test 2");
        {
            let code = Buffer::from_slice(&[
                CALLF as u8, 0x00, 0x00,
                STOP as u8]);
            let meta = FunctionMetadata {
                input: 0,
                output: 0,
                max_stack_height: 0,
            };
            let result = machine.validate_code(&code, 0, &vec![meta]);
            assert!(result.is_ok());
        }
        println!("Test 3");
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
            let result = machine.validate_code(&code, 0, &vec![meta]);
            assert!(result.is_ok());
        }
        println!("Test 4");
        {
            let code = Buffer::from_slice(&[
                CALLER.u8(), POP.u8()]);
            let meta = FunctionMetadata {
                input: 0,
                output: 0,
                max_stack_height: 1,
            };
            let result = machine.validate_code(&code, 0, &vec![meta]);
            assert!(result.is_err());
            assert_eq!(result.err().unwrap().to_string(), Error::InvalidCodeTermination(POP.u8(), 1).to_string())
        }
        println!("Test 5");
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
            let result = machine.validate_code(&code, 0, &vec![meta]);
            assert!(result.is_err());
            assert_eq!(result.err().unwrap().to_string(), Error::UnreachableCode.to_string())
        }
        println!("Test 6");
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
            let result = machine.validate_code(&code, 0, &vec![meta]);
            assert!(result.is_err());
            assert_eq!(result.err().unwrap().to_string(), Error::StackUnderflow.to_string())
        }
        println!("Test 7");
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
            let result = machine.validate_code(&code, 0, &vec![meta]);
            assert!(result.is_err());
            assert_eq!(result.err().unwrap().to_string(), Error::InvalidMaxStackHeight(0, 1, 2).to_string())
        }
        println!("Test 8");
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
            let result = machine.validate_code(&code, 0, &vec![meta]);
            assert!(result.is_err());
            assert_eq!(result.err().unwrap().to_string(), Error::InvalidJump(Address::from_hex("0xbd770416a3345f91e4b34576cb804a576fa48eb1").unwrap(), 5).to_string())
        }
        println!("Test 9");
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
            let result = machine.validate_code(&code, 0, &vec![meta]);
            assert!(result.is_err());
            assert_eq!(result.err().unwrap().to_string(), Error::InvalidJump(Address::from_hex("0xbd770416a3345f91e4b34576cb804a576fa48eb1").unwrap(), 8).to_string())
        }
        println!("Test 10");
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
            let result = machine.validate_code(&code, 0, &vec![meta]);
            assert!(result.is_err());
            assert_eq!(result.err().unwrap().to_string(), Error::InvalidBranchCount(Address::from_hex("0xbd770416a3345f91e4b34576cb804a576fa48eb1").unwrap(), 2).to_string())
        }
        println!("Test 11");
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
            let result = machine.validate_code(&code, 0, &vec![meta]);
            assert!(result.is_ok());
        }
        println!("Test 12");
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
            let result = machine.validate_code(&code, 0, &vec![meta]);
            assert!(result.is_ok());
        }
        println!("Test 13");
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
            let result = machine.validate_code(&code, 0, &vec![meta]);
            assert!(result.is_ok());
        }
        println!("Test 14");
        {
            let code = Buffer::from_slice(&[STOP as u8,
                STOP as u8,
                INVALID as u8, ]);
            let meta = FunctionMetadata {
                input: 0,
                output: 0,
                max_stack_height: 0,
            };
            let result = machine.validate_code(&code, 0, &vec![meta]);
            assert!(result.is_err());
            assert_eq!(result.err().unwrap().to_string(), Error::UnreachableCode.to_string())
        }
        println!("Test 15");
        {
            let code = Buffer::from_slice(&[RETF as u8, ]);
            let meta = FunctionMetadata {
                input: 0,
                output: 1,
                max_stack_height: 0,
            };
            let result = machine.validate_code(&code, 0, &vec![meta]);
            assert!(result.is_err());
            assert_eq!(result.err().unwrap().to_string(), Error::InvalidOutputs(1, 0 ,0).to_string())
        }
        println!("Test 16");
        {
            let code = Buffer::from_slice(&[RETF as u8, ]);
            let meta = FunctionMetadata {
                input: 3,
                output: 3,
                max_stack_height: 3,
            };
            let result = machine.validate_code(&code, 0, &vec![meta]);
            assert!(result.is_ok());
        }
        println!("Test 17");
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
            let result = machine.validate_code(&code, 0, &vec![meta1, meta2]);
            assert!(result.is_ok());
        }
        println!("Test 18");
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
            let result = machine.validate_code(&code, 0, &vec![meta1, meta2]);
            assert!(result.is_ok());
        }
    }
}
