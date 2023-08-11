use crate::evm::Buffer;
use crate::evm::opcode_table::OpCode::*;

pub struct Bitvec(Vec<u8>);

const SET2BITS_MASK: u16 = 0b11;
const SET3BITS_MASK: u16 = 0b111;
const SET4BITS_MASK: u16 = 0b1111;
const SET5BITS_MASK: u16 = 0b1_1111;
const SET6BITS_MASK: u16 = 0b11_1111;
const SET7BITS_MASK: u16 = 0b111_1111;

impl Bitvec {
    pub fn new(capacity: usize) -> Self {
        Bitvec(vec![0; capacity])
    }

    pub fn set1(&mut self, pos: usize) {
        self.0[pos / 8] |= 1 << (pos % 8)
    }

    pub fn set_n(&mut self, flag: u16, pos: usize) {
        let a = flag << (pos % 8);
        self.0[pos / 8] |= a as u8;
        let b = (a >> 8) as u8;
        if b != 0 {
            self.0[pos / 8 + 1] = b
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
        return ((self.0[pos / 8] >> (pos % 8)) & 1) == 0;
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

            if op >= PUSH1.u8() && op <= PUSH32.u8() {
                numbits = op - PUSH1.u8() + 1
            } else if op == RJUMP.u8() || op == RJUMPI.u8() || op == CALLF.u8() {
                numbits = 2
            } else if op == RJUMPV.u8() {
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
                    numbits = (end - pc) as u8 //todo: check overflow
                }
            } else {
                continue;
            }

            if numbits >= 8 {
                while numbits >= 16 {
                    self.set16(pc);
                    numbits -= 16;
                    pc += 16;
                };
                while numbits >= 8 {
                    self.set8(pc);
                    numbits -= 8;
                    pc += 8;
                }
            }

            match numbits {
                1 => {
                    self.set1(pc);
                    pc += 1
                }
                2 => {
                    self.set_n(SET2BITS_MASK, pc);
                    pc += 2
                }
                3 => {
                    self.set_n(SET3BITS_MASK, pc);
                    pc += 3
                }
                4 => {
                    self.set_n(SET4BITS_MASK, pc);
                    pc += 4
                }
                5 => {
                    self.set_n(SET5BITS_MASK, pc);
                    pc += 5
                }
                6 => {
                    self.set_n(SET6BITS_MASK, pc);
                    pc += 6
                }
                7 => {
                    self.set_n(SET7BITS_MASK, pc);
                    pc += 7
                }
                _ => ()
            };
        }
    }
}


#[cfg(test)]
mod tests {
    use crate::evm::analysis::Bitvec;
    use crate::evm::Buffer;
    use crate::evm::opcode_table::OpCode::*;

    #[test]
    fn eof_code_bitmap_test1() {
        let code = Buffer::from_slice(&[
            RJUMP.u8(), 0x01, 0x01, 0x01
        ]);
        let bitvec = Bitvec::eof_code_bitmap(&code);
        assert_eq!(bitvec.to_vec()[0], 0b0000_0110);
    }

    #[test]
    fn eof_code_bitmap_test2() {
        let code = Buffer::from_slice(&[
            RJUMPI.u8(),
            RJUMP.u8(),
            RJUMP.u8(),
            RJUMPI.u8()
        ]);
        let bitvec = Bitvec::eof_code_bitmap(&code);
        assert_eq!(bitvec.to_vec()[0], 0b0011_0110);
    }

    #[test]
    fn eof_code_bitmap_test3() {
        let code = Buffer::from_slice(&[
            RJUMPV.u8(),
            0x02,
            RJUMP.u8(),
            0x00,
            RJUMPI.u8(),
            0x00,
        ]);
        let bitvec = Bitvec::eof_code_bitmap(&code);
        assert_eq!(bitvec.to_vec()[0], 0b0011_1110);
    }
}
