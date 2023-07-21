use std::convert::TryInto;
use std::fmt::{Display, Formatter};

use ethnum::U256;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

use evm_loader::account::EthereumAccount;
use evm_loader::{
    account::{ether_storage::EthereumStorageAddress, EthereumStorage},
    config::STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT,
    types::Address,
};

use crate::{
    account_storage::{account_info, EmulatorAccountStorage},
    rpc::Rpc,
    types::block,
    NeonResult,
};

#[derive(Default, Serialize, Deserialize)]
pub struct GetStorageAtReturn(pub [u8; 32]);

impl Display for GetStorageAtReturn {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{}", hex::encode(&self.0))
    }
}

pub async fn execute(
    rpc_client: &dyn Rpc,
    evm_loader: &Pubkey,
    ether_address: Address,
    index: &U256,
) -> NeonResult<GetStorageAtReturn> {
    let value = if let (solana_address, Some(mut account)) =
        EmulatorAccountStorage::get_account_from_solana(rpc_client, evm_loader, &ether_address)
            .await
    {
        let info = account_info(&solana_address, &mut account);

        let account_data = EthereumAccount::from_account(evm_loader, &info)?;
        if let Some(contract) = account_data.contract_data() {
            if *index < U256::from(STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT) {
                let index: usize = index.as_usize() * 32;
                GetStorageAtReturn(contract.storage()[index..index + 32].try_into().unwrap())
            } else {
                let subindex = (*index & 0xFF).as_u8();
                let index = *index & !U256::new(0xFF);

                let address =
                    EthereumStorageAddress::new(evm_loader, account_data.info.key, &index);

                if let Ok(mut account) = block(rpc_client.get_account(address.pubkey())) {
                    if solana_sdk::system_program::check_id(&account.owner) {
                        Default::default()
                    } else {
                        let account_info = account_info(address.pubkey(), &mut account);
                        let storage = EthereumStorage::from_account(evm_loader, &account_info)?;
                        if (storage.address != ether_address)
                            || (storage.index != index)
                            || (storage.generation != account_data.generation)
                        {
                            Default::default()
                        } else {
                            GetStorageAtReturn(storage.get(subindex))
                        }
                    }
                } else {
                    Default::default()
                }
            }
        } else {
            Default::default()
        }
    } else {
        Default::default()
    };

    Ok(value)
}
