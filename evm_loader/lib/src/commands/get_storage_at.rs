use ethnum::U256;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

use evm_loader::{account_storage::AccountStorage, types::Address};

use crate::{account_storage::EmulatorAccountStorage, rpc::Rpc, NeonResult};

#[derive(Default, Serialize, Deserialize)]
pub struct GetStorageAtReturn(pub [u8; 32]);

pub async fn execute(
    rpc_client: &dyn Rpc,
    program_id: &Pubkey,
    address: Address,
    index: U256,
) -> NeonResult<GetStorageAtReturn> {
    let value = EmulatorAccountStorage::new(rpc_client, *program_id, None, None, None)
        .await?
        .storage(address, index)
        .await;

    Ok(GetStorageAtReturn(value))
}
