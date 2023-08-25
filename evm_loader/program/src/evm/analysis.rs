#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use crate::evm::Buffer;

#[allow(clippy::wildcard_imports)]
use crate::evm::opcode_table::opcode::*;

pub struct Bitvec(Vec<u8>);

const BITS_MASK: [u16; 8] = [0, 1, 0b11, 0b111, 0b1111, 0b1_1111, 0b11_1111, 0b111_1111];

impl Bitvec {
    pub fn new(capacity: usize) -> Self {
        Bitvec(vec![0; capacity])
    }

    pub fn _set1(&mut self, pos: usize) {
        self.0[pos / 8] |= 1 << (pos % 8);
    }

    pub fn set_n(&mut self, flag: u16, pos: usize) {
        let a = flag << (pos % 8);
        self.0[pos / 8] |= a as u8;
        let b = (a >> 8) as u8;
        if b != 0 {
            self.0[pos / 8 + 1] = b;
        }
    }

    pub fn set8(&mut self, pos: usize) {
        let a = (0xFF << (pos % 8)) as u8;
        self.0[pos / 8] |= a;
        self.0[pos / 8 + 1] = !a;
    }

    pub fn set16(&mut self, pos: usize) {
        let a = (0xFF << (pos % 8)) as u8;
        self.0[pos / 8] |= a;
        self.0[pos / 8 + 1] = 0xFF;
        self.0[pos / 8 + 2] = !a;
    }

    pub fn is_code_segment(&self, pos: usize) -> bool {
        ((self.0[pos / 8] >> (pos % 8)) & 1) == 0
    }

    #[allow(dead_code)]
    pub fn to_vec(&self) -> &Vec<u8> {
        &self.0
    }

    // eofCodeBitmap collects data locations in code.
    pub fn eof_code_bitmap(code: &Buffer) -> Bitvec {
        // The bitmap is 4 bytes longer than necessary, in case the code
        // ends with a PUSH32, the algorithm will push zeroes onto the
        // bitvector outside the bounds of the actual code.
        let mut bits = Bitvec::new(code.len() / 8 + 1 + 4);
        bits.eof_code_bitmap_internal(code);
        bits
    }

    // eofCodeBitmapInternal is the internal implementation of codeBitmap for EOF
    // code validation.
    pub fn eof_code_bitmap_internal(&mut self, code: &Buffer) {
        let mut pc: usize = 0;
        while pc < code.len() {
            let op = code.get_or_default(pc);
            let mut numbits: u8;
            pc += 1;

            match op {
                PUSH1..=PUSH32 => {
                    numbits = op - PUSH1 + 1;
                }

                RJUMP | RJUMPI | CALLF => {
                    numbits = 2;
                }

                RJUMPV => {
                    // RJUMPV is unique as it has a variable sized operand.
                    // The total size is determined by the count byte which
                    // immediate proceeds RJUMPV. Truncation will be caught
                    // in other validation steps -- for now, just return a
                    // valid bitmap for as much of the code as is
                    // available.
                    let end = code.len();
                    if pc >= end {
                        // Count missing, no more bits to mark.
                        return;
                    }
                    numbits = code.get_or_default(pc) * 2 + 1;
                    if pc + numbits as usize > end {
                        // Jump table is truncated, mark as many bits
                        // as possible.
                        numbits = (end - pc) as u8;
                    }
                }

                _ => continue,
            }

            if numbits >= 8 {
                while numbits >= 16 {
                    self.set16(pc);
                    numbits -= 16;
                    pc += 16;
                }
                while numbits >= 8 {
                    self.set8(pc);
                    numbits -= 8;
                    pc += 8;
                }
            }

            if (1..=7).contains(&numbits) {
                self.set_n(BITS_MASK[numbits as usize], pc);
                pc += numbits as usize;
            }
        }
    }
}

#[allow(clippy::enum_glob_use)]
#[cfg(test)]
mod tests {
    use crate::evm::analysis::Bitvec;
    use crate::evm::opcode_table::opcode::*;
    use crate::evm::Buffer;

    #[test]
    fn eof_code_bitmap_test1() {
        let code = Buffer::from_slice(&[RJUMP, 0x01, 0x01, 0x01]);
        let bitvec = Bitvec::eof_code_bitmap(&code);
        assert_eq!(bitvec.to_vec()[0], 0b0000_0110);
    }

    #[test]
    fn eof_code_bitmap_test2() {
        let code = Buffer::from_slice(&[RJUMPI, RJUMP, RJUMP, RJUMPI]);
        let bitvec = Bitvec::eof_code_bitmap(&code);
        assert_eq!(bitvec.to_vec()[0], 0b0011_0110);
    }

    #[test]
    fn eof_code_bitmap_test3() {
        let code = Buffer::from_slice(&[RJUMPV, 0x02, RJUMP, 0x00, RJUMPI, 0x00]);
        let bitvec = Bitvec::eof_code_bitmap(&code);
        assert_eq!(bitvec.to_vec()[0], 0b0011_1110);
    }
}
