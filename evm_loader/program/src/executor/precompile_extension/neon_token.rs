use std::convert::TryInto;

use arrayref::array_ref;
use ethnum::U256;
use maybe_async::maybe_async;
use solana_program::{account_info::IntoAccountInfo, program_pack::Pack, pubkey::Pubkey};
use spl_associated_token_account::get_associated_token_address;

use crate::types::vector::into_vector;
use crate::vector;

use crate::{
    account::token,
    account_storage::AccountStorage,
    error::{Error, Result},
    executor::ExecutorState,
    types::Address,
};

// Neon token method ids:
//--------------------------------------------------
// withdraw(bytes32)           => 8e19899e
//--------------------------------------------------
const NEON_TOKEN_METHOD_WITHDRAW_ID: &[u8; 4] = &[0x8e, 0x19, 0x89, 0x9e];

impl<B: AccountStorage> ExecutorState<'_, B> {
    #[maybe_async]
    pub async fn neon_token(
        &mut self,
        address: &Address,
        input: &[u8],
        context: &crate::evm::Context,
        is_static: bool,
    ) -> Result<Vec<u8>> {
        debug_print!("neon_token({})", hex::encode(input));

        if &context.contract != address {
            return Err(Error::Custom(
                "Withdraw: callcode or delegatecall is not allowed".to_string(),
            ));
        }

        let (method_id, rest) = input.split_at(4);
        let method_id: &[u8; 4] = method_id.try_into().unwrap_or(&[0_u8; 4]);

        if method_id == NEON_TOKEN_METHOD_WITHDRAW_ID {
            if is_static {
                return Err(Error::StaticModeViolation(*address));
            }

            let source = context.contract;
            let chain_id = context.contract_chain_id;
            let value = context.value;
            // owner of the associated token account
            let destination = array_ref![rest, 0, 32];
            let destination = Pubkey::new_from_array(*destination);

            self.withdraw(source, chain_id, destination, value).await?;

            let mut output = vec![0_u8; 32];
            output[31] = 1; // return true

            return Ok(output);
        };

        debug_print!("neon_token UNKNOWN");
        Err(Error::UnknownPrecompileMethodSelector(*address, *method_id))
    }

    #[maybe_async]
    async fn withdraw(
        &mut self,
        source: Address,
        chain_id: u64,
        target: Pubkey,
        value: U256,
    ) -> Result<()> {
        if value == 0 {
            return Err(Error::Custom("Neon Withdraw: value == 0".to_string()));
        }

        let mint_address = self.backend.chain_id_to_token(chain_id);

        let mut mint_account = self.external_account(mint_address).await?;
        let mint_data = {
            let info = mint_account.into_account_info();
            token::Mint::from_account(&info)?.into_data()
        };

        assert!(mint_data.decimals < 18);

        let additional_decimals: u32 = (18 - mint_data.decimals).into();
        let min_amount: u128 = u128::pow(10, additional_decimals);

        let spl_amount = value / min_amount;
        let remainder = value % min_amount;

        if spl_amount > U256::from(u64::MAX) {
            return Err(Error::Custom(
                "Neon Withdraw: value exceeds u64::max".to_string(),
            ));
        }

        if remainder != 0 {
            return Err(Error::Custom(std::format!(
                "Neon Withdraw: value must be divisible by 10^{additional_decimals}"
            )));
        }

        let target_token = get_associated_token_address(&target, &mint_address);
        let account = self.external_account(target_token).await?;
        if !spl_token::check_id(&account.owner) {
            use spl_associated_token_account::instruction::create_associated_token_account;

            let create_associated = create_associated_token_account(
                &self.backend.operator(),
                &target,
                &mint_address,
                &spl_token::ID,
            );

            let fee = self
                .backend
                .rent()
                .minimum_balance(spl_token::state::Account::LEN);
            self.queue_external_instruction(create_associated, vector![], fee);
        }

        let (authority, bump_seed) =
            Pubkey::find_program_address(&[b"Deposit"], self.backend.program_id());
        let pool = get_associated_token_address(&authority, &mint_address);

        let transfer = spl_token::instruction::transfer_checked(
            &spl_token::ID,
            &pool,
            &mint_address,
            &target_token,
            &authority,
            &[],
            spl_amount.as_u64(),
            mint_data.decimals,
        )?;
        let transfer_seeds = vector![into_vector(b"Deposit".to_vec()), vector![bump_seed]];
        self.queue_external_instruction(transfer, transfer_seeds, 0);

        self.burn(source, chain_id, value);

        Ok(())
    }
}
