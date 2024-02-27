#![allow(clippy::needless_pass_by_ref_mut)]

/// <https://ethereum.github.io/yellowpaper/paper.pdf>
use ethnum::{I256, U256};
use maybe_async::maybe_async;

use super::{
    database::{Database, DatabaseExt},
    tracing_event, Context, Machine, Reason,
};
use crate::evm::tracing::EventListener;
use crate::{
    debug::log_data,
    error::{Error, Result},
    evm::{trace_end_step, Buffer},
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

#[allow(clippy::unused_async)]
impl<B: Database, T: EventListener> Machine<B, T> {
    /// Unknown instruction
    #[maybe_async]
    pub async fn opcode_unknown(&mut self, _backend: &mut B) -> Result<Action> {
        Err(Error::UnknownOpcode(
            self.context.contract,
            self.execution_code[self.pc],
        ))
    }

    /// (u)int256 addition modulo 2**256
    #[maybe_async]
    pub async fn opcode_add(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_u256()?;
        let b = self.stack.pop_u256()?;
        let c = a.wrapping_add(b);

        self.stack.push_u256(c)?;

        Ok(Action::Continue)
    }

    /// (u)int256 multiplication modulo 2**256
    #[maybe_async]
    pub async fn opcode_mul(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_u256()?;
        let b = self.stack.pop_u256()?;
        let c = a.wrapping_mul(b);

        self.stack.push_u256(c)?;

        Ok(Action::Continue)
    }

    /// (u)int256 subtraction modulo 2**256
    #[maybe_async]
    pub async fn opcode_sub(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_u256()?;
        let b = self.stack.pop_u256()?;
        let c = a.wrapping_sub(b);

        self.stack.push_u256(c)?;

        Ok(Action::Continue)
    }

    /// uint256 division
    #[maybe_async]
    pub async fn opcode_div(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_u256()?;
        let b = self.stack.pop_u256()?;

        if b == U256::ZERO {
            self.stack.push_zero()?;
        } else {
            let c = a.wrapping_div(b);
            self.stack.push_u256(c)?;
        }

        Ok(Action::Continue)
    }

    /// int256 division
    #[maybe_async]
    pub async fn opcode_sdiv(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_i256()?;
        let b = self.stack.pop_i256()?;

        if b == I256::ZERO {
            self.stack.push_zero()?;
        } else {
            // Wrapping occurs when dividing MIN / -1, in which case c = I256::MIN
            let c = a.wrapping_div(b);
            self.stack.push_i256(c)?;
        }

        Ok(Action::Continue)
    }

    /// uint256 modulus
    #[maybe_async]
    pub async fn opcode_mod(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_u256()?;
        let b = self.stack.pop_u256()?;

        if b == U256::ZERO {
            self.stack.push_zero()?;
        } else {
            let c = a.wrapping_rem(b);
            self.stack.push_u256(c)?;
        }

        Ok(Action::Continue)
    }

    /// int256 modulus
    #[maybe_async]
    pub async fn opcode_smod(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_i256()?;
        let b = self.stack.pop_i256()?;

        if b == I256::ZERO {
            self.stack.push_zero()?;
        } else {
            let c = a.wrapping_rem(b);
            self.stack.push_i256(c)?;
        }

        Ok(Action::Continue)
    }

    /// (u)int256 addition modulo M
    /// (a + b) % m
    /// <https://stackoverflow.com/a/11249135>
    #[maybe_async]
    pub async fn opcode_addmod(&mut self, _backend: &mut B) -> Result<Action> {
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
                (m - b).wrapping_add(a)
            }
        };

        self.stack.push_u256(result)?;

        Ok(Action::Continue)
    }

    /// (u)int256 multiplication modulo M
    /// (a * b) % m
    /// <https://stackoverflow.com/a/18680280>
    #[maybe_async]
    pub async fn opcode_mulmod(&mut self, _backend: &mut B) -> Result<Action> {
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
                if b >= m.wrapping_sub(result) {
                    result = result.wrapping_sub(m);
                }
                result = result.wrapping_add(b);
            }
            a >>= 1;

            // (b + b) % m, without overflow
            let mut temp_b = b;
            if b >= m.wrapping_sub(b) {
                temp_b = temp_b.wrapping_sub(m);
            }
            b = b.wrapping_add(temp_b);
        }

        self.stack.push_u256(result)?;

        Ok(Action::Continue)
    }

    /// uint256 exponentiation modulo 2**256
    /// a ** b
    #[maybe_async]
    pub async fn opcode_exp(&mut self, _backend: &mut B) -> Result<Action> {
        let mut a = self.stack.pop_u256()?;
        let mut b = self.stack.pop_u256()?;

        let mut result = U256::ONE;

        // exponentiation by squaring
        while b > 1 {
            if (b & 1) == 1 {
                result = result.wrapping_mul(a);
            }

            b >>= 1;
            a = a.wrapping_mul(a);
        }

        // Deal with the final bit of the exponent separately, since
        // squaring the base afterwards is not necessary and may cause a
        // needless overflow.
        if b == 1 {
            result = result.wrapping_mul(a);
        }

        self.stack.push_u256(result)?;

        Ok(Action::Continue)
    }

    /// sign extends x from (b + 1) * 8 bits to 256 bits.
    #[maybe_async]
    pub async fn opcode_signextend(&mut self, _backend: &mut B) -> Result<Action> {
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
    #[maybe_async]
    pub async fn opcode_lt(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_u256()?;
        let b = self.stack.pop_u256()?;

        self.stack.push_bool(a < b)?;

        Ok(Action::Continue)
    }

    /// uint256 comparison
    /// a > b
    #[maybe_async]
    pub async fn opcode_gt(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_u256()?;
        let b = self.stack.pop_u256()?;

        self.stack.push_bool(a > b)?;

        Ok(Action::Continue)
    }

    /// int256 comparison
    /// a < b
    #[maybe_async]
    pub async fn opcode_slt(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_i256()?;
        let b = self.stack.pop_i256()?;
        self.stack.push_bool(a < b)?;

        Ok(Action::Continue)
    }

    /// int256 comparison
    /// a > b
    #[maybe_async]
    pub async fn opcode_sgt(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_i256()?;
        let b = self.stack.pop_i256()?;
        self.stack.push_bool(a > b)?;

        Ok(Action::Continue)
    }

    /// (u)int256 equality
    /// a == b
    #[maybe_async]
    pub async fn opcode_eq(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_u256()?;
        let b = self.stack.pop_u256()?;

        self.stack.push_bool(a == b)?;

        Ok(Action::Continue)
    }

    /// (u)int256 is zero
    /// a == 0
    #[maybe_async]
    pub async fn opcode_iszero(&mut self, _backend: &mut B) -> Result<Action> {
        let result = {
            let a = self.stack.pop_array()?;
            a == &[0_u8; 32]
        };

        self.stack.push_bool(result)?;

        Ok(Action::Continue)
    }

    /// 256-bit bitwise and
    #[maybe_async]
    pub async fn opcode_and(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_u256()?;
        let b = self.stack.pop_u256()?;

        self.stack.push_u256(a & b)?;

        Ok(Action::Continue)
    }

    /// 256-bit bitwise or
    #[maybe_async]
    pub async fn opcode_or(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_u256()?;
        let b = self.stack.pop_u256()?;

        self.stack.push_u256(a | b)?;

        Ok(Action::Continue)
    }

    /// 256-bit bitwise xor
    #[maybe_async]
    pub async fn opcode_xor(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_u256()?;
        let b = self.stack.pop_u256()?;

        self.stack.push_u256(a ^ b)?;

        Ok(Action::Continue)
    }

    /// 256-bit bitwise not
    #[maybe_async]
    pub async fn opcode_not(&mut self, _backend: &mut B) -> Result<Action> {
        let a = self.stack.pop_u256()?;
        self.stack.push_u256(!a)?;

        Ok(Action::Continue)
    }

    /// ith byte of (u)int256 x, counting from most significant byte
    #[maybe_async]
    pub async fn opcode_byte(&mut self, _backend: &mut B) -> Result<Action> {
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
    #[maybe_async]
    pub async fn opcode_shl(&mut self, _backend: &mut B) -> Result<Action> {
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
    #[maybe_async]
    pub async fn opcode_shr(&mut self, _backend: &mut B) -> Result<Action> {
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
    #[maybe_async]
    pub async fn opcode_sar(&mut self, _backend: &mut B) -> Result<Action> {
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
    #[maybe_async]
    pub async fn opcode_sha3(&mut self, _backend: &mut B) -> Result<Action> {
        use solana_program::keccak::{hash, Hash};

        let offset = self.stack.pop_usize()?;
        let length = self.stack.pop_usize()?;

        let data = self.memory.read(offset, length)?;
        let Hash(hash) = hash(data);

        self.stack.push_array(&hash)?;

        Ok(Action::Continue)
    }

    /// address of the executing contract
    #[maybe_async]
    pub async fn opcode_address(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_address(&self.context.contract)?;

        Ok(Action::Continue)
    }

    /// address balance in wei
    #[maybe_async]
    pub async fn opcode_balance(&mut self, backend: &mut B) -> Result<Action> {
        let balance = {
            let address = self.stack.pop_address()?;
            backend.balance(address, self.chain_id).await?
        };

        self.stack.push_u256(balance)?;

        Ok(Action::Continue)
    }

    /// transaction origin address
    /// tx.origin
    #[maybe_async]
    pub async fn opcode_origin(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_address(&self.origin)?;

        Ok(Action::Continue)
    }

    /// message caller address
    /// msg.caller
    #[maybe_async]
    pub async fn opcode_caller(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_address(&self.context.caller)?;

        Ok(Action::Continue)
    }

    /// message funds in wei
    /// msg.value
    #[maybe_async]
    pub async fn opcode_callvalue(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_u256(self.context.value)?;

        Ok(Action::Continue)
    }

    /// reads a (u)int256 from message data
    /// msg.data[i:i+32]
    #[maybe_async]
    pub async fn opcode_calldataload(&mut self, _backend: &mut B) -> Result<Action> {
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
    #[maybe_async]
    pub async fn opcode_calldatasize(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_usize(self.call_data.len())?;

        Ok(Action::Continue)
    }

    /// copy message data to memory
    #[maybe_async]
    pub async fn opcode_calldatacopy(&mut self, _backend: &mut B) -> Result<Action> {
        let memory_offset = self.stack.pop_usize()?;
        let data_offset = self.stack.pop_usize()?;
        let length = self.stack.pop_usize()?;

        self.memory
            .write_buffer(memory_offset, length, &self.call_data, data_offset)?;

        Ok(Action::Continue)
    }

    /// length of the executing contract's code in bytes
    /// address(this).code.size
    #[maybe_async]
    pub async fn opcode_codesize(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_usize(self.execution_code.len())?;

        Ok(Action::Continue)
    }

    /// copy executing contract's bytecode
    #[maybe_async]
    pub async fn opcode_codecopy(&mut self, _backend: &mut B) -> Result<Action> {
        let memory_offset = self.stack.pop_usize()?;
        let data_offset = self.stack.pop_usize()?;
        let length = self.stack.pop_usize()?;

        self.memory
            .write_buffer(memory_offset, length, &self.execution_code, data_offset)?;

        Ok(Action::Continue)
    }

    /// gas price of the executing transaction, in wei per unit of gas
    /// tx.gasprice
    #[maybe_async]
    pub async fn opcode_gasprice(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_u256(self.gas_price)?;

        Ok(Action::Continue)
    }

    /// length of the contract bytecode at addr, in bytes
    /// address(addr).code.size
    #[maybe_async]
    pub async fn opcode_extcodesize(&mut self, backend: &mut B) -> Result<Action> {
        let code_size = {
            let address = self.stack.pop_address()?;
            backend.code_size(address).await?
        };

        self.stack.push_usize(code_size)?;

        Ok(Action::Continue)
    }

    /// copy contract's bytecode
    #[maybe_async]
    pub async fn opcode_extcodecopy(&mut self, backend: &mut B) -> Result<Action> {
        let address = self.stack.pop_address()?;
        let memory_offset = self.stack.pop_usize()?;
        let data_offset = self.stack.pop_usize()?;
        let length = self.stack.pop_usize()?;

        let code = backend.code(address).await?;

        self.memory
            .write_buffer(memory_offset, length, &code, data_offset)?;

        Ok(Action::Continue)
    }

    /// Byzantium hardfork, EIP-211: the size of the returned data from the last external call, in bytes
    #[maybe_async]
    pub async fn opcode_returndatasize(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_usize(self.return_data.len())?;

        Ok(Action::Continue)
    }

    /// Byzantium hardfork, EIP-211: copy returned data
    #[maybe_async]
    pub async fn opcode_returndatacopy(&mut self, _backend: &mut B) -> Result<Action> {
        let memory_offset = self.stack.pop_usize()?;
        let data_offset = self.stack.pop_usize()?;
        let length = self.stack.pop_usize()?;

        if data_offset.saturating_add(length) > self.return_data.len() {
            return Err(Error::ReturnDataCopyOverflow(data_offset, length));
        }

        self.memory
            .write_buffer(memory_offset, length, &self.return_data, data_offset)?;

        Ok(Action::Continue)
    }

    /// Constantinople hardfork, EIP-1052: hash of the contract bytecode at addr
    #[maybe_async]
    pub async fn opcode_extcodehash(&mut self, backend: &mut B) -> Result<Action> {
        let code_hash = {
            let address = self.stack.pop_address()?;
            backend.code_hash(address, self.chain_id).await?
        };

        self.stack.push_array(&code_hash)?;

        Ok(Action::Continue)
    }

    /// hash of the specific block, only valid for the 256 most recent blocks, excluding the current one
    /// Solana limits to 150 most recent blocks
    #[maybe_async]
    pub async fn opcode_blockhash(&mut self, backend: &mut B) -> Result<Action> {
        let block_hash = {
            let block_number = self.stack.pop_u256()?;

            backend.block_hash(block_number).await?
        };

        self.stack.push_array(&block_hash)?;

        Ok(Action::Continue)
    }

    /// address of the current block's miner
    /// NOT SUPPORTED
    #[maybe_async]
    pub async fn opcode_coinbase(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_zero()?;

        Ok(Action::Continue)
    }

    /// current block's Unix timestamp in seconds
    #[maybe_async]
    pub async fn opcode_timestamp(&mut self, backend: &mut B) -> Result<Action> {
        let timestamp = backend.block_timestamp()?;

        self.stack.push_u256(timestamp)?;

        Ok(Action::Continue)
    }

    /// current block's number
    #[maybe_async]
    pub async fn opcode_number(&mut self, backend: &mut B) -> Result<Action> {
        let block_number = backend.block_number()?;

        self.stack.push_u256(block_number)?;

        Ok(Action::Continue)
    }

    /// current block's difficulty
    /// NOT SUPPORTED
    #[maybe_async]
    pub async fn opcode_difficulty(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_zero()?;

        Ok(Action::Continue)
    }

    /// current block's gas limit
    /// NOT SUPPORTED
    #[maybe_async]
    pub async fn opcode_gaslimit(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_u256(U256::MAX)?;

        Ok(Action::Continue)
    }

    /// Istanbul hardfork, EIP-1344: current network's chain id
    #[maybe_async]
    pub async fn opcode_chainid(&mut self, _backend: &mut B) -> Result<Action> {
        let chain_id = self.chain_id.into();

        self.stack.push_u256(chain_id)?;

        Ok(Action::Continue)
    }

    /// Istanbul hardfork, EIP-1884: balance of the executing contract in wei
    #[maybe_async]
    pub async fn opcode_selfbalance(&mut self, backend: &mut B) -> Result<Action> {
        let balance = backend
            .balance(self.context.contract, self.chain_id)
            .await?;

        self.stack.push_u256(balance)?;

        Ok(Action::Continue)
    }

    /// London hardfork, EIP-3198: current block's base fee
    /// NOT SUPPORTED
    #[maybe_async]
    pub async fn opcode_basefee(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_zero()?;

        Ok(Action::Continue)
    }

    /// pops a (u)int256 off the stack and discards it
    #[maybe_async]
    pub async fn opcode_pop(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.discard()?;

        Ok(Action::Continue)
    }

    /// reads a (u)int256 from memory
    #[maybe_async]
    pub async fn opcode_mload(&mut self, _backend: &mut B) -> Result<Action> {
        let offset = self.stack.pop_usize()?;
        let value = self.memory.read_32(offset)?;

        self.stack.push_array(value)?;

        Ok(Action::Continue)
    }

    /// writes a (u)int256 to memory
    #[maybe_async]
    pub async fn opcode_mstore(&mut self, _backend: &mut B) -> Result<Action> {
        let offset = self.stack.pop_usize()?;
        let value = self.stack.pop_array()?;

        self.memory.write_32(offset, value)?;

        Ok(Action::Continue)
    }

    /// writes a uint8 to memory
    #[maybe_async]
    pub async fn opcode_mstore8(&mut self, _backend: &mut B) -> Result<Action> {
        let offset = self.stack.pop_usize()?;
        let value = self.stack.pop_array()?;

        self.memory.write_byte(offset, value[31])?;

        Ok(Action::Continue)
    }

    /// reads a (u)int256 from storage
    #[maybe_async]
    pub async fn opcode_sload(&mut self, backend: &mut B) -> Result<Action> {
        let index = self.stack.pop_u256()?;
        let value = backend.storage(self.context.contract, index).await?;

        tracing_event!(
            self,
            backend,
            super::tracing::Event::StorageAccess { index, value }
        );

        self.stack.push_array(&value)?;

        Ok(Action::Continue)
    }

    /// writes a (u)int256 to storage
    #[maybe_async]
    pub async fn opcode_sstore(&mut self, backend: &mut B) -> Result<Action> {
        if self.is_static {
            return Err(Error::StaticModeViolation(self.context.contract));
        }

        let index = self.stack.pop_u256()?;
        let value = *self.stack.pop_array()?;

        tracing_event!(
            self,
            backend,
            super::tracing::Event::StorageAccess { index, value }
        );

        backend.set_storage(self.context.contract, index, value)?;

        Ok(Action::Continue)
    }

    /// unconditional jump
    #[maybe_async]
    pub async fn opcode_jump(&mut self, _backend: &mut B) -> Result<Action> {
        const JUMPDEST: u8 = 0x5B;

        let value = self.stack.pop_usize()?;

        if self.execution_code.get(value) == Some(&JUMPDEST) {
            Ok(Action::Jump(value))
        } else {
            Err(Error::InvalidJump(self.context.contract, value))
        }
    }

    /// conditional jump
    #[maybe_async]
    pub async fn opcode_jumpi(&mut self, _backend: &mut B) -> Result<Action> {
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
    #[maybe_async]
    pub async fn opcode_pc(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_usize(self.pc)?;

        Ok(Action::Continue)
    }

    /// memory size
    #[maybe_async]
    pub async fn opcode_msize(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_usize(self.memory.size())?;

        Ok(Action::Continue)
    }

    /// remaining gas
    #[maybe_async]
    pub async fn opcode_gas(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_u256(self.gas_limit)?;

        Ok(Action::Continue)
    }

    /// metadata to annotate possible jump destinations
    #[maybe_async]
    pub async fn opcode_jumpdest(&mut self, _backend: &mut B) -> Result<Action> {
        Ok(Action::Continue)
    }

    /// Place zero on stack
    #[maybe_async]
    pub async fn opcode_push_0(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.push_zero()?;

        Ok(Action::Continue)
    }

    /// Place 1 byte item on stack
    /// ~50% of contract bytecode are PUSH opcodes
    #[maybe_async]
    pub async fn opcode_push_1(&mut self, _backend: &mut B) -> Result<Action> {
        if self.execution_code.len() <= self.pc + 1 {
            return Err(Error::PushOutOfBounds(self.context.contract));
        }

        let value = unsafe { *self.execution_code.get_unchecked(self.pc + 1) };

        self.stack.push_byte(value)?;

        Ok(Action::Jump(self.pc + 1 + 1))
    }

    /// Place 2-31 byte item on stack.
    #[maybe_async]
    pub async fn opcode_push_2_31<const N: usize>(&mut self, _backend: &mut B) -> Result<Action> {
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
    #[maybe_async]
    pub async fn opcode_push_32(&mut self, _backend: &mut B) -> Result<Action> {
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
    #[maybe_async]
    pub async fn opcode_dup_1_16<const N: usize>(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.dup_1_16::<N>()?;

        Ok(Action::Continue)
    }

    /// Exchange 1st and (N+1)th stack item
    #[maybe_async]
    pub async fn opcode_swap_1_16<const N: usize>(&mut self, _backend: &mut B) -> Result<Action> {
        self.stack.swap_1_16::<N>()?;

        Ok(Action::Continue)
    }

    /// Append log record with N topics
    #[rustfmt::skip]
    #[maybe_async]
    pub async fn opcode_log_0_4<const N: usize>(&mut self, _backend: &mut B) -> Result<Action> {
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
            0 => log_data(&[b"LOG0", address, &[0], data]),                                                
            1 => log_data(&[b"LOG1", address, &[1], &topics[0], data]),                                    
            2 => log_data(&[b"LOG2", address, &[2], &topics[0], &topics[1], data]),                        
            3 => log_data(&[b"LOG3", address, &[3], &topics[0], &topics[1], &topics[2], data]),            
            4 => log_data(&[b"LOG4", address, &[4], &topics[0], &topics[1], &topics[2], &topics[3], data]),
            _ => unreachable!(),
        }

        Ok(Action::Continue)
    }

    /// Create a new account with associated code.
    #[maybe_async]
    pub async fn opcode_create(&mut self, backend: &mut B) -> Result<Action> {
        if self.is_static {
            return Err(Error::StaticModeViolation(self.context.contract));
        }

        let value = self.stack.pop_u256()?;
        let offset = self.stack.pop_usize()?;
        let length = self.stack.pop_usize()?;

        let created_address = {
            let source = self.context.contract;
            let chain_id = self.context.contract_chain_id;

            let nonce = backend.nonce(source, chain_id).await?;

            Address::from_create(&source, nonce)
        };

        self.opcode_create_impl(created_address, value, offset, length, backend)
            .await
    }

    /// Constantinople harfork, EIP-1014: creates a create a new account with a deterministic address
    #[maybe_async]
    pub async fn opcode_create2(&mut self, backend: &mut B) -> Result<Action> {
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

        self.opcode_create_impl(created_address, value, offset, length, backend)
            .await
    }

    #[maybe_async]
    async fn opcode_create_impl(
        &mut self,
        address: Address,
        value: U256,
        offset: usize,
        length: usize,
        backend: &mut B,
    ) -> Result<Action> {
        let chain_id = self.context.contract_chain_id;

        let contract_nonce = backend.nonce(self.context.contract, chain_id).await?;
        if contract_nonce == u64::MAX {
            return Err(Error::NonceOverflow(self.context.contract));
        }

        backend.increment_nonce(self.context.contract, chain_id)?;

        self.return_data = Buffer::empty();
        self.return_range = 0..0;

        let init_code = self.memory.read_buffer(offset, length)?;

        let context = Context {
            caller: self.context.contract,
            contract: address,
            contract_chain_id: chain_id,
            value,
            code_address: None,
        };

        tracing_event!(
            self,
            backend,
            super::tracing::Event::BeginVM {
                context,
                code: init_code.to_vec()
            }
        );

        self.fork(
            Reason::Create,
            chain_id,
            context,
            init_code,
            Buffer::empty(),
            None,
        );
        backend.snapshot();

        log_data(&[b"ENTER", b"CREATE", address.as_bytes()]);

        if (backend.nonce(address, chain_id).await? != 0)
            || (backend.code_size(address).await? != 0)
        {
            return Err(Error::DeployToExistingAccount(address, self.context.caller));
        }

        backend.increment_nonce(address, chain_id)?;
        backend
            .transfer(self.context.caller, address, chain_id, value)
            .await?;

        Ok(Action::Noop)
    }

    /// Message-call into an account
    #[maybe_async]
    pub async fn opcode_call(&mut self, backend: &mut B) -> Result<Action> {
        let gas_limit = self.stack.pop_u256()?;
        let address = self.stack.pop_address()?;
        let value = self.stack.pop_u256()?;
        let args_offset = self.stack.pop_usize()?;
        let args_length = self.stack.pop_usize()?;
        let return_offset = self.stack.pop_usize()?;
        let return_length = self.stack.pop_usize()?;

        self.return_data = Buffer::empty();
        self.return_range = return_offset..(return_offset + return_length);

        let call_data = self.memory.read_buffer(args_offset, args_length)?;
        let code = backend.code(address).await?;

        let chain_id = self.context.contract_chain_id;
        let context = Context {
            caller: self.context.contract,
            contract: address,
            contract_chain_id: backend.contract_chain_id(address).await.unwrap_or(chain_id),
            value,
            code_address: Some(address),
        };

        tracing_event!(
            self,
            backend,
            super::tracing::Event::BeginVM {
                context,
                code: code.to_vec()
            }
        );

        self.fork(
            Reason::Call,
            chain_id,
            context,
            code,
            call_data,
            Some(gas_limit),
        );
        backend.snapshot();

        log_data(&[b"ENTER", b"CALL", address.as_bytes()]);

        if self.is_static && (value != U256::ZERO) {
            return Err(Error::StaticModeViolation(self.context.caller));
        }

        backend
            .transfer(self.context.caller, self.context.contract, chain_id, value)
            .await?;

        self.opcode_call_precompile_impl(backend, &address).await
    }

    /// Message-call into this account with an alternative account’s code
    #[maybe_async]
    pub async fn opcode_callcode(&mut self, backend: &mut B) -> Result<Action> {
        let gas_limit = self.stack.pop_u256()?;
        let address = self.stack.pop_address()?;
        let value = self.stack.pop_u256()?;
        let args_offset = self.stack.pop_usize()?;
        let args_length = self.stack.pop_usize()?;
        let return_offset = self.stack.pop_usize()?;
        let return_length = self.stack.pop_usize()?;

        self.return_data = Buffer::empty();
        self.return_range = return_offset..(return_offset + return_length);

        let call_data = self.memory.read_buffer(args_offset, args_length)?;
        let code = backend.code(address).await?;

        let chain_id = self.context.contract_chain_id;
        let context = Context {
            value,
            code_address: Some(address),
            ..self.context
        };

        tracing_event!(
            self,
            backend,
            super::tracing::Event::BeginVM {
                context,
                code: code.to_vec()
            }
        );

        self.fork(
            Reason::Call,
            chain_id,
            context,
            code,
            call_data,
            Some(gas_limit),
        );
        backend.snapshot();

        log_data(&[b"ENTER", b"CALLCODE", address.as_bytes()]);

        if backend.balance(self.context.caller, chain_id).await? < value {
            return Err(Error::InsufficientBalance(
                self.context.caller,
                chain_id,
                value,
            ));
        }

        self.opcode_call_precompile_impl(backend, &address).await
    }

    /// Homestead hardfork, EIP-7: Message-call into this account with an alternative account’s code,
    /// but persisting the current values for sender and value
    #[maybe_async]
    pub async fn opcode_delegatecall(&mut self, backend: &mut B) -> Result<Action> {
        let gas_limit = self.stack.pop_u256()?;
        let address = self.stack.pop_address()?;
        let args_offset = self.stack.pop_usize()?;
        let args_length = self.stack.pop_usize()?;
        let return_offset = self.stack.pop_usize()?;
        let return_length = self.stack.pop_usize()?;

        self.return_data = Buffer::empty();
        self.return_range = return_offset..(return_offset + return_length);

        let call_data = self.memory.read_buffer(args_offset, args_length)?;
        let code = backend.code(address).await?;

        let context = Context {
            code_address: Some(address),
            ..self.context
        };

        tracing_event!(
            self,
            backend,
            super::tracing::Event::BeginVM {
                context,
                code: code.to_vec()
            }
        );

        self.fork(
            Reason::Call,
            self.chain_id,
            context,
            code,
            call_data,
            Some(gas_limit),
        );
        backend.snapshot();

        log_data(&[b"ENTER", b"DELEGATECALL", address.as_bytes()]);

        self.opcode_call_precompile_impl(backend, &address).await
    }

    /// Byzantium hardfork, EIP-214: Static message-call into an account
    /// Disallowed contract creation, event emission, storage modification and contract destruction
    #[maybe_async]
    pub async fn opcode_staticcall(&mut self, backend: &mut B) -> Result<Action> {
        let gas_limit = self.stack.pop_u256()?;
        let address = self.stack.pop_address()?;
        let args_offset = self.stack.pop_usize()?;
        let args_length = self.stack.pop_usize()?;
        let return_offset = self.stack.pop_usize()?;
        let return_length = self.stack.pop_usize()?;

        self.return_data = Buffer::empty();
        self.return_range = return_offset..(return_offset + return_length);

        let call_data = self.memory.read_buffer(args_offset, args_length)?;
        let code = backend.code(address).await?;

        let chain_id = self.context.contract_chain_id;
        let context = Context {
            caller: self.context.contract,
            contract: address,
            contract_chain_id: backend.contract_chain_id(address).await.unwrap_or(chain_id),
            value: U256::ZERO,
            code_address: Some(address),
        };

        tracing_event!(
            self,
            backend,
            super::tracing::Event::BeginVM {
                context,
                code: code.to_vec()
            }
        );

        self.fork(
            Reason::Call,
            chain_id,
            context,
            code,
            call_data,
            Some(gas_limit),
        );
        self.is_static = true;

        backend.snapshot();

        log_data(&[b"ENTER", b"STATICCALL", address.as_bytes()]);

        self.opcode_call_precompile_impl(backend, &address).await
    }

    /// Call precompile contract.
    /// Returns `Action::Noop` if address is not a precompile
    #[maybe_async]
    async fn opcode_call_precompile_impl(
        &mut self,
        backend: &mut B,
        address: &Address,
    ) -> Result<Action> {
        let result = match Self::precompile(address, &self.call_data).map(Ok) {
            Some(x) => Some(x),
            None => {
                backend
                    .precompile_extension(&self.context, address, &self.call_data, self.is_static)
                    .await
            }
        };

        if let Some(return_data) = result.transpose()? {
            return self.opcode_return_impl(return_data, backend).await;
        }

        Ok(Action::Noop)
    }

    /// Halt execution returning output data
    #[maybe_async]
    pub async fn opcode_return(&mut self, backend: &mut B) -> Result<Action> {
        let offset = self.stack.pop_usize()?;
        let length = self.stack.pop_usize()?;

        let return_data = self.memory.read(offset, length)?.to_vec();

        self.opcode_return_impl(return_data, backend).await
    }

    /// Halt execution returning output data
    #[maybe_async]
    pub async fn opcode_return_impl(
        &mut self,
        mut return_data: Vec<u8>,
        backend: &mut B,
    ) -> Result<Action> {
        if self.reason == Reason::Create {
            let code = std::mem::take(&mut return_data);
            backend.set_code(self.context.contract, self.chain_id, code)?;
        }

        backend.commit_snapshot();
        log_data(&[b"EXIT", b"RETURN"]);

        if self.parent.is_none() {
            return Ok(Action::Return(return_data));
        }

        trace_end_step!(self, backend, Some(return_data.clone()));
        tracing_event!(
            self,
            backend,
            super::tracing::Event::EndVM {
                status: super::ExitStatus::Return(return_data.clone())
            }
        );

        let returned = self.join();
        match returned.reason {
            Reason::Call => {
                self.memory.write_range(&self.return_range, &return_data)?;
                self.stack.push_bool(true)?; // success

                self.return_data = Buffer::from_vec(return_data);
            }
            Reason::Create => {
                let address = returned.context.contract;
                self.stack.push_address(&address)?;
            }
        }

        Ok(Action::Continue)
    }

    /// Byzantium hardfork, EIP-140: Halt execution reverting state changes but returning data
    #[maybe_async]
    pub async fn opcode_revert(&mut self, backend: &mut B) -> Result<Action> {
        let offset = self.stack.pop_usize()?;
        let length = self.stack.pop_usize()?;

        let return_data = self.memory.read(offset, length)?.to_vec();

        self.opcode_revert_impl(return_data, backend).await
    }

    #[maybe_async]
    pub async fn opcode_revert_impl(
        &mut self,
        return_data: Vec<u8>,
        backend: &mut B,
    ) -> Result<Action> {
        backend.revert_snapshot();
        log_data(&[b"EXIT", b"REVERT", &return_data]);

        if self.parent.is_none() {
            return Ok(Action::Revert(return_data));
        }

        trace_end_step!(self, backend, Some(return_data.clone()));
        tracing_event!(
            self,
            backend,
            super::tracing::Event::EndVM {
                status: super::ExitStatus::Revert(return_data.clone())
            }
        );

        let returned = self.join();
        match returned.reason {
            Reason::Call => {
                self.memory.write_range(&self.return_range, &return_data)?;
                self.stack.push_bool(false)?; // fail
            }
            Reason::Create => {
                self.stack.push_zero()?;
            }
        }

        self.return_data = Buffer::from_vec(return_data);

        Ok(Action::Continue)
    }

    /// Invalid instruction
    #[maybe_async]
    pub async fn opcode_invalid(&mut self, _backend: &mut B) -> Result<Action> {
        Err(Error::InvalidOpcode(
            self.context.contract,
            self.execution_code[self.pc],
        ))
    }

    /// Halt execution, destroys the contract and send all funds to address
    #[maybe_async]
    pub async fn opcode_selfdestruct(&mut self, backend: &mut B) -> Result<Action> {
        if self.is_static {
            return Err(Error::StaticModeViolation(self.context.contract));
        }

        let address = self.stack.pop_address()?;

        let chain_id = self.context.contract_chain_id;
        let value = backend.balance(self.context.contract, chain_id).await?;
        backend
            .transfer(self.context.contract, address, chain_id, value)
            .await?;
        backend.selfdestruct(self.context.contract)?;

        backend.commit_snapshot();
        log_data(&[b"EXIT", b"SELFDESTRUCT"]);

        if self.parent.is_none() {
            return Ok(Action::Suicide);
        }

        trace_end_step!(self, backend, None);
        tracing_event!(
            self,
            backend,
            super::tracing::Event::EndVM {
                status: super::ExitStatus::Suicide
            }
        );

        let returned = self.join();
        match returned.reason {
            Reason::Call => {
                self.memory.write_range(&self.return_range, &[])?;
                self.stack.push_bool(true)?; // success
            }
            Reason::Create => {
                self.stack.push_zero()?;
            }
        }

        Ok(Action::Continue)
    }

    /// Halts execution of the contract
    #[maybe_async]
    pub async fn opcode_stop(&mut self, backend: &mut B) -> Result<Action> {
        backend.commit_snapshot();
        log_data(&[b"EXIT", b"STOP"]);

        if self.parent.is_none() {
            return Ok(Action::Stop);
        }

        trace_end_step!(self, backend, None);
        tracing_event!(
            self,
            backend,
            super::tracing::Event::EndVM {
                status: super::ExitStatus::Stop
            }
        );

        let returned = self.join();
        match returned.reason {
            Reason::Call => {
                self.memory.write_range(&self.return_range, &[])?;
                self.stack.push_bool(true)?; // success
            }
            Reason::Create => {
                self.stack.push_zero()?;
            }
        }

        Ok(Action::Continue)
    }
}
