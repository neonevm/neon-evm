use evm::{H160};

use evm_loader::{
    account::{EthereumAccount, EthereumContract},
};

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
        Some((mut acc, code_account)) => {
            let (solana_address, _solana_nonce) = crate::make_solana_program_address(ether_address, &config.evm_loader);
            let acc_info = account_info(&solana_address, &mut acc);

            let account_data = EthereumAccount::from_account(&config.evm_loader, &acc_info).unwrap();

            println!("Ethereum address: 0x{}", &hex::encode(ether_address.as_fixed_bytes()));
            println!("Solana address: {}", solana_address);

            println!("Account fields");
            println!("    address: {}", account_data.address);
            println!("    bump_seed: {}", account_data.bump_seed);
            println!("    trx_count: {}", account_data.trx_count);
            if let Some(code_account) = account_data.code_account {
                println!("    code_account: {}", code_account);
            } else {
                println!("    code_account: None");
            }
            println!("    ro_blocked_count: {}", account_data.ro_blocked_count);
            println!("    rw_blocked: {}", account_data.rw_blocked);
            println!("    balance: {}", account_data.balance);

            if let Some(mut code_account) = code_account {
                let code_key = account_data.code_account.unwrap();
                let code_info = account_info(&code_key, &mut code_account);
                let code_data = EthereumContract::from_account(&config.evm_loader, &code_info).unwrap();

                println!("Contract fields");
                println!("    owner: {}", &code_data.owner);
                println!("    code_size: {}", &code_data.code_size);
                println!("    code as hex:");

                let code_size = code_data.code_size as usize;
                let mut offset = 0;
                while offset < code_size {
                    let data_slice = &code_data.extension.code;
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
        None => {
            println!("Account not found {}", &ether_address.to_string());
        }
    }
}

