use log::{ trace };

use solana_sdk::{ pubkey::Pubkey };

use evm::{H160, U256};

use evm_loader::{
    account::{EthereumStorage, ACCOUNT_SEED_VERSION},
};

use crate::{
    account_storage::account_info,
    Config,
};

pub fn value(
    config: &Config,
    ether_address: H160,
    index: &U256
) -> U256 {
    trace!("Get Storage At {:?} - {}", ether_address, index);

    let mut index_bytes = [0_u8; 32];
    index.to_little_endian(&mut index_bytes);
    let seeds: &[&[u8]] = &[&[ACCOUNT_SEED_VERSION], b"ContractStorage", ether_address.as_bytes(), &index_bytes];

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


pub fn execute(
    config: &Config,
    ether_address: H160,
    index: &U256
) {
    trace!("Enter execution for address {:?}", ether_address);

    print!("{:#x}", value(config, ether_address, index));
}

