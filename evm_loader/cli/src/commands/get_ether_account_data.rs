use evm::{H160};

use evm_loader::account::EthereumAccount;

use crate::{
    account_storage::{
        EmulatorAccountStorage,
        account_info,
    },
    Config,
};


pub fn execute (
    config: &Config,
    ether_address: &H160,
) {
    match EmulatorAccountStorage::get_account_from_solana(config, ether_address) {
        (solana_address, Some(mut acc)) => {
            let acc_info = account_info(&solana_address, &mut acc);
            let account_data = EthereumAccount::from_account(&config.evm_loader, &acc_info).unwrap();

            println!("Ethereum address: 0x{}", &hex::encode(ether_address.as_fixed_bytes()));
            println!("Solana address: {}", solana_address);

            println!("Account fields");
            println!("    address: {}", account_data.address);
            println!("    bump_seed: {}", account_data.bump_seed);
            println!("    trx_count: {}", account_data.trx_count);
            println!("    rw_blocked: {}", account_data.rw_blocked);
            println!("    balance: {}", account_data.balance);
            println!("    code_size: {}", account_data.code_size);

            if let Some(contract) = account_data.contract_data() {
                let code_size = account_data.code_size as usize;
                let mut offset = 0;
                while offset < code_size {
                    let data_slice = &contract.code();
                    let remains = if code_size - offset > 80 {
                        80
                    } else {
                        code_size - offset
                    };

                    println!("        {}", &hex::encode(&data_slice[offset..offset+remains]));
                    offset += remains;
                }
            }


        },
        (_, None) => {
            println!("Account not found {}", &ether_address.to_string());
        }
    }
}

