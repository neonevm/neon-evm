use arrayref::{array_ref, array_refs};
use evm::U256;
use solana_program::account_info::AccountInfo;
use solana_program::entrypoint::ProgramResult;
use solana_program::msg;
use solana_program::pubkey::Pubkey;

use crate::account::{EthereumAccount, Operator, program};
use crate::account_storage::ProgramAccountStorage;
use crate::config::chain_id;

enum AccountIndexes {
    Operator,
    SystemProgram,
    EthereumAccount,
}

struct ParsedInstructionData {
    index: U256,
    value: U256,
}

impl ParsedInstructionData {
    /// Instruction data layout:
    /// 0..32:  index (key)
    /// 32..64: value
    fn parse(data: &[u8]) -> Self {
        let data = array_ref![data, 0, 32 + 32];
        let (index, value) = array_refs![data, 32, 32];
        Self {
            index: U256::from(index),
            value: U256::from(value),
        }
    }
}

/// Processes the writing of one single value to the distributed storage.
pub fn process<'a>(
    program_id: &'a Pubkey,
    accounts: &'a [AccountInfo<'a>],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!("Instruction: WriteValueToDistributedStorage");

    let mut account_storage = ProgramAccountStorage::new(
        program_id,
        accounts,
        crate::config::token_mint::id(),
        chain_id().as_u64(),
    )?;

    let operator = unsafe {
        Operator::from_account_not_whitelisted(&accounts[AccountIndexes::Operator as usize])
    }?;
    let system_program = program::System::from_account(
        &accounts[AccountIndexes::SystemProgram as usize],
    )?;
    let ethereum_account = EthereumAccount::from_account(
        program_id,
        &accounts[AccountIndexes::EthereumAccount as usize],
    )?;
    let parsed_instruction_data = ParsedInstructionData::parse(instruction_data);

    account_storage.update_storage_infinite(
        ethereum_account.address,
        parsed_instruction_data.index,
        parsed_instruction_data.value,
        &operator,
        &system_program,
    )
}
