use std::convert::TryInto;

use arrayref::array_ref;
use ethnum::U256;
use solana_program::{pubkey::Pubkey, program_pack::Pack};
use spl_associated_token_account::get_associated_token_address;

use crate::{
    account_storage::AccountStorage,
    executor::{ExecutorState},
    error::{Result, Error}, types::Address, evm::database::Database,
};


// Neon token method ids:
//--------------------------------------------------
// withdraw(bytes32)           => 8e19899e
//--------------------------------------------------
const NEON_TOKEN_METHOD_WITHDRAW_ID: &[u8; 4]       = &[0x8e, 0x19, 0x89, 0x9e];

pub fn neon_token<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    address: &Address,
    input: &[u8],
    context: &crate::evm::Context,
    is_static: bool,
) -> Result<Vec<u8>> {
    debug_print!("neon_token({})", hex::encode(input));

    if &context.contract != address {
        return Err(Error::Custom("Withdraw: callcode or delegatecall is not allowed".to_string()))
    }

    let (method_id, rest) = input.split_at(4);
    let method_id: &[u8; 4] = method_id.try_into().unwrap_or(&[0_u8; 4]);

    if method_id == NEON_TOKEN_METHOD_WITHDRAW_ID  {
        if is_static { return Err(Error::StaticModeViolation(*address)); }

        let source = context.caller; // caller contract

        // owner of the associated token account
        let destination = array_ref![rest, 0, 32];
        let destination = Pubkey::new_from_array(*destination);

        withdraw(state, source, destination, context.value)?;

        let mut output = vec![0_u8; 32];
        output[31] = 1; // return true

        return Ok(output);
    };

    debug_print!("neon_token UNKNOWN");
    Err(Error::UnknownPrecompileMethodSelector(*address, *method_id))
}


fn withdraw<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    source: Address,
    target: Pubkey,
    value: U256
) -> Result<()> {
    if value == 0 {
        return Err(Error::Custom("Neon Withdraw: value == 0".to_string()));
    }

    if state.balance(&source)? < value {
        return Err(Error::InsufficientBalanceForTransfer(source, value));
    }

    let min_amount = u128::pow(10, u32::from(crate::config::token_mint::decimals()));
    let spl_amount = value / min_amount;
    let remainder = value % min_amount;

    if spl_amount > U256::from(u64::MAX) {
        return Err(Error::Custom("Neon Withdraw: value exceeds u64::max".to_string()));
    }

    if remainder != 0 {
        return Err(Error::Custom("Neon Withdraw: value must be divisible by 10^9".to_string()));
    }


    let target_token = get_associated_token_address(&target, state.backend.neon_token_mint());
    let account = state.external_account(target_token)?;
    if !spl_token::check_id(&account.owner) {
        use spl_associated_token_account::instruction::create_associated_token_account;

        let create_associated = create_associated_token_account(
            state.backend.operator(),
            &target,
            state.backend.neon_token_mint(),
            &spl_token::ID
        );
        state.queue_external_instruction(create_associated, vec![], spl_token::state::Account::LEN);
    }


    let (authority, bump_seed) = Pubkey::find_program_address(&[b"Deposit"], state.backend.program_id());
    let pool = get_associated_token_address(&authority, state.backend.neon_token_mint());

    let transfer = spl_token::instruction::transfer_checked(
        &spl_token::ID,
        &pool,
        state.backend.neon_token_mint(),
        &target_token,
        &authority,
        &[],
        spl_amount.as_u64(),
        crate::config::token_mint::decimals()
    ).unwrap();
    let transfer_seeds = vec![ b"Deposit".to_vec(), vec![bump_seed] ];
    state.queue_external_instruction(transfer, transfer_seeds, 0);


    state.withdraw_neons(source, value);


    Ok(())

}