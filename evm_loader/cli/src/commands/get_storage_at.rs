use solana_sdk::{ pubkey::Pubkey };

use evm::{H160, U256};

use evm_loader::{
    account::{EthereumStorage, EthereumContract, ACCOUNT_SEED_VERSION},
    config::STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT,
};

use crate::{
    account_storage::{EmulatorAccountStorage, account_info },
    Config,
};



pub fn execute(
    config: &Config,
    ether_address: H160,
    index: &U256
) {
    let value = if let Some((_, Some(mut code_account))) =  EmulatorAccountStorage::get_account_from_solana(config, &ether_address) {
        let code_key = Pubkey::default();
        let code_info = account_info(&code_key, &mut code_account);

        let contract = EthereumContract::from_account(&config.evm_loader, &code_info).unwrap();

        if *index < U256::from(STORAGE_ENTIRIES_IN_CONTRACT_ACCOUNT) {
            let index: usize = index.as_usize() * 32;
            U256::from_big_endian(&contract.extension.storage[index..index+32])
        } else {
            let mut index_bytes = [0_u8; 32];
            index.to_little_endian(&mut index_bytes);
            let seeds: &[&[u8]] = &[&[ACCOUNT_SEED_VERSION], b"ContractStorage", ether_address.as_bytes(), &contract.generation.to_le_bytes(), &index_bytes];
        
            let (address, _) = Pubkey::find_program_address(seeds, &config.evm_loader);
        
            if let Ok(mut account) = config.rpc_client.get_account(&address) {
                if solana_sdk::system_program::check_id(&account.owner) {
                    U256::zero()
                } else {
                    let account_info = account_info(&address, &mut account);
                    let storage = EthereumStorage::from_account(&config.evm_loader, &account_info).unwrap();
                    storage.value
                }
            } else {
                U256::zero()
            }
        }
    } else {
        U256::zero()
    };

    print!("{:#x}", value);
}

