use solana_sdk::{ pubkey::Pubkey };

use evm::{H160, U256};

use evm_loader::{
    account::{EthereumStorage},
    config::STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT,
};
use evm_loader::account::EthereumAccount;

use crate::{
    account_storage::{EmulatorAccountStorage, account_info },
    Config,
    rpc::Rpc,
};



pub fn execute(
    config: &Config,
    ether_address: H160,
    index: &U256
) {
    let value = if let (solana_address, Some(mut account)) = EmulatorAccountStorage::get_account_from_solana(config, &ether_address) {
        let info = account_info(&solana_address, &mut account);

        let account_data = EthereumAccount::from_account(&config.evm_loader, &info).unwrap();
        if let Some(contract) = account_data.contract_data() {
            if *index < U256::from(STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT) {
                let index: usize = index.as_usize() * 32;
                U256::from_big_endian(&contract.storage()[index..index + 32])
            } else {
                #[allow(clippy::cast_possible_truncation)]
                let subindex = (*index & U256::from(0xFF)).as_u64() as u8;
                let index = *index & !U256::from(0xFF);

                let seed = EthereumStorage::creation_seed(&index);
                let address = Pubkey::create_with_seed(&solana_address, &seed, &config.evm_loader).unwrap();

                if let Ok(mut account) = config.rpc_client.get_account(&address) {
                    if solana_sdk::system_program::check_id(&account.owner) {
                        U256::zero()
                    } else {
                        let account_info = account_info(&address, &mut account);
                        let storage = EthereumStorage::from_account(&config.evm_loader, &account_info).unwrap();
                        if (storage.address != ether_address) || (storage.index != index) || (storage.generation != account_data.generation) {
                            U256::zero()
                        } else {
                            storage.get(subindex)
                        }
                    }
                } else {
                    U256::zero()
                }
            }
        } else {
            U256::zero()
        }
    } else {
        U256::zero()
    };

    print!("{:#x}", value);
}
