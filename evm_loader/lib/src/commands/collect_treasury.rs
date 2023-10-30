use crate::rpc::check_account_for_fee;
use crate::{
    commands::get_neon_elf::read_elf_parameters_from_account, errors::NeonError, Config, Context,
    NeonResult,
};
use evm_loader::account::{MainTreasury, Treasury};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    system_program,
    transaction::Transaction,
};
use spl_token::instruction::sync_native;

#[derive(Debug, Serialize, Deserialize)]
pub struct CollectTreasuryReturn {
    pub pool_address: String,
    pub balance: u64,
}

pub async fn execute(config: &Config, context: &Context<'_>) -> NeonResult<CollectTreasuryReturn> {
    let neon_params = read_elf_parameters_from_account(config, context).await?;
    let signer = context.signer()?;

    let pool_count: u32 = neon_params
        .get("NEON_POOL_COUNT")
        .and_then(|value| value.parse().ok())
        .ok_or(NeonError::IncorrectProgram(config.evm_loader))?;

    let main_balance_address = MainTreasury::address(&config.evm_loader).0;

    info!("Main pool balance: {}", main_balance_address);

    let client = context
        .rpc_client
        .as_any()
        .downcast_ref::<RpcClient>()
        .expect("cast to solana_client::rpc_client::RpcClient error");

    for i in 0..pool_count {
        let (aux_balance_address, _) = Treasury::address(&config.evm_loader, i);

        if let Some(aux_balance_account) = context
            .rpc_client
            .get_account_with_commitment(&aux_balance_address, config.commitment)
            .await?
            .value
        {
            let minimal_balance = context
                .rpc_client
                .get_minimum_balance_for_rent_exemption(aux_balance_account.data.len())
                .await?;
            let available_lamports = aux_balance_account.lamports.saturating_sub(minimal_balance);
            if available_lamports > 0 {
                info!(
                    "{:4}: collect {} lamports from {}",
                    i, available_lamports, aux_balance_address
                );
                let mut message = Message::new(
                    &[Instruction::new_with_bincode(
                        config.evm_loader,
                        &(30_u8, i),
                        vec![
                            AccountMeta::new(main_balance_address, false),
                            AccountMeta::new(aux_balance_address, false),
                            AccountMeta::new_readonly(system_program::id(), false),
                        ],
                    )],
                    Some(&signer.pubkey()),
                );
                let blockhash = context.rpc_client.get_latest_blockhash().await?;
                message.recent_blockhash = blockhash;

                check_account_for_fee(client, &signer.pubkey(), &message).await?;

                let mut trx = Transaction::new_unsigned(message);
                trx.try_sign(&[&*signer], blockhash)?;
                context
                    .rpc_client
                    .send_and_confirm_transaction_with_spinner(&trx)
                    .await?;
            } else {
                info!("{:4}: skip account {}", i, aux_balance_address);
            }
        } else {
            warn!("{:4}: not found account {}", i, aux_balance_address);
        }
    }
    let mut message = Message::new(
        &[sync_native(&spl_token::id(), &main_balance_address)?],
        Some(&signer.pubkey()),
    );
    let blockhash = context.rpc_client.get_latest_blockhash().await?;
    message.recent_blockhash = blockhash;

    check_account_for_fee(client, &signer.pubkey(), &message).await?;

    let mut trx = Transaction::new_unsigned(message);
    trx.try_sign(&[&*signer], blockhash)?;
    context
        .rpc_client
        .send_and_confirm_transaction_with_spinner(&trx)
        .await?;

    let main_balance_account = context
        .rpc_client
        .get_account(&main_balance_address)
        .await?;
    Ok(CollectTreasuryReturn {
        pool_address: main_balance_address.to_string(),
        balance: main_balance_account.lamports,
    })
}
