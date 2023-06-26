use std::convert::TryInto;

use ethnum::U256;

use evm_loader::account::EthereumAccount;
use evm_loader::{
    account::{ether_storage::EthereumStorageAddress, EthereumStorage},
    config::STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT,
    types::Address,
};

use crate::context::Context;
use crate::{
    account_storage::{account_info, EmulatorAccountStorage},
    Config, NeonResult,
};

pub type GetStorageAtReturn = String;

pub fn execute(
    config: &Config,
    context: &Context,
    ether_address: Address,
    index: &U256,
) -> NeonResult<GetStorageAtReturn> {
    let value = if let (solana_address, Some(mut account)) =
        EmulatorAccountStorage::get_account_from_solana(config, context, &ether_address)
    {
        let info = account_info(&solana_address, &mut account);

        let account_data = EthereumAccount::from_account(&config.evm_loader, &info)?;
        if let Some(contract) = account_data.contract_data() {
            if *index < U256::from(STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT) {
                let index: usize = index.as_usize() * 32;
                contract.storage()[index..index + 32].try_into().unwrap()
            } else {
                let subindex = (*index & 0xFF).as_u8();
                let index = *index & !U256::new(0xFF);

                let address =
                    EthereumStorageAddress::new(&config.evm_loader, account_data.info.key, &index);

                if let Ok(mut account) = context.rpc_client.get_account(address.pubkey()) {
                    if solana_sdk::system_program::check_id(&account.owner) {
                        <[u8; 32]>::default()
                    } else {
                        let account_info = account_info(address.pubkey(), &mut account);
                        let storage =
                            EthereumStorage::from_account(&config.evm_loader, &account_info)?;
                        if (storage.address != ether_address)
                            || (storage.index != index)
                            || (storage.generation != account_data.generation)
                        {
                            <[u8; 32]>::default()
                        } else {
                            storage.get(subindex)
                        }
                    }
                } else {
                    <[u8; 32]>::default()
                }
            }
        } else {
            <[u8; 32]>::default()
        }
    } else {
        <[u8; 32]>::default()
    };

    Ok(hex::encode(value))
}
