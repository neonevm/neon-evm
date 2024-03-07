use evm_loader::account::StateAccount;
use log::info;

use serde::{Deserialize, Serialize};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Signature,
    signer::Signer,
};

use crate::{
    account_storage::account_info, commands::send_transaction, rpc::CloneRpcClient, NeonResult,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct CancelTrxReturn {
    pub transaction: Signature,
}

pub async fn execute(
    rpc_client: CloneRpcClient,
    signer: &dyn Signer,
    program_id: Pubkey,
    storage_account: &Pubkey,
) -> NeonResult<CancelTrxReturn> {
    let mut acc = rpc_client.get_account(storage_account).await?;
    let storage_info = account_info(storage_account, &mut acc);
    let storage = StateAccount::from_account(&program_id, &storage_info)?;

    let operator = &signer.pubkey();

    let default_chain_id =
        crate::commands::get_config::read_default_chain_id(&rpc_client, program_id).await?;
    let chain_id = storage.trx().chain_id().unwrap_or(default_chain_id);

    let origin = storage.trx_origin();
    let (origin_pubkey, _) = origin.find_balance_address(&program_id, chain_id);

    let mut accounts_meta: Vec<AccountMeta> = vec![
        AccountMeta::new(*storage_account, false), // State account
        AccountMeta::new(*operator, true),         // Operator
        AccountMeta::new(origin_pubkey, false),
    ];

    for key in storage.accounts() {
        let meta = AccountMeta::new(*key, true);
        info!("\t{:?}", meta);

        accounts_meta.push(meta);
    }

    let cancel_with_nonce_instruction =
        Instruction::new_with_bincode(program_id, &(0x37_u8, storage.trx().hash()), accounts_meta);

    let instructions = vec![cancel_with_nonce_instruction];

    let signature = send_transaction(&rpc_client, signer, &instructions).await?;

    Ok(CancelTrxReturn {
        transaction: signature,
    })
}
