use std::convert::{Infallible, TryInto};

use arrayref::array_ref;
use evm::{Capture, ExitReason, U256, H160};
use solana_program::{pubkey::Pubkey, program_pack::Pack, program_error::ProgramError};
use spl_associated_token_account::get_associated_token_address;

use crate::{
    account_storage::AccountStorage,
    executor::{ExecutorState, Gasometer}
};


// Neon token method ids:
//--------------------------------------------------
// withdraw(bytes32)           => 8e19899e
//--------------------------------------------------
const NEON_TOKEN_METHOD_WITHDRAW_ID: &[u8; 4]       = &[0x8e, 0x19, 0x89, 0x9e];


#[must_use]
pub fn neon_token<B: AccountStorage>(
    input: &[u8],
    context: &evm::Context,
    state: &mut ExecutorState<B>,
    gasometer: &mut Gasometer
)
    -> Capture<(ExitReason, Vec<u8>), Infallible>
{
    debug_print!("neon_token({})", hex::encode(input));

    let (method_id, rest) = input.split_at(4);
    let method_id: &[u8; 4] = method_id.try_into().unwrap_or(&[0_u8; 4]);

    if method_id == NEON_TOKEN_METHOD_WITHDRAW_ID  {
        if state.is_static_context() {
            let revert_message = b"neon_token: withdraw is not allowed in static context".to_vec();
            return Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
        }

        let source = context.address; // caller contract

        // owner of the associated token account
        let destination = array_ref![rest, 0, 32];
        let destination = Pubkey::new_from_array(*destination);


        if withdraw(state, gasometer, source, destination, context.apparent_value).is_err() {
            let revert_message = b"neon_token: failed to withdraw NEON".to_vec();
            return Capture::Exit((ExitReason::Revert(evm::ExitRevert::Reverted), revert_message))
        }

        let mut output = vec![0_u8; 32];
        output[31] = 1; // return true

        return Capture::Exit((ExitReason::Succeed(evm::ExitSucceed::Returned), output));
    };

    debug_print!("neon_token UNKNOWN");
    Capture::Exit((ExitReason::Fatal(evm::ExitFatal::NotSupported), vec![]))
}


fn withdraw<B: AccountStorage>(
    state: &mut ExecutorState<B>,
    gasometer: &mut Gasometer,
    source: H160,
    target: Pubkey,
    value: U256
) -> Result<(), ProgramError> {
    if value.is_zero() {
        return Ok(())
    }

    if state.balance(&source) < value {
        return Err(ProgramError::InsufficientFunds);
    }

    let min_amount: u64 = u64::pow(10, u32::from(crate::config::token_mint::decimals()));
    let (spl_amount, remainder) = value.div_mod(U256::from(min_amount));

    if spl_amount > U256::from(u64::MAX) {
        return Err(ProgramError::InvalidArgument);
    }

    if !remainder.is_zero() {
        return Err(ProgramError::InvalidArgument);
    }


    let target_token = get_associated_token_address(&target, state.backend.neon_token_mint());
    let account = state.external_account(target_token)?;
    if !spl_token::check_id(&account.owner) {
        use spl_associated_token_account::instruction::create_associated_token_account;

        gasometer.record_account_rent(spl_token::state::Account::LEN);

        let create_associated = create_associated_token_account(
            state.backend.operator(), &target, state.backend.neon_token_mint()
        );
        state.queue_external_instruction(create_associated, vec![]);
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
    state.queue_external_instruction(transfer, transfer_seeds);


    state.withdraw(source, value);


    Ok(())

}