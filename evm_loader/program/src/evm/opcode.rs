/// <https://ethereum.github.io/yellowpaper/paper.pdf>
use ethnum::{I256, U256};
use solana_program::log::sol_log_data;

use super::{database::Database, tracing_event, Context, Machine, Reason};
use crate::{
    error::{build_revert_message, Error, Result},
    evm::Buffer,
    types::Address,
};

#[derive(Eq, PartialEq)]
pub enum Action {
    Continue,
    Jump(usize),
    Stop,
    Return(Vec<u8>),
    Revert(Vec<u8>),
    Suicide,
    Noop,
}

impl<B: Database> Machine<B> {
    /// Unknown instruction
    pub fn opcode_unknown(&mut self, _backend: &mut B) -> Result<Action> {
        Err(Error::UnknownOpcode(
            self.context.contract,
            self.execution_code[self.pc],
        ))
    }

    /// (u)int256 addition modulo 2**256
    pub fn opcode_add(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_u256()?;
        let b = self.stack.pop_u256()?;

        self.stack.push_u256(a + b)?;

        Ok(Action::Continue)
    }

    /// (u)int256 multiplication modulo 2**256
    pub fn opcode_mul(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_u256()?;
        let b = self.stack.pop_u256()?;

        self.stack.push_u256(a * b)?;

        Ok(Action::Continue)
    }

    /// (u)int256 subtraction modulo 2**256
    pub fn opcode_sub(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_u256()?;
        let b = self.stack.pop_u256()?;

        self.stack.push_u256(a - b)?;

        Ok(Action::Continue)
    }

    /// uint256 division
    pub fn opcode_div(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_u256()?;
        let b = self.stack.pop_u256()?;

        if b == U256::ZERO {
            self.stack.push_zero()?;
        } else {
            self.stack.push_u256(a / b)?;
        }

        Ok(Action::Continue)
    }

    /// int256 division
    pub fn opcode_sdiv(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_i256()?;
        let b = self.stack.pop_i256()?;

        match (a, b) {
            (_, I256::ZERO) => self.stack.push_zero()?,
            (I256::MIN, I256::MINUS_ONE) => self.stack.push_i256(I256::MIN)?,
            (a, b) => self.stack.push_i256(a / b)?,
        }

        Ok(Action::Continue)
    }

    /// uint256 modulus
    pub fn opcode_mod(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_u256()?;
        let b = self.stack.pop_u256()?;

        if b == U256::ZERO {
            self.stack.push_zero()?;
        } else {
            self.stack.push_u256(a % b)?;
        }

        Ok(Action::Continue)
    }

    /// int256 modulus
    pub fn opcode_smod(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_i256()?;
        let b = self.stack.pop_i256()?;

        if b == I256::ZERO {
            self.stack.push_zero()?;
        } else {
            self.stack.push_i256(a % b)?;
        }

        Ok(Action::Continue)
    }

    /// (u)int256 addition modulo M
    /// (a + b) % m
    /// <https://stackoverflow.com/a/11249135>
    pub fn opcode_addmod(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_u256()?;
        let b = self.stack.pop_u256()?;
        let m = self.stack.pop_u256()?;

        if m == U256::ZERO {
            self.stack.push_zero()?;
            return Ok(Action::Continue);
        }

        let result = if b == U256::ZERO {
            a % m
        } else {
            let a = a % m;
            let b = m - (b % m);

            if a >= b {
                a - b
            } else {
                m - b + a
            }
        };

        self.stack.push_u256(result)?;

        Ok(Action::Continue)
    }

    /// (u)int256 multiplication modulo M
    /// (a * b) % m
    /// <https://stackoverflow.com/a/18680280>
    pub fn opcode_mulmod(&mut self, _backend: &mut B) -> Result<Action> {
        let mut a = self.stack.pop_u256()?;
        let mut b = self.stack.pop_u256()?;
        let m = self.stack.pop_u256()?;

        if m == U256::ZERO {
            self.stack.push_zero()?;
            return Ok(Action::Continue);
        }

        if b < a {
            core::mem::swap(&mut a, &mut b);
        }

        if b >= m {
            b %= m;
        }

        let mut result = U256::ZERO;
        while a != U256::ZERO {
            if (a & 1) != U256::ZERO {
                // (result + b) % m, without overflow
                // logic is the same as in `addmod`
                if b >= (m - result) {
                    result -= m;
                }
                result += b;
            }
            a >>= 1;

            // (b + b) % m, without overflow
            let mut temp_b = b;
            if b >= (m - b) {
                temp_b -= m;
            }
            b += temp_b;
        }

        self.stack.push_u256(result)?;

        Ok(Action::Continue)
    }

    /// uint256 exponentiation modulo 2**256
    /// a ** b
    pub fn opcode_exp(&mut self, _backend: &mut B) -> Result<Action> {
        let mut a = self.stack.pop_u256()?;
        let mut b = self.stack.pop_u256()?;

        let mut result = U256::ONE;

        // exponentiation by squaring
        while b > 1 {
            if (b & 1) == 1 {
                result *= a;
            }

            b >>= 1;
            a = a * a;
        }

        // Deal with the final bit of the exponent separately, since
        // squaring the base afterwards is not necessary and may cause a
        // needless overflow.
        if b == 1 {
            result *= a;
        }

        self.stack.push_u256(result)?;

        Ok(Action::Continue)
    }

    /// sign extends x from (b + 1) * 8 bits to 256 bits.
    pub fn opcode_signextend(&mut self, _backend: &mut B) -> Result<Action> {
        let b = self.stack.pop_u256()?;
        let x = self.stack.pop_u256()?;

        let result = if b < 32_u128 {
            // `low` works since b < 32
            let bit_index = (8 * b.low() + 7) as usize;
            let bit = (x & (U256::ONE << bit_index)) != 0;
            let mask = (U256::ONE << bit_index) - U256::ONE;
            if bit {
                x | !mask
            } else {
                x & mask
            }
        } else {
            x
        };

        self.stack.push_u256(result)?;

        Ok(Action::Continue)
    }

    /// uint256 comparison
    /// a < b
    pub fn opcode_lt(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_u256()?;
        let b = self.stack.pop_u256()?;

        self.stack.push_bool(a < b)?;

        Ok(Action::Continue)
    }

    /// uint256 comparison
    /// a > b
    pub fn opcode_gt(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_u256()?;
        let b = self.stack.pop_u256()?;

        self.stack.push_bool(a > b)?;

        Ok(Action::Continue)
    }

    /// int256 comparison
    /// a < b
    pub fn opcode_slt(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_i256()?;
        let b = self.stack.pop_i256()?;
        self.stack.push_bool(a < b)?;

        Ok(Action::Continue)
    }

    /// int256 comparison
    /// a > b
    pub fn opcode_sgt(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_i256()?;
        let b = self.stack.pop_i256()?;
        self.stack.push_bool(a > b)?;

        Ok(Action::Continue)
    }

    /// (u)int256 equality
    /// a == b
    pub fn opcode_eq(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_u256()?;
        let b = self.stack.pop_u256()?;

        self.stack.push_bool(a == b)?;

        Ok(Action::Continue)
    }

    /// (u)int256 is zero
    /// a == 0
    pub fn opcode_iszero(&mut self, _backend: &mut B) -> Result<Action> {
        let result = {
            let a = self.stack.pop_array()?;
            a == &[0_u8; 32]
        };

        self.stack.push_bool(result)?;

        Ok(Action::Continue)
    }

    /// 256-bit bitwise and
    pub fn opcode_and(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_u256()?;
        let b = self.stack.pop_u256()?;

        self.stack.push_u256(a & b)?;

        Ok(Action::Continue)
    }

    /// 256-bit bitwise or
    pub fn opcode_or(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_u256()?;
        let b = self.stack.pop_u256()?;

        self.stack.push_u256(a | b)?;

        Ok(Action::Continue)
    }

    /// 256-bit bitwise xor
    pub fn opcode_xor(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_u256()?;
        let b = self.stack.pop_u256()?;

        self.stack.push_u256(a ^ b)?;

        Ok(Action::Continue)
    }

    /// 256-bit bitwise not
    pub fn opcode_not(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_u256()?;
        self.stack.push_u256(!a)?;

        Ok(Action::Continue)
    }

    /// ith byte of (u)int256 x, counting from most significant byte
    pub fn opcode_byte(&mut self, _backend: &mut B) -> Result<Action> {
        let result = {
            let i = self.stack.pop_u256()?;
            let x = self.stack.pop_array()?;

            if i >= 32 {
                0_u8
            } else {
                x[i.as_usize()]
            }
        };

        self.stack.push_byte(result)?;

        Ok(Action::Continue)
    }

    /// 256-bit shift left
    pub fn opcode_shl(&mut self, _backend: &mut B) -> Result<Action> {
        let shift = self.stack.pop_u256()?;
        let value = self.stack.pop_u256()?;

        if shift < 256 {
            self.stack.push_u256(value << shift)?;
        } else {
            self.stack.push_zero()?;
        }

        Ok(Action::Continue)
    }

    /// 256-bit shift right
    pub fn opcode_shr(&mut self, _backend: &mut B) -> Result<Action> {
        let shift = self.stack.pop_u256()?;
        let value = self.stack.pop_u256()?;

        if shift < 256 {
            self.stack.push_u256(value >> shift)?;
        } else {
            self.stack.push_zero()?;
        }

        Ok(Action::Continue)
    }

    /// arithmetic int256 shift right
    pub fn opcode_sar(&mut self, _backend: &mut B) -> Result<Action> {
        let (shift, value) = {
            let shift = self.stack.pop_u256()?;
            let value = self.stack.pop_i256()?;
            (shift, value)
        };

        if shift < 256 {
            self.stack.push_i256(value >> shift)?;
        } else {
            self.stack.push_zero()?;
        }

        Ok(Action::Continue)
    }

    /// hash = keccak256(memory[offset:offset+length])
    pub fn opcode_sha3(&mut self, _backend: &mut B) -> Result<Action> {
        use solana_program::keccak::{hash, Hash};

        let offset = self.stack.pop_usize()?;
        let length = self.stack.pop_usize()?;

        let data = self.memory.read(offset, length)?;
        let Hash(hash) = hash(data);

        self.stack.push_array(&hash)?;

        Ok(Action::Continue)
    }

    /// address of the executing contract
    pub fn opcode_address(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_address(&self.context.contract)?;

        Ok(Action::Continue)
    }

    /// address balance in wei
    pub fn opcode_balance(&mut self, backend: &mut B) -> Result<Action> {
        let balance = {
            let address = self.stack.pop_address()?;
            backend.balance(address)?
        };

        self.stack.push_u256(balance)?;

        Ok(Action::Continue)
    }

    /// transaction origin address
    /// tx.origin
    pub fn opcode_origin(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_address(&self.origin)?;

        Ok(Action::Continue)
    }

    /// message caller address
    /// msg.caller
    pub fn opcode_caller(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_address(&self.context.caller)?;

        Ok(Action::Continue)
    }

    /// message funds in wei
    /// msg.value
    pub fn opcode_callvalue(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_u256(self.context.value)?;

        Ok(Action::Continue)
    }

    /// reads a (u)int256 from message data
    /// msg.data[i:i+32]
    pub fn opcode_calldataload(&mut self, _backend: &mut B) -> Result<Action> {
        let index = self.stack.pop_usize()?;

        if let Some(buffer) = self.call_data.get(index..index + 32) {
            let buffer = arrayref::array_ref![buffer, 0, 32];
            self.stack.push_array(buffer)?;
        } else {
            let source = self.call_data.get(index..).unwrap_or(&[]);
            let len = source.len(); // len < 32

            let mut buffer = [0_u8; 32];
            buffer[..len].copy_from_slice(source);

            self.stack.push_array(&buffer)?;
        }

        Ok(Action::Continue)
    }

    /// message data length in bytes
    /// msg.data.size
    pub fn opcode_calldatasize(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_usize(self.call_data.len())?;

        Ok(Action::Continue)
    }

    /// copy message data to memory
    pub fn opcode_calldatacopy(&mut self, _backend: &mut B) -> Result<Action> {
        let memory_offset = self.stack.pop_usize()?;
        let data_offset = self.stack.pop_usize()?;
        let length = self.stack.pop_usize()?;

        self.memory
            .write_buffer(memory_offset, length, &self.call_data, data_offset)?;

        Ok(Action::Continue)
    }

    /// length of the executing contract's code in bytes
    /// address(this).code.size
    pub fn opcode_codesize(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_usize(self.execution_code.len())?;

        Ok(Action::Continue)
    }

    /// copy executing contract's bytecode
    pub fn opcode_codecopy(&mut self, _backend: &mut B) -> Result<Action> {
        let memory_offset = self.stack.pop_usize()?;
        let data_offset = self.stack.pop_usize()?;
        let length = self.stack.pop_usize()?;

        if data_offset.saturating_add(length) > self.execution_code.len() {
            return Err(Error::CodeCopyOffsetExceedsCodeSize(data_offset, length));
        }

        self.memory
            .write_buffer(memory_offset, length, &self.execution_code, data_offset)?;

        Ok(Action::Continue)
    }

    /// gas price of the executing transaction, in wei per unit of gas
    /// tx.gasprice
    pub fn opcode_gasprice(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_u256(self.gas_price)?;

        Ok(Action::Continue)
    }

    /// length of the contract bytecode at addr, in bytes
    /// address(addr).code.size
    pub fn opcode_extcodesize(&mut self, backend: &mut B) -> Result<Action> {
        let code_size = {
            let address = self.stack.pop_address()?;
            backend.code_size(address)?
        };

        self.stack.push_usize(code_size)?;

        Ok(Action::Continue)
    }

    /// copy contract's bytecode
    pub fn opcode_extcodecopy(&mut self, backend: &mut B) -> Result<Action> {
        let address = *self.stack.pop_address()?;
        let memory_offset = self.stack.pop_usize()?;
        let data_offset = self.stack.pop_usize()?;
        let length = self.stack.pop_usize()?;

        let code = backend.code(&address)?;

        if data_offset.saturating_add(length) > code.len() {
            return Err(Error::CodeCopyOffsetExceedsCodeSize(data_offset, length));
        }

        self.memory
            .write_buffer(memory_offset, length, &code, data_offset)?;

        Ok(Action::Continue)
    }

    /// Byzantium hardfork, EIP-211: the size of the returned data from the last external call, in bytes
    pub fn opcode_returndatasize(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_usize(self.return_data.len())?;

        Ok(Action::Continue)
    }

    /// Byzantium hardfork, EIP-211: copy returned data
    pub fn opcode_returndatacopy(&mut self, _backend: &mut B) -> Result<Action> {
        let memory_offset = self.stack.pop_usize()?;
        let data_offset = self.stack.pop_usize()?;
        let length = self.stack.pop_usize()?;

        self.memory
            .write_buffer(memory_offset, length, &self.return_data, data_offset)?;

        Ok(Action::Continue)
    }

    /// Constantinople hardfork, EIP-1052: hash of the contract bytecode at addr
    pub fn opcode_extcodehash(&mut self, backend: &mut B) -> Result<Action> {
        let code_hash = {
            let address = self.stack.pop_address()?;
            backend.code_hash(address)?
        };

        self.stack.push_array(&code_hash)?;

        Ok(Action::Continue)
    }

    /// hash of the specific block, only valid for the 256 most recent blocks, excluding the current one
    /// Solana limits to 150 most recent blocks
    pub fn opcode_blockhash(&mut self, backend: &mut B) -> Result<Action> {
        let block_hash = {
            let block_number = self.stack.pop_u256()?;
            backend.block_hash(block_number)?
        };

        self.stack.push_array(&block_hash)?;

        Ok(Action::Continue)
    }

    /// address of the current block's miner
    /// NOT SUPPORTED
    pub fn opcode_coinbase(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_zero()?;

        Ok(Action::Continue)
    }

    /// current block's Unix timestamp in seconds
    pub fn opcode_timestamp(&mut self, backend: &mut B) -> Result<Action> {
        let timestamp = backend.block_timestamp()?;

        self.stack.push_u256(timestamp)?;

        Ok(Action::Continue)
    }

    /// current block's number
    pub fn opcode_number(&mut self, backend: &mut B) -> Result<Action> {
        let block_number = backend.block_number()?;

        self.stack.push_u256(block_number)?;

        Ok(Action::Continue)
    }

    /// current block's difficulty
    /// NOT SUPPORTED
    pub fn opcode_difficulty(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_zero()?;

        Ok(Action::Continue)
    }

    /// current block's gas limit
    /// NOT SUPPORTED
    pub fn opcode_gaslimit(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_u256(U256::MAX)?;

        Ok(Action::Continue)
    }

    /// Istanbul hardfork, EIP-1344: current network's chain id
    pub fn opcode_chainid(&mut self, backend: &mut B) -> Result<Action> {
        let chain_id = backend.chain_id();

        self.stack.push_u256(chain_id)?;

        Ok(Action::Continue)
    }

    /// Istanbul hardfork, EIP-1884: balance of the executing contract in wei
    pub fn opcode_selfbalance(&mut self, backend: &mut B) -> Result<Action> {
        let balance = backend.balance(&self.context.contract)?;

        self.stack.push_u256(balance)?;

        Ok(Action::Continue)
    }

    /// London hardfork, EIP-3198: current block's base fee
    /// NOT SUPPORTED
    pub fn opcode_basefee(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_zero()?;

        Ok(Action::Continue)
    }

    /// pops a (u)int256 off the stack and discards it
    pub fn opcode_pop(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.discard()?;

        Ok(Action::Continue)
    }

    /// reads a (u)int256 from memory
    pub fn opcode_mload(&mut self, _backend: &mut B) -> Result<Action> {
        let offset = self.stack.pop_usize()?;
        let value = self.memory.read_32(offset)?;

        self.stack.push_array(value)?;

        Ok(Action::Continue)
    }

    /// writes a (u)int256 to memory
    pub fn opcode_mstore(&mut self, _backend: &mut B) -> Result<Action> {
        let offset = self.stack.pop_usize()?;
        let value = self.stack.pop_array()?;

        self.memory.write_32(offset, value)?;

        Ok(Action::Continue)
    }

    /// writes a uint8 to memory
    pub fn opcode_mstore8(&mut self, _backend: &mut B) -> Result<Action> {
        let offset = self.stack.pop_usize()?;
        let value = self.stack.pop_array()?;

        self.memory.write_byte(offset, value[31])?;

        Ok(Action::Continue)
    }

    /// reads a (u)int256 from storage
    pub fn opcode_sload(&mut self, backend: &mut B) -> Result<Action> {
        let index = self.stack.pop_u256()?;
        let value = backend.storage(&self.context.contract, &index)?;

        tracing_event!(super::tracing::Event::StorageAccess { index, value });

        self.stack.push_array(&value)?;

        Ok(Action::Continue)
    }

    /// writes a (u)int256 to storage
    pub fn opcode_sstore(&mut self, backend: &mut B) -> Result<Action> {
        if self.is_static {
            return Err(Error::StaticModeViolation(self.context.contract));
        }

        let index = self.stack.pop_u256()?;
        let value = *self.stack.pop_array()?;

        tracing_event!(super::tracing::Event::StorageSet { index, value });
        tracing_event!(super::tracing::Event::StorageAccess { index, value });

        backend.set_storage(self.context.contract, index, value)?;

        Ok(Action::Continue)
    }

    /// unconditional jump
    pub fn opcode_jump(&mut self, _backend: &mut B) -> Result<Action> {
        const JUMPDEST: u8 = 0x5B;

        let value = self.stack.pop_usize()?;

        if self.execution_code.get(value) == Some(&JUMPDEST) {
            Ok(Action::Jump(value))
        } else {
            Err(Error::InvalidJump(self.context.contract, value))
        }
    }

    /// conditional jump
    pub fn opcode_jumpi(&mut self, _backend: &mut B) -> Result<Action> {
        const JUMPDEST: u8 = 0x5B;

        let value = self.stack.pop_usize()?;
        let condition = self.stack.pop_array()?;

        if condition == &[0_u8; 32] {
            return Ok(Action::Continue);
        }

        if self.execution_code.get(value) == Some(&JUMPDEST) {
            Ok(Action::Jump(value))
        } else {
            Err(Error::InvalidJump(self.context.contract, value))
        }
    }

    /// program counter
    pub fn opcode_pc(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_usize(self.pc)?;

        Ok(Action::Continue)
    }

    /// memory size
    pub fn opcode_msize(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_usize(self.memory.size())?;

        Ok(Action::Continue)
    }

    /// remaining gas
    pub fn opcode_gas(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_u256(self.gas_limit)?;

        Ok(Action::Continue)
    }

    /// metadata to annotate possible jump destinations
    pub fn opcode_jumpdest(&mut self, _backend: &mut B) -> Result<Action> {
        Ok(Action::Continue)
    }

    /// Place 1 byte item on stack
    /// ~50% of contract bytecode are PUSH opcodes
    pub fn opcode_push_1(&mut self, _backend: &mut B) -> Result<Action> {
        if self.execution_code.len() <= self.pc + 1 {
            return Err(Error::PushOutOfBounds(self.context.contract));
        }

        let value = unsafe { *self.execution_code.get_unchecked(self.pc + 1) };

        self.stack.push_byte(value)?;

        Ok(Action::Jump(self.pc + 1 + 1))
    }

    /// Place 2-31 byte item on stack.
    pub fn opcode_push_2_31<const N: usize>(&mut self, _backend: &mut B) -> Result<Action> {
        if self.execution_code.len() <= self.pc + 1 + N {
            return Err(Error::PushOutOfBounds(self.context.contract));
        }

        let value = unsafe {
            let ptr = self.execution_code.as_ptr().add(self.pc + 1);
            &*ptr.cast::<[u8; N]>()
        };

        self.stack.push_array_2_31(value)?;

        Ok(Action::Jump(self.pc + 1 + N))
    }

    /// Place 32 byte item on stack
    pub fn opcode_push_32(&mut self, _backend: &mut B) -> Result<Action> {
        if self.execution_code.len() <= self.pc + 1 + 32 {
            return Err(Error::PushOutOfBounds(self.context.contract));
        }

        let value = unsafe {
            let ptr = self.execution_code.as_ptr().add(self.pc + 1);
            &*ptr.cast::<[u8; 32]>()
        };

        self.stack.push_array(value)?;

        Ok(Action::Jump(self.pc + 1 + 32))
    }

    /// Duplicate Nth stack item
    /// ~25% of contract bytecode are DUP and SWAP opcodes
    pub fn opcode_dup_1_16<const N: usize>(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.dup_1_16::<N>()?;

        Ok(Action::Continue)
    }

    /// Exchange 1st and (N+1)th stack item
    pub fn opcode_swap_1_16<const N: usize>(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.swap_1_16::<N>()?;

        Ok(Action::Continue)
    }

    /// Append log record with N topics
    #[rustfmt::skip]
    pub fn opcode_log_0_4<const N: usize>(&mut self, _backend: &mut B) -> Result<Action> {
        if self.is_static {
            return Err(Error::StaticModeViolation(self.context.contract));
        }

        let offset = self.stack.pop_usize()?;
        let length = self.stack.pop_usize()?;

        let data = self.memory.read(offset, length)?;
        let topics: [[u8; 32]; N] = {
            let mut topics = [[0_u8; 32]; N];
            for topic in &mut topics {
                *topic = *self.stack.pop_array()?;
            }

            topics
        };

        let address = self.context.contract.as_bytes();

        match N {
            0 => sol_log_data(&[b"LOG0", address, &[0], data]),                                                
            1 => sol_log_data(&[b"LOG1", address, &[1], &topics[0], data]),                                    
            2 => sol_log_data(&[b"LOG2", address, &[2], &topics[0], &topics[1], data]),                        
            3 => sol_log_data(&[b"LOG3", address, &[3], &topics[0], &topics[1], &topics[2], data]),            
            4 => sol_log_data(&[b"LOG4", address, &[4], &topics[0], &topics[1], &topics[2], &topics[3], data]),
            _ => unreachable!(),
        }

        Ok(Action::Continue)
    }

    /// Create a new account with associated code.
    pub fn opcode_create(&mut self, backend: &mut B) -> Result<Action> {
        if self.is_static {
            return Err(Error::StaticModeViolation(self.context.contract));
        }

        let value = self.stack.pop_u256()?;
        let offset = self.stack.pop_usize()?;
        let length = self.stack.pop_usize()?;

        let created_address = {
            let nonce = backend.nonce(&self.context.contract)?;
            Address::from_create(&self.context.contract, nonce)
        };

        sol_log_data(&[b"ENTER", b"CREATE", created_address.as_bytes()]);

        self.opcode_create_impl(created_address, value, offset, length, backend)
    }

    /// Constantinople harfork, EIP-1014: creates a create a new account with a deterministic address
    pub fn opcode_create2(&mut self, backend: &mut B) -> Result<Action> {
        if self.is_static {
            return Err(Error::StaticModeViolation(self.context.contract));
        }

        let value = self.stack.pop_u256()?;
        let offset = self.stack.pop_usize()?;
        let length = self.stack.pop_usize()?;
        let salt = *self.stack.pop_array()?;

        let created_address = {
            let initialization_code = self.memory.read(offset, length)?;
            Address::from_create2(&self.context.contract, &salt, initialization_code)
        };

        sol_log_data(&[b"ENTER", b"CREATE2", created_address.as_bytes()]);

        self.opcode_create_impl(created_address, value, offset, length, backend)
    }

    fn opcode_create_impl(
        &mut self,
        address: Address,
        value: U256,
        offset: usize,
        length: usize,
        backend: &mut B,
    ) -> Result<Action> {
        if backend.nonce(&self.context.contract)? == u64::MAX {
            return Err(Error::NonceOverflow(self.context.contract));
        }

        if (backend.nonce(&address)? != 0) || (backend.code_size(&address)? != 0) {
            // return Err(Error::DeployToExistingAccount(address, self.context.contract));
            self.stack.push_zero()?;
            return Ok(Action::Continue);
        }

        if backend.balance(&self.context.contract)? < value {
            // return Err(Error::InsufficientBalanceForTransfer(self.context.contract, value));
            self.stack.push_zero()?;
            return Ok(Action::Continue);
        }

        backend.increment_nonce(address)?;
        backend.snapshot()?;

        backend.increment_nonce(self.context.contract)?;
        backend.transfer(self.context.contract, address, value)?;

        let context = Context {
            caller: self.context.contract,
            contract: address,
            value,
            code_address: None,
        };
        let init_code = Buffer::new(self.memory.read(offset, length)?);

        tracing_event!(super::tracing::Event::BeginVM {
            context,
            code: init_code.to_vec()
        });

        self.fork(Reason::Create, context, init_code, Buffer::empty(), None);

        Ok(Action::Noop)
    }

    /// Message-call into an account
    pub fn opcode_call(&mut self, backend: &mut B) -> Result<Action> {
        let gas_limit = self.stack.pop_u256()?;
        let address = *self.stack.pop_address()?;
        let value = self.stack.pop_u256()?;
        let args_offset = self.stack.pop_usize()?;
        let args_length = self.stack.pop_usize()?;
        // let return_offset = self.stack.pop_usize()?;
        // let return_length = self.stack.pop_usize()?;

        if self.is_static && (value != U256::ZERO) {
            return Err(Error::StaticModeViolation(self.context.contract));
        }

        if backend.balance(&self.context.contract)? < value {
            self.stack.discard()?; // return_offset
            self.stack.discard()?; // return_length
            self.stack.push_bool(false)?; // fail

            self.return_data = Buffer::empty();

            return Ok(Action::Continue);
        }

        let context = Context {
            caller: self.context.contract,
            contract: address,
            value,
            code_address: Some(address),
        };
        let call_data = Buffer::new(self.memory.read(args_offset, args_length)?);

        let precompile_result =
            self.opcode_call_precompile_impl(backend, &context, &address, &call_data, false)?;
        if precompile_result != Action::Noop {
            return Ok(precompile_result);
        }

        backend.snapshot()?;
        backend.transfer(self.context.contract, address, value)?;

        let execution_code = backend.code(&address)?;

        tracing_event!(super::tracing::Event::BeginVM {
            context,
            code: execution_code.to_vec()
        });

        sol_log_data(&[b"ENTER", b"CALL", address.as_bytes()]);

        self.fork(
            Reason::Call,
            context,
            execution_code,
            call_data,
            Some(gas_limit),
        );

        Ok(Action::Noop)
    }

    /// Message-call into this account with an alternative account’s code
    pub fn opcode_callcode(&mut self, backend: &mut B) -> Result<Action> {
        let gas_limit = self.stack.pop_u256()?;
        let address = *self.stack.pop_address()?;
        let value = self.stack.pop_u256()?;
        let args_offset = self.stack.pop_usize()?;
        let args_length = self.stack.pop_usize()?;
        // let return_offset = self.stack.pop_usize()?;
        // let return_length = self.stack.pop_usize()?;

        if backend.balance(&self.context.contract)? < value {
            self.stack.discard()?; // return_offset
            self.stack.discard()?; // return_length
            self.stack.push_bool(false)?; // fail

            self.return_data = Buffer::empty();

            return Ok(Action::Continue);
        }

        let context = Context {
            caller: self.context.contract,
            contract: self.context.contract,
            value,
            code_address: Some(address),
        };
        let call_data = Buffer::new(self.memory.read(args_offset, args_length)?);

        let precompile_result =
            self.opcode_call_precompile_impl(backend, &context, &address, &call_data, false)?;
        if precompile_result != Action::Noop {
            return Ok(precompile_result);
        }

        backend.snapshot()?;
        // no need to transfer funds to yourself

        let execution_code = backend.code(&address)?;

        tracing_event!(super::tracing::Event::BeginVM {
            context,
            code: execution_code.to_vec()
        });

        sol_log_data(&[b"ENTER", b"CALLCODE", address.as_bytes()]);

        self.fork(
            Reason::Call,
            context,
            execution_code,
            call_data,
            Some(gas_limit),
        );

        Ok(Action::Noop)
    }

    /// Homestead hardfork, EIP-7: Message-call into this account with an alternative account’s code,
    /// but persisting the current values for sender and value
    pub fn opcode_delegatecall(&mut self, backend: &mut B) -> Result<Action> {
        let gas_limit = self.stack.pop_u256()?;
        let address = *self.stack.pop_address()?;
        let args_offset = self.stack.pop_usize()?;
        let args_length = self.stack.pop_usize()?;
        // let return_offset = self.stack.pop_usize()?;
        // let return_length = self.stack.pop_usize()?;

        let context = self.context;
        let call_data = Buffer::new(self.memory.read(args_offset, args_length)?);

        let precompile_result =
            self.opcode_call_precompile_impl(backend, &context, &address, &call_data, false)?;
        if precompile_result != Action::Noop {
            return Ok(precompile_result);
        }

        backend.snapshot()?;

        let execution_code = backend.code(&address)?;

        tracing_event!(super::tracing::Event::BeginVM {
            context,
            code: execution_code.to_vec()
        });

        sol_log_data(&[b"ENTER", b"DELEGATECALL", address.as_bytes()]);

        self.fork(
            Reason::Call,
            context,
            execution_code,
            call_data,
            Some(gas_limit),
        );

        Ok(Action::Noop)
    }

    /// Byzantium hardfork, EIP-214: Static message-call into an account
    /// Disallowed contract creation, event emission, storage modification and contract destruction
    pub fn opcode_staticcall(&mut self, backend: &mut B) -> Result<Action> {
        let gas_limit = self.stack.pop_u256()?;
        let address = *self.stack.pop_address()?;
        let args_offset = self.stack.pop_usize()?;
        let args_length = self.stack.pop_usize()?;
        // let return_offset = self.stack.pop_usize()?;
        // let return_length = self.stack.pop_usize()?;

        let context = Context {
            caller: self.context.contract,
            contract: address,
            value: U256::ZERO,
            code_address: Some(address),
        };
        let call_data = Buffer::new(self.memory.read(args_offset, args_length)?);

        let precompile_result =
            self.opcode_call_precompile_impl(backend, &context, &address, &call_data, false)?;
        if precompile_result != Action::Noop {
            return Ok(precompile_result);
        }

        backend.snapshot()?;

        let execution_code = backend.code(&address)?;

        tracing_event!(super::tracing::Event::BeginVM {
            context,
            code: execution_code.to_vec()
        });

        sol_log_data(&[b"ENTER", b"STATICCALL", address.as_bytes()]);

        self.fork(
            Reason::Call,
            context,
            execution_code,
            call_data,
            Some(gas_limit),
        );
        self.is_static = true;

        Ok(Action::Noop)
    }

    /// Call precompile contract.
    /// Returns `Action::Noop` if address is not a precompile
    fn opcode_call_precompile_impl(
        &mut self,
        backend: &mut B,
        context: &Context,
        address: &Address,
        data: &[u8],
        is_static: bool,
    ) -> Result<Action> {
        let is_static = self.is_static || is_static;

        let mut result = Self::precompile(address, data).map(Ok);
        if result.is_none() {
            result = backend.precompile_extension(context, address, data, is_static);
        }

        let result = result.map(|r| match r {
            Ok(v) => (v, true),
            Err(e) => (build_revert_message(&e.to_string()), false),
        });

        if let Some((result, status)) = result {
            self.return_data = Buffer::new(&result);

            let return_offset = self.stack.pop_usize()?;
            let return_length = self.stack.pop_usize()?;

            self.memory
                .write_buffer(return_offset, return_length, &self.return_data, 0)?;
            self.stack.push_bool(status)?;

            Ok(Action::Continue)
        } else {
            Ok(Action::Noop)
        }
    }

    /// Halt execution returning output data
    pub fn opcode_return(&mut self, backend: &mut B) -> Result<Action> {
        sol_log_data(&[b"EXIT", b"RETURN"]);

        let offset = self.stack.pop_usize()?;
        let length = self.stack.pop_usize()?;

        let return_data = Buffer::new(self.memory.read(offset, length)?);

        if self.parent.is_none() {
            match self.reason {
                Reason::Call => return Ok(Action::Return(return_data.to_vec())),
                Reason::Create => {
                    backend.set_code(self.context.contract, return_data)?;
                    return Ok(Action::Return(Vec::new()));
                }
            }
        }

        tracing_event!(super::tracing::Event::EndStep { gas_used: 0_u64 });
        tracing_event!(super::tracing::Event::EndVM {
            status: super::ExitStatus::Return(return_data.to_vec())
        });

        let returned = self.join();
        match returned.reason {
            Reason::Call => {
                let return_offset = self.stack.pop_usize()?;
                let return_length = self.stack.pop_usize()?;

                self.memory
                    .write_buffer(return_offset, return_length, &return_data, 0)?;
                self.stack.push_bool(true)?; // success

                self.return_data = return_data;
            }
            Reason::Create => {
                let address = returned.context.contract;

                backend.set_code(address, return_data)?;

                self.stack.push_address(&address)?;
            }
        }

        backend.commit_snapshot()?;

        Ok(Action::Continue)
    }

    /// Byzantium hardfork, EIP-140: Halt execution reverting state changes but returning data
    pub fn opcode_revert(&mut self, backend: &mut B) -> Result<Action> {
        sol_log_data(&[b"EXIT", b"REVERT"]);

        let offset = self.stack.pop_usize()?;
        let length = self.stack.pop_usize()?;

        let return_data = Buffer::new(self.memory.read(offset, length)?);

        backend.revert_snapshot()?;

        if self.parent.is_none() {
            return Ok(Action::Revert(return_data.to_vec()));
        }

        tracing_event!(super::tracing::Event::EndStep { gas_used: 0_u64 });
        tracing_event!(super::tracing::Event::EndVM {
            status: super::ExitStatus::Revert(return_data.to_vec())
        });

        let returned = self.join();
        match returned.reason {
            Reason::Call => {
                let return_offset = self.stack.pop_usize()?;
                let return_length = self.stack.pop_usize()?;

                self.memory
                    .write_buffer(return_offset, return_length, &return_data, 0)?;
                self.stack.push_bool(false)?; // fail

                self.return_data = return_data;
            }
            Reason::Create => {
                self.stack.push_zero()?;
            }
        }

        Ok(Action::Continue)
    }

    /// Invalid instruction
    pub fn opcode_invalid(&mut self, _backend: &mut B) -> Result<Action> {
        Err(Error::InvalidOpcode(
            self.context.contract,
            self.execution_code[self.pc],
        ))
    }

    /// Halt execution, destroys the contract and send all funds to address
    pub fn opcode_selfdestruct(&mut self, backend: &mut B) -> Result<Action> {
        sol_log_data(&[b"EXIT", b"SELFDESTRUCT"]);

        if self.is_static {
            return Err(Error::StaticModeViolation(self.context.contract));
        }

        let address = *self.stack.pop_address()?;

        let value = backend.balance(&self.context.contract)?;
        backend.transfer(self.context.contract, address, value)?;

        backend.selfdestruct(self.context.contract)?;

        if self.parent.is_none() {
            return Ok(Action::Suicide);
        }

        tracing_event!(super::tracing::Event::EndStep { gas_used: 0_u64 });
        tracing_event!(super::tracing::Event::EndVM {
            status: super::ExitStatus::Suicide
        });

        let returned = self.join();
        match returned.reason {
            Reason::Call => {
                let return_offset = self.stack.pop_usize()?;
                let return_length = self.stack.pop_usize()?;

                self.memory
                    .write_buffer(return_offset, return_length, &[], 0)?;
                self.stack.push_bool(true)?; // success

                self.return_data = Buffer::empty();
            }
            Reason::Create => {
                self.stack.push_zero()?;
            }
        }

        backend.commit_snapshot()?;

        Ok(Action::Continue)
    }

    /// Halts execution of the contract
    pub fn opcode_stop(&mut self, backend: &mut B) -> Result<Action> {
        sol_log_data(&[b"EXIT", b"STOP"]);

        if self.parent.is_none() {
            return Ok(Action::Stop);
        }

        tracing_event!(super::tracing::Event::EndStep { gas_used: 0_u64 });
        tracing_event!(super::tracing::Event::EndVM {
            status: super::ExitStatus::Stop
        });

        let returned = self.join();
        if returned.reason == Reason::Call {
            let return_offset = self.stack.pop_usize()?;
            let return_length = self.stack.pop_usize()?;

            self.memory
                .write_buffer(return_offset, return_length, &[], 0)?;
            self.stack.push_bool(true)?; // success

            self.return_data = Buffer::empty();
        }

        backend.commit_snapshot()?;

        Ok(Action::Continue)
    }
}
