use crate::account::{Operator, program, EthereumAccount, sysvar, Treasury, State};
use crate::transaction::{check_secp256k1_instruction, UnsignedTransaction};
use crate::account_storage::ProgramAccountStorage;
use arrayref::{array_ref};
use evm::{H160};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult,
    pubkey::Pubkey,
};
use crate::instruction::transaction::Accounts;
use crate::config::chain_id;


pub fn process<'a>(program_id: &'a Pubkey, accounts: &'a [AccountInfo<'a>], instruction: &[u8]) -> ProgramResult {
    solana_program::msg!("Instruction: Begin Transaction from Instruction");

    let treasury_index = u32::from_le_bytes(*array_ref![instruction, 0, 4]);
    let step_count = u64::from_le_bytes(*array_ref![instruction, 4, 8]);
    let caller = H160::from(*array_ref![instruction, 4 + 8, 20]);
    let signature = array_ref![instruction, 4 + 8 + 20, 65];
    let unsigned_msg = &instruction[4 + 8 + 20 + 65..];


    let storage_info = &accounts[0];

    let sysvar_instructions = sysvar::Instructions::from_account(&accounts[1])?;
    check_secp256k1_instruction(sysvar_instructions.info, unsigned_msg.len(), 13_u16)?;

    let accounts = Accounts {
        operator: Operator::from_account(&accounts[2])?,
        treasury: Treasury::from_account(program_id, treasury_index, &accounts[3])?,
        operator_ether_account: EthereumAccount::from_account(program_id, &accounts[4])?,
        system_program: program::System::from_account(&accounts[5])?,
        neon_program: program::Neon::from_account(program_id, &accounts[6])?,
        remaining_accounts: &accounts[7..]
    };


    let trx = UnsignedTransaction::from_rlp(unsigned_msg)?;

    let storage = State::new(program_id, storage_info, &accounts, caller, &trx, signature)?;
    let mut account_storage = ProgramAccountStorage::new(
        program_id,
        accounts.remaining_accounts,
        crate::config::token_mint::id(),
        chain_id().as_u64(),
    )?;

    super::transaction::do_begin(step_count, accounts, storage, &mut account_storage, trx, caller)
}

