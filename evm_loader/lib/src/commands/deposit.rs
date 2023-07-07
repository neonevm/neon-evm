use log::debug;
use serde::Serialize;

use crate::{Config, Context, NeonResult};
use evm_loader::types::Address;
use solana_cli::checks::check_account_for_fee;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::Signature,
    system_program,
    transaction::Transaction,
};
use spl_associated_token_account::get_associated_token_address;

#[derive(Serialize)]
pub struct DepositReturn {
    pub transaction: Signature,
}

/// Executes subcommand `deposit`.
pub async fn execute(
    config: &Config,
    context: &Context,
    amount: u64,
    ether_address: &Address,
) -> NeonResult<DepositReturn> {
    let (ether_pubkey, _) = ether_address.find_solana_address(&config.evm_loader);

    let token_mint_id = evm_loader::config::token_mint::id();

    let signer_token_pubkey =
        get_associated_token_address(&context.signer.pubkey(), &token_mint_id);
    let evm_token_authority = Pubkey::find_program_address(&[b"Deposit"], &config.evm_loader).0;
    let evm_pool_pubkey = get_associated_token_address(&evm_token_authority, &token_mint_id);

    let instructions = vec![
        spl_approve_instruction(config, context, signer_token_pubkey, ether_pubkey, amount),
        deposit_instruction(
            config,
            context,
            signer_token_pubkey,
            evm_pool_pubkey,
            ether_address,
            ether_pubkey,
        ),
    ];

    let mut finalize_message = Message::new(&instructions, Some(&context.signer.pubkey()));
    let blockhash = context.rpc_client.get_latest_blockhash().await?;
    finalize_message.recent_blockhash = blockhash;

    let client = context
        .blocking_rpc_client
        .as_ref()
        .expect("Blocking RPC client not initialized");

    check_account_for_fee(client, &context.signer.pubkey(), &finalize_message)?;

    let mut finalize_tx = Transaction::new_unsigned(finalize_message);

    finalize_tx.try_sign(&[&*context.signer], blockhash)?;
    debug!("signed: {:x?}", finalize_tx);

    let signature = context
        .rpc_client
        .send_and_confirm_transaction_with_spinner(&finalize_tx)
        .await?;

    Ok(DepositReturn {
        transaction: signature,
    })
}

/// Returns instruction to approve transfer of NEON tokens.
fn spl_approve_instruction(
    _config: &Config,
    context: &Context,
    source_pubkey: Pubkey,
    delegate_pubkey: Pubkey,
    amount: u64,
) -> Instruction {
    use spl_token::instruction::TokenInstruction;

    let accounts = vec![
        AccountMeta::new(source_pubkey, false),
        AccountMeta::new_readonly(delegate_pubkey, false),
        AccountMeta::new_readonly(context.signer.pubkey(), true),
    ];

    let data = TokenInstruction::Approve { amount }.pack();

    Instruction {
        program_id: spl_token::id(),
        accounts,
        data,
    }
}

/// Returns instruction to deposit NEON tokens.
fn deposit_instruction(
    config: &Config,
    context: &Context,
    source_pubkey: Pubkey,
    destination_pubkey: Pubkey,
    ether_address: &Address,
    ether_account_pubkey: Pubkey,
) -> Instruction {
    Instruction::new_with_bincode(
        config.evm_loader,
        &(0x27_u8, ether_address.as_bytes()),
        vec![
            AccountMeta::new(source_pubkey, false),
            AccountMeta::new(destination_pubkey, false),
            AccountMeta::new(ether_account_pubkey, false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new(context.signer.pubkey(), true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
    )
}
