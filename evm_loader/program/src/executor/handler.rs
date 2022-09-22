use evm::{H160, ExitReason, U256, Transfer, ExitError, CONFIG, Handler, H256, Capture};
use solana_program::{entrypoint::ProgramResult, program_error::ProgramError};

use crate::{
    event, account_storage::AccountStorage, precompile::{call_precompile, is_precompile_address}
};

use super::{state::ExecutorState, gasometer::Gasometer};


pub struct CallInterrupt {
    pub context: evm::Context,
    pub transfer: Option<evm::Transfer>,
    pub code_address: H160,
    pub input: Vec<u8>,
    pub is_static: bool,
}

pub struct CreateInterrupt {
    pub context: evm::Context,
    pub transfer: Option<evm::Transfer>,
    pub address: H160,
    pub init_code: Vec<u8>,
}


/// Stack-based executor.
pub struct Executor<'a, B: AccountStorage> {
    pub state: ExecutorState<'a, B>,
    pub gasometer: Gasometer,
    pub origin: H160,
    pub gas_limit: U256,
    pub gas_price: U256,
}


impl<'a, B: AccountStorage> Executor<'a, B> {
    pub fn create_address(&self, scheme: evm::CreateScheme) -> H160 {
        match scheme {
            evm::CreateScheme::Create2 { caller, code_hash, salt } => {
                crate::utils::keccak256_h256_v(&[&[0xff], &caller[..], &salt[..], &code_hash[..]]).into()
            },
            evm::CreateScheme::Legacy { caller } => {
                let nonce = self.nonce(caller);
                let mut stream = rlp::RlpStream::new_list(2);
                stream.append(&caller);
                stream.append(&nonce);
                crate::utils::keccak256_h256(&stream.out()).into()
            },
            evm::CreateScheme::Fixed(naddress) => {
                naddress
            },
        }
    }

    pub fn transfer(&mut self, transfer: Transfer) -> Result<(), ExitError> {
        self.state.transfer(transfer.source, transfer.target, transfer.value)
    }

    pub fn call_begin(
        &mut self,
        origin: H160,
        address: H160,
        _data: &[u8],
        value: U256,
        gas_limit: U256,
        gas_price: U256
    ) -> ProgramResult {
        event!(TransactCall { caller, address, value, data, gas_limit });

        self.gas_limit = gas_limit;
        self.gas_price = gas_price;

        self.state.inc_nonce(origin);
        self.state.enter(false);
        
        if let Err(error) = self.state.transfer(origin, address, value)  {
            return Err!(ProgramError::InsufficientFunds; "ExitError={:?}", error);
        }

        Ok(())
    }

    pub fn create_begin(
        &mut self,
        origin: H160,
        _init_code: &[u8],
        value: U256,
        gas_limit: U256,
        gas_price: U256
    ) -> Result<H160, ProgramError> {
        event!(TransactCreate { caller, value, init_code, gas_limit });

        self.gas_limit = gas_limit;
        self.gas_price = gas_price;

        let scheme = evm::CreateScheme::Legacy { caller: origin };
        let address = self.create_address(scheme);
        
        if self.code_size(address) > U256::zero() {
            return Err!(ProgramError::AccountAlreadyInitialized; "Attempt to deploy to existing account (code_size > 0)")
        }
        
        if self.nonce(address) > U256::zero() {
            return Err!(ProgramError::AccountAlreadyInitialized; "Attempt to deploy to existing account (nonce > 0)")
        }

        self.state.inc_nonce(origin);
        self.state.enter(false);

        if CONFIG.create_increase_nonce {
            self.state.inc_nonce(address);
        }

        if let Err(error) = self.state.transfer(origin, address, value) {
            return Err!(ProgramError::InsufficientFunds; "ExitError={:?}", error);
        }

        Ok(address)
    }
}


impl<'a, B: AccountStorage> Handler for Executor<'a, B> {
    type CreateInterrupt = CreateInterrupt;
    type CreateFeedback = std::convert::Infallible;
    type CallInterrupt = CallInterrupt;
    type CallFeedback = std::convert::Infallible;

    fn keccak256_h256(&self, data: &[u8]) -> H256 {
        crate::utils::keccak256_h256(data)
    }

    fn nonce(&self, address: H160) -> U256 {
        self.state.nonce(&address)
    }

    fn balance(&self, address: H160) -> U256 {
        self.state.balance(&address)
    }

    fn code_size(&self, address: H160) -> U256 {
        if is_precompile_address(&address) {
            return U256::one();
        }

        self.state.code_size(&address)
    }

    fn code_hash(&self, address: H160) -> H256 {
        self.state.code_hash(&address)
    }

    fn code(&self, address: H160) -> Vec<u8> {
        self.state.code(&address)
    }

    fn valids(&self, address: H160) -> Vec<u8> {
        self.state.valids(&address)
    }

    fn storage(&self, address: H160, index: U256) -> U256 {
        self.state.storage(&address, &index)
    }

    fn gas_left(&self) -> U256 {
        self.gas_limit.saturating_sub(self.gasometer.used_gas_total())
    }

    fn gas_price(&self) -> U256 {
        self.gas_price
    }

    fn origin(&self) -> H160 {
        self.origin
    }

    fn block_hash(&self, number: U256) -> H256 {
        self.state.block_hash(number)
    }

    fn block_number(&self) -> U256 {
        self.state.block_number()
    }

    fn block_coinbase(&self) -> H160 {
        H160::default()
    }

    fn block_timestamp(&self) -> U256 {
        self.state.block_timestamp()
    }

    fn block_difficulty(&self) -> U256 {
        U256::zero()
    }

    fn block_gas_limit(&self) -> U256 {
        U256::max_value()
    }

    fn chain_id(&self) -> U256 {
        let chain_id = self.state.backend.chain_id();
        U256::from(chain_id)
    }

    fn set_storage(&mut self, address: H160, index: U256, value: U256) -> Result<(), ExitError> {
        if self.state.is_static_context() {
            return Err(ExitError::StaticModeViolation);
        }

        self.gasometer.record_storage_write(&self.state, address, index, value);

        self.state.set_storage(address, index, value);
        Ok(())
    }

    fn log(&mut self, address: H160, topics: Vec<H256>, data: Vec<u8>) -> Result<(), ExitError> {
        if self.state.is_static_context() {
            return Err(ExitError::StaticModeViolation);
        }

        self.state.log(address, topics, data);
        Ok(())
    }

    fn mark_delete(&mut self, address: H160, target: H160) -> Result<(), ExitError> {
        if self.state.is_static_context() {
            return Err(ExitError::StaticModeViolation);
        }

        let balance = self.balance(address);

        self.state.transfer(address, target, balance)?;
        self.state.set_deleted(address);

        Ok(())
    }

    fn create(
        &mut self,
        caller: H160,
        scheme: evm::CreateScheme,
        value: U256,
        init_code: Vec<u8>,
        #[allow(unused_variables)] target_gas: Option<u64>,
    ) -> Capture<(ExitReason, Option<H160>, Vec<u8>), Self::CreateInterrupt> {
        debug_print!("create");

        if self.state.is_static_context() {
            return Capture::Exit((ExitError::StaticModeViolation.into(), None, Vec::new()))
        }

        if !value.is_zero() && (self.balance(caller) < value) {
            return Capture::Exit((ExitError::OutOfFund.into(), None, Vec::new()))
        }

        // Get the create address from given scheme.
        let address = self.create_address(scheme);

        event!(Create {
            caller,
            address,
            scheme,
            value,
            init_code: &init_code,
            target_gas,
        });


        self.state.inc_nonce(caller);

        if self.state.code_size(&address) > U256::zero() {
            return Capture::Exit((ExitError::CreateCollision.into(), None, Vec::new()))
        }

        if self.state.nonce(&address) > U256::zero() {
            return Capture::Exit((ExitError::CreateCollision.into(), None, Vec::new()))
        }

        let context = evm::Context {
            address,
            caller,
            apparent_value: value,
        };

        let transfer = Some(evm::Transfer { source: caller, target: address, value });

        Capture::Trap(CreateInterrupt{context, transfer, address, init_code})
    }

    fn call(
        &mut self,
        code_address: H160,
        transfer: Option<evm::Transfer>,
        input: Vec<u8>,
        #[allow(unused_variables)] target_gas: Option<u64>,
        is_static: bool,
        context: evm::Context,
    ) -> Capture<(ExitReason, Vec<u8>), Self::CallInterrupt> {
        event!(Call {
            code_address,
            transfer: &transfer,
            input: &input,
            target_gas,
            is_static,
            context: &context,
        });

        debug_print!("call {:?}, {:?}", code_address, input);

        if let Some(transfer) = transfer {
            if (self.state.is_static_context() || is_static) && !transfer.value.is_zero() {
                return Capture::Exit((ExitError::StaticModeViolation.into(), Vec::new()))
            }
        }

        let precompile_result = call_precompile(code_address, &input, &context, &mut self.state, &mut self.gasometer);
        if let Some(Capture::Exit(exit_value)) = precompile_result {
            return Capture::Exit(exit_value);
        }

        Capture::Trap(CallInterrupt{context, transfer, code_address, input, is_static})
    }

    fn pre_validate(
        &mut self,
        _context: &evm::Context,
        _opcode: evm::Opcode,
        _stack: &evm::Stack,
    ) -> Result<(), ExitError> {
        Ok(())
    }
}
