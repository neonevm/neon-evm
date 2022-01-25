use evm::{H160};

use evm_loader::{
    account_data::AccountData,
};

use crate::{
    account_storage::{
        EmulatorAccountStorage,
    },
    Config,
};


pub fn execute (
    config: &Config,
    ether_address: &H160,
) {
    match EmulatorAccountStorage::get_account_from_solana(config, ether_address) {
        Some((acc, balance, code_account)) => {
            let (solana_address, _solana_nonce) = crate::make_solana_program_address(ether_address, &config.evm_loader);
            let account_data = AccountData::unpack(&acc.data).unwrap();
            let account_data = AccountData::get_account(&account_data).unwrap();

            println!("Ethereum address: 0x{}", &hex::encode(&ether_address.as_fixed_bytes()));
            println!("Solana address: {}", solana_address);

            println!("Account fields");
            println!("    ether: {}", &account_data.ether);
            println!("    nonce: {}", &account_data.nonce);
            println!("    trx_count: {}", &account_data.trx_count);
            println!("    code_account: {}", &account_data.code_account);
            println!("    ro_blocked_cnt: {}", &account_data.ro_blocked_cnt);
            println!("    rw_blocked_acc: {}",
                     if account_data.rw_blocked_acc.is_some() {
                         account_data.rw_blocked_acc.unwrap().to_string()
                     }
                     else {
                         "".to_string()
                     }
            );
            println!("    token_account: {}", &account_data.eth_token_account);
            println!("    token_amount: {}", &balance);

            if let Some(code_account) = code_account {
                let code_data = AccountData::unpack(&code_account.data).unwrap();
                let header = AccountData::size(&code_data);
                let code_data = AccountData::get_contract(&code_data).unwrap();

                println!("Contract fields");
                println!("    owner: {}", &code_data.owner);
                println!("    code_size: {}", &code_data.code_size);
                println!("    code as hex:");

                let code_size = code_data.code_size;
                let mut offset = header;
                while offset < ( code_size as usize + header) {
                    let data_slice = &code_account.data.as_slice();
                    let remains = if code_size as usize + header - offset > 80 {
                        80
                    } else {
                        code_size as usize + header - offset
                    };

                    println!("        {}", &hex::encode(&data_slice[offset+header..offset+header+remains]));
                    offset += remains;
                }
            }


        },
        None => {
            println!("Account not found {}", &ether_address.to_string());
        }
    }
}

