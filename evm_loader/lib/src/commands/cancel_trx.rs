use log::info;

use serde::{Deserialize, Serialize};
use solana_sdk::{
    incinerator,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Signature,
    signer::Signer,
};

use evm_loader::account::State;

use crate::{account_storage::account_info, commands::send_transaction, rpc::Rpc, NeonResult};

#[derive(Debug, Serialize, Deserialize)]
pub struct CancelTrxReturn {
    pub transaction: Signature,
}

pub async fn execute(
    rpc_client: &dyn Rpc,
    signer: &dyn Signer,
    evm_loader: Pubkey,
    storage_account: &Pubkey,
) -> NeonResult<CancelTrxReturn> {
    let mut acc = rpc_client.get_account(storage_account).await?;
    let storage_info = account_info(storage_account, &mut acc);
    let storage = State::from_account(&evm_loader, &storage_info)?;

    let operator = &signer.pubkey();

    let mut accounts_meta: Vec<AccountMeta> = vec![
        AccountMeta::new(*storage_account, false),  // State account
        AccountMeta::new(*operator, true),          // Operator
        AccountMeta::new(incinerator::id(), false), // Incinerator
    ];

    let blocked_accounts = storage.read_blocked_accounts()?;
    for blocked_account_meta in blocked_accounts {
        if blocked_account_meta.is_writable {
            accounts_meta.push(AccountMeta::new(blocked_account_meta.key, false));
        } else {
            accounts_meta.push(AccountMeta::new_readonly(blocked_account_meta.key, false));
        }
    }
    for meta in &accounts_meta {
        info!("\t{:?}", meta);
    }

    let cancel_with_nonce_instruction = Instruction::new_with_bincode(
        evm_loader,
        &(0x23_u8, storage.transaction_hash),
        accounts_meta,
    );

    let instructions = vec![cancel_with_nonce_instruction];

    let signature = send_transaction(rpc_client, signer, &instructions).await?;

    Ok(CancelTrxReturn {
        transaction: signature,
    })
}
