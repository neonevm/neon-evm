use arrayref::{array_ref, array_refs};
use evm::U256;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::account::{EthereumAccount, Operator, program};
use crate::account_storage::ProgramAccountStorage;
use crate::config::{chain_id, STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT};

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
        const U256_SIZE: usize = 32;
        const INDEX_SIZE: usize = U256_SIZE;
        const VALUE_SIZE: usize = U256_SIZE;

        let data = array_ref![data, 0, INDEX_SIZE + VALUE_SIZE];
        #[allow(clippy::ptr_offset_with_cast)]
        let (index, value) = array_refs![data, INDEX_SIZE, VALUE_SIZE];

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

    let ethereum_account = EthereumAccount::from_account(
        program_id,
        &accounts[AccountIndexes::EthereumAccount as usize],
    )?;

    let parsed_instruction_data = ParsedInstructionData::parse(instruction_data);

    validate(&ethereum_account, &parsed_instruction_data)?;

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

    account_storage.update_storage_infinite(
        ethereum_account.address,
        parsed_instruction_data.index,
        parsed_instruction_data.value,
        &operator,
        &system_program,
    )
}

/// Validates provided data.
fn validate(
    ethereum_account: &EthereumAccount,
    instruction_data: &ParsedInstructionData,
) -> ProgramResult {
    if ethereum_account.code_account.is_none() {
        return Err!(
            ProgramError::InvalidArgument;
            "Ethereum account {} must be a contract account",
            ethereum_account.address
        );
    }

    if instruction_data.index < U256::from(STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT) {
        return Err!(
            ProgramError::InvalidArgument;
            "Index ({}) is not supported in distributed storage. Indexes in range 0..{} must be \
                stored into contract account's data.",
            instruction_data.index,
            STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT
        );
    }

    Ok(())
}
