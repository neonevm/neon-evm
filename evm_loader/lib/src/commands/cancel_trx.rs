use evm_loader::account::StateAccount;
use log::info;

use serde::{Deserialize, Serialize};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Signature,
    signer::Signer,
};

use crate::{account_storage::account_info, commands::send_transaction, NeonResult};

#[derive(Debug, Serialize, Deserialize)]
pub struct CancelTrxReturn {
    pub transaction: Signature,
}

pub async fn execute(
    rpc_client: &RpcClient,
    signer: &dyn Signer,
    evm_loader: Pubkey,
    storage_account: &Pubkey,
) -> NeonResult<CancelTrxReturn> {
    let mut acc = rpc_client.get_account(storage_account).await?;
    let storage_info = account_info(storage_account, &mut acc);
    let storage = StateAccount::from_account(&evm_loader, storage_info)?;

    let operator = &signer.pubkey();

    let origin = storage.trx_origin();
    let chain_id: u64 = storage.trx_chain_id();
    let (origin_pubkey, _) = origin.find_balance_address(&evm_loader, chain_id);

    let mut accounts_meta: Vec<AccountMeta> = vec![
        AccountMeta::new(*storage_account, false), // State account
        AccountMeta::new(*operator, true),         // Operator
        AccountMeta::new(origin_pubkey, false),
    ];

    for blocked_account_meta in storage.blocked_accounts().iter() {
        if blocked_account_meta.is_writable {
            accounts_meta.push(AccountMeta::new(blocked_account_meta.key, false));
        } else {
            accounts_meta.push(AccountMeta::new_readonly(blocked_account_meta.key, false));
        }
    }
    for meta in &accounts_meta {
        info!("\t{:?}", meta);
    }

    let cancel_with_nonce_instruction =
        Instruction::new_with_bincode(evm_loader, &(0x37_u8, storage.trx_hash()), accounts_meta);

    let instructions = vec![cancel_with_nonce_instruction];

    let signature = send_transaction(rpc_client, signer, &instructions).await?;

    Ok(CancelTrxReturn {
        transaction: signature,
    })
}
