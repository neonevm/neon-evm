use std::mem::size_of;

use arrayref::{array_ref, array_refs};
use evm::{H160, U256};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::account::{ACCOUNT_SEED_VERSION, EthereumAccount, EthereumStorage, Operator, program};
use crate::config::STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT;
use crate::error::EvmLoaderError;
use crate::instruction::storage_to_v2::OPERATOR_PUBKEY;

enum AccountIndexes {
    Operator,
    SystemProgram,
    EthereumAccount,
}

struct InstructionData {
    index: U256,
    value: U256,
}

impl InstructionData {
    /// Instruction data layout:
    /// 0..32:  index (key)
    /// 32..64: value
    fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        const U256_SIZE: usize = 32;
        const INDEX_SIZE: usize = U256_SIZE;
        const VALUE_SIZE: usize = U256_SIZE;
        const INSTRUCTION_DATA_SIZE: usize = INDEX_SIZE + VALUE_SIZE;

        if input.len() != INSTRUCTION_DATA_SIZE {
            msg!(
                "Fail: The instruction data size is {}, but it is expected to have a size {}.",
                input.len(),
                INSTRUCTION_DATA_SIZE,
            );
            return Err(ProgramError::InvalidArgument);
        }

        let instruction_data = array_ref![input, 0, INSTRUCTION_DATA_SIZE];
        #[allow(clippy::ptr_offset_with_cast)]
        let (index, value) = array_refs![instruction_data, INDEX_SIZE, VALUE_SIZE];

        Ok(Self {
            index: U256::from(index),
            value: U256::from(value),
        })
    }
}

fn write_to_storage<'a>(
    system_program: &program::System<'a>,
    program_id: &Pubkey,
    operator: &Operator<'a>,
    accounts: &'a [AccountInfo<'a>],
    address: &H160,
    index: &U256,
    value: U256,
) -> ProgramResult {
    let mut index_bytes = [0_u8; 32];
    index.to_little_endian(&mut index_bytes);

    let mut seeds: Vec<&[u8]> = vec![&[ACCOUNT_SEED_VERSION], b"ContractStorage", address.as_bytes(), &[0; size_of::<u32>()], &index_bytes];

    let (solana_address, bump_seed) = Pubkey::find_program_address(&seeds, program_id);
    let account = accounts.iter().find(|account| *account.key == solana_address)
        .ok_or_else(|| E!(ProgramError::InvalidArgument; "Account {} - storage account not found", solana_address))?;

    if !solana_program::system_program::check_id(account.owner) {
        return Err!(ProgramError::InvalidAccountData; "Account {} - expected system or program owned", account.key);
    }

    if value.is_zero() {
        return Ok(());
    }

    let bump_seed = [bump_seed];
    seeds.push(&bump_seed);

    system_program.create_pda_account(program_id, operator, account, &seeds, EthereumStorage::SIZE)?;

    EthereumStorage::init(account, crate::account::ether_storage::Data { value })?;

    Ok(())
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

    let parsed_instruction_data = InstructionData::unpack(instruction_data)?;

    validate(&ethereum_account, &parsed_instruction_data)?;

    let operator = unsafe {
        Operator::from_account_not_whitelisted(&accounts[AccountIndexes::Operator as usize])
    }?;

    if operator.key != &OPERATOR_PUBKEY {
        return Err!(
            EvmLoaderError::UnauthorizedOperator.into();
            "Account {} - expected authorized operator",
            operator.key
        );
    }

    let system_program = program::System::from_account(
        &accounts[AccountIndexes::SystemProgram as usize],
    )?;

    write_to_storage(
        &system_program,
        program_id,
        &operator,
        accounts,
        &ethereum_account.address,
        &parsed_instruction_data.index,
        parsed_instruction_data.value,
    )
}

/// Validates provided data.
fn validate(
    ethereum_account: &EthereumAccount,
    instruction_data: &InstructionData,
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
